// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use log::{debug, error, info, warn};
use nalgebra::{Matrix3, Rotation3, UnitQuaternion, Vector3};
use tokio::sync::{RwLock, watch};

const CALIBRATION_FILE: &str = "imu_calibration.txt";

// --- SVD CONFIGURATION CONSTANTS ---
const SVD_MATURITY_SIZE: usize = 15;
// The minimum 3D volume required to trust a calibration matrix.
// 0.15 (15%) requires an intentional physical roll of the telescope.
const MIN_CALIBRATION_CONFIDENCE: f64 = 0.15;
// If the hardware shifts by more than this amount, force a recalibration override.
const HARDWARE_ALTERATION_THRESHOLD_DEG: f64 = 10.0;
// Wait this many frames after physical movement stops to allow the accelerometer's
// gravity vector to completely level out.
const SETTLE_FRAMES: usize = 15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImuRotationMode {
    Standard,
    Game,
    ArvrStabilized,
    ArvrStabilizedGame,
    Gyro,
    GyroHybrid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MotionState {
    Initializing,
    Moving,
    Stable,
}

#[derive(Clone, Copy, Debug)]
pub struct RawSensorData {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountCoordinates {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
}

#[derive(Clone, Debug)]
pub struct TransformMetrics {
    pub transform_error_fraction: f64,
    pub camera_view_gyro_axis: String,
    pub camera_view_misalignment: f64,
    pub camera_up_gyro_axis: String,
    pub camera_up_misalignment: f64,
}

// Helper to strictly enforce saving to the program's launch directory
fn get_calibration_file_path() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(CALIBRATION_FILE)
}

#[derive(Clone, Copy, Debug)]
pub struct ImuUpdate {
    pub timestamp: SystemTime,
    pub accel: RawSensorData,
    pub gyro: RawSensorData,
    pub quaternion: UnitQuaternion<f64>,
    pub jerk_magnitude: f64,
    pub angular_velocity: f64,
    pub motion_state: MotionState,
}

#[derive(Clone)]
struct AlignmentState {
    // The known true camera pointing from the most recent successful plate solve.
    last_camera_position: Option<MountCoordinates>,
    // The IMU's raw quaternion recorded at the exact time the plate solve image was taken.
    imu_anchor_state: Option<UnitQuaternion<f64>>,
    // The dynamically calculated physical mounting rotation of the IMU relative to the camera.
    mount_q: UnitQuaternion<f64>,
    // The calculated calibration health between the camera and IMU.
    transform_metrics: Option<TransformMetrics>,
    // Store distinct rotational axes to continuously refine the 3D calibration
    calibration_axes: Vec<(Vector3<f64>, Vector3<f64>)>,
    // Flag to track if the current mount_q was loaded from disk or previously locked
    loaded_from_disk: bool,
    // The highest confidence score achieved by the currently locked mount_q
    best_calibration_confidence: f64,
    // Counter to throttle SD card writes
    calibration_updates_since_save: usize,
    // Rolling history of expected vs true error for metric tracking
    error_history: Vec<f64>,
}

impl Default for AlignmentState {
    fn default() -> Self {
        Self {
            last_camera_position: None,
            imu_anchor_state: None,
            mount_q: UnitQuaternion::identity(),
            transform_metrics: None,
            calibration_axes: Vec::new(),
            loaded_from_disk: false,
            best_calibration_confidence: 0.0,
            calibration_updates_since_save: 0,
            error_history: Vec::new(),
        }
    }
}

pub struct Bno085Imu {
    state_rx: watch::Receiver<Option<ImuUpdate>>,
    alignment: Arc<RwLock<AlignmentState>>,
    // History buffer stores MotionState to enable State-Bracketed Extraction,
    // and a per-axis Vector3<f64> to track rotational velocity for bias calculation.
    history: Arc<Mutex<VecDeque<(SystemTime, UnitQuaternion<f64>, MotionState, Vector3<f64>)>>>,
}

impl Bno085Imu {
    pub fn start(rotation_mode: ImuRotationMode) -> Result<Self, Box<dyn std::error::Error>> {
        // We use a watch channel so the latest IMU state can be asynchronously read
        // by the engine at any time without blocking or needing to consume a queue.
        let (state_tx, state_rx) = watch::channel(None);

        let mut initial_alignment = AlignmentState::default();
        let cal_path = get_calibration_file_path();

        // Load previous calibration (Strictly requires the 5-part format)
        if let Ok(data) = std::fs::read_to_string(&cal_path) {
            let parts: Vec<&str> = data.trim().split(',').collect();
            if parts.len() == 5 {
                if let (Ok(x), Ok(y), Ok(z), Ok(w), Ok(confidence)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                    parts[3].parse::<f64>(),
                    parts[4].parse::<f64>(),
                ) {
                    // nalgebra::Quaternion::new takes (w, i, j, k) -> (w, x, y, z)
                    initial_alignment.mount_q =
                        UnitQuaternion::new_normalize(nalgebra::Quaternion::new(w, x, y, z));
                    initial_alignment.loaded_from_disk = true; // Protect this saved matrix
                    initial_alignment.best_calibration_confidence = confidence;

                    info!(
                        "Successfully loaded saved calibration from {:?} (Confidence: {:.1}%): {:.3}, {:.3}, {:.3}, {:.3}",
                        cal_path,
                        initial_alignment.best_calibration_confidence * 100.0,
                        x,
                        y,
                        z,
                        w
                    );
                } else {
                    info!(
                        "Could not parse calibration file at {:?}. Starting fresh.",
                        cal_path
                    );
                }
            } else {
                info!(
                    "Calibration file at {:?} is not in the expected 5-part format. Starting fresh.",
                    cal_path
                );
            }
        } else {
            info!(
                "No calibration file found at {:?}. Starting fresh.",
                cal_path
            );
        }

        // --- SYNCHRONOUS HARDWARE INITIALIZATION ---
        // We initialize the hardware on the main thread so we can fail-fast
        // and return the error to the caller if the chip is dead or disconnected.
        use bno080::interface::i2c::I2cInterface;
        use bno080::wrapper::BNO080;
        use linux_embedded_hal::{Delay, I2cdev};

        info!("Initializing BNO085 hardware over I2C...");
        let i2c = I2cdev::new("/dev/i2c-1")?;
        let interface = I2cInterface::new(i2c, 0x4B);
        let mut imu = BNO080::new_with_interface(interface);

        // The BNO085 requires an embedded-hal Delay struct for certain startup sequences
        let mut delay = Delay {};
        imu.init(&mut delay)
            .map_err(|e| format!("Failed to initialize BNO085 over I2C: {:?}", e))?;

        let report_interval_ms = 10; // 100Hz
        match rotation_mode {
            ImuRotationMode::Standard => {
                imu.enable_rotation_vector(report_interval_ms)
                    .map_err(|e| format!("Failed to enable Standard rotation: {:?}", e))?;
                info!("Hardware initialized at 100Hz using Standard Rotation Vector (9-axis).");
            }
            ImuRotationMode::Game => {
                imu.enable_game_rotation_vector(report_interval_ms)
                    .map_err(|e| format!("Failed to enable Game rotation: {:?}", e))?;
                info!("Hardware initialized at 100Hz using Game Rotation Vector (6-axis).");
            }
            ImuRotationMode::ArvrStabilized => {
                imu.enable_arvr_stabilized_rotation_vector(report_interval_ms)
                    .map_err(|e| format!("Failed to enable AR/VR rotation: {:?}", e))?;
                info!(
                    "Hardware initialized at 100Hz using AR/VR Stabilized Rotation Vector (9-axis)."
                );
            }
            ImuRotationMode::ArvrStabilizedGame => {
                imu.enable_arvr_stabilized_game_rotation_vector(report_interval_ms)
                    .map_err(|e| format!("Failed to enable AR/VR Game rotation: {:?}", e))?;
                info!(
                    "Hardware initialized at 100Hz using AR/VR Stabilized Game Rotation Vector (6-axis)."
                );
            }
            ImuRotationMode::Gyro => {
                imu.enable_gyro(report_interval_ms)
                    .map_err(|e| format!("Failed to enable Calibrated Gyroscope: {:?}", e))?;
                info!("Hardware initialized at 100Hz using Calibrated Gyroscope.");
            }
            ImuRotationMode::GyroHybrid => {
                imu.enable_rotation_vector(report_interval_ms)
                    .map_err(|e| format!("Failed to enable Standard rotation: {:?}", e))?;
                imu.enable_gyro(report_interval_ms)
                    .map_err(|e| format!("Failed to enable Calibrated Gyroscope: {:?}", e))?;
                info!(
                    "Hardware initialized at 100Hz using GyroHybrid Mode (9-axis + Calibrated Gyroscope)."
                );
            }
        }

        let alignment = Arc::new(RwLock::new(initial_alignment));
        let alignment_clone = Arc::clone(&alignment);

        // 3 seconds of IMU history (capped at 300 items for 100Hz)
        let history = Arc::new(Mutex::new(VecDeque::with_capacity(300)));
        let history_clone = Arc::clone(&history);

        // --- ASYNCHRONOUS POLLING THREAD ---
        // We spawn a dedicated OS thread for hardware polling because I2C operations are
        // blocking and we strictly do not want to stall the async Tokio runtime.
        std::thread::spawn(move || {
            let mut prev_quat = UnitQuaternion::identity();
            let mut prev_time = SystemTime::now();
            let boot_time = SystemTime::now();
            let warm_up_duration = Duration::from_secs(3);

            let mut last_msg_time = SystemTime::now(); // Hardware watchdog tracker

            // Main hardware polling loop. Extracts messages from the I2C bus as fast as they arrive.
            loop {
                std::thread::sleep(Duration::from_millis(5));

                let msg_count = imu.handle_all_messages(&mut delay, 5);

                if msg_count > 0 {
                    let mut current_quat_opt = None;
                    let mut motion_state_opt = None;
                    let mut gyro_vec_deg_opt = None;
                    let mut gyro_mag_opt = None;
                    let mut gyro_raw = RawSensorData {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    };

                    let now = SystemTime::now();
                    let dt = now
                        .duration_since(prev_time)
                        .unwrap_or(Duration::from_millis(10)) // Default to 10ms for 100Hz
                        .as_secs_f64();

                    let is_warming_up = boot_time.elapsed().unwrap_or_default() < warm_up_duration;

                    match rotation_mode {
                        ImuRotationMode::Gyro => {
                            if let Ok(gyro_data) = imu.gyro() {
                                last_msg_time = now; // Kick the watchdog

                                let wx = gyro_data[0] as f64;
                                let wy = gyro_data[1] as f64;
                                let wz = gyro_data[2] as f64;
                                gyro_raw = RawSensorData {
                                    x: wx,
                                    y: wy,
                                    z: wz,
                                };

                                let gyro_vec_rad = Vector3::new(wx, wy, wz);
                                let gyro_mag = gyro_vec_rad.norm();
                                gyro_mag_opt = Some(gyro_mag);

                                let delta_q = if gyro_mag > 1e-6 {
                                    UnitQuaternion::new(gyro_vec_rad * dt)
                                } else {
                                    UnitQuaternion::identity()
                                };

                                let current_quat = prev_quat * delta_q;
                                current_quat_opt = Some(current_quat);

                                gyro_vec_deg_opt =
                                    Some(gyro_vec_rad * (180.0 / std::f64::consts::PI));

                                motion_state_opt = Some(if is_warming_up {
                                    MotionState::Initializing
                                } else if gyro_mag > 0.05 {
                                    MotionState::Moving
                                } else {
                                    MotionState::Stable
                                });
                            }
                        }
                        ImuRotationMode::GyroHybrid => {
                            if let (Ok(quat), Ok(gyro_data)) =
                                (imu.rotation_quaternion(), imu.gyro())
                            {
                                last_msg_time = now; // Kick the watchdog

                                let mag_sq = quat[0] * quat[0]
                                    + quat[1] * quat[1]
                                    + quat[2] * quat[2]
                                    + quat[3] * quat[3];
                                if mag_sq > 0.1 {
                                    let abs_9axis =
                                        UnitQuaternion::new_normalize(nalgebra::Quaternion::new(
                                            quat[3] as f64,
                                            quat[0] as f64,
                                            quat[1] as f64,
                                            quat[2] as f64,
                                        ));

                                    let wx = gyro_data[0] as f64;
                                    let wy = gyro_data[1] as f64;
                                    let wz = gyro_data[2] as f64;
                                    gyro_raw = RawSensorData {
                                        x: wx,
                                        y: wy,
                                        z: wz,
                                    };

                                    let gyro_vec_rad = Vector3::new(wx, wy, wz);
                                    let gyro_mag = gyro_vec_rad.norm();
                                    gyro_mag_opt = Some(gyro_mag);

                                    // Gyro integration
                                    let delta_q = if gyro_mag > 1e-6 {
                                        UnitQuaternion::new(gyro_vec_rad * dt)
                                    } else {
                                        UnitQuaternion::identity()
                                    };

                                    // Align prev_quat to absolute frame on first valid pass
                                    if prev_quat == UnitQuaternion::identity()
                                        && abs_9axis != UnitQuaternion::identity()
                                    {
                                        prev_quat = abs_9axis;
                                    }

                                    // 1. Calculate physical deltas in IMU body frame
                                    let q_diff_9 = prev_quat.conjugate() * abs_9axis;
                                    let q_diff_g = delta_q;

                                    // 2. Map deltas to Abstract Camera frame using SVD mount_q
                                    let align = alignment_clone.blocking_read();
                                    let mount_q = align.mount_q;
                                    let cam_diff_9 = mount_q * q_diff_9 * mount_q.conjugate();
                                    let cam_diff_g = mount_q * q_diff_g * mount_q.conjugate();

                                    // 3. Extract rotation vectors (x=Roll, y=Altitude, z=Azimuth)
                                    let vec_9 = cam_diff_9.scaled_axis();
                                    let vec_g = cam_diff_g.scaled_axis();

                                    let mut vec_hybrid = vec_9;

                                    // 4. Hybridize: Altitude (y) from Gyro, Azimuth/Roll (x, z) from 9-axis
                                    vec_hybrid.y = vec_g.y;

                                    // 5. Map back to IMU body frame and apply
                                    let cam_diff_hybrid = UnitQuaternion::new(vec_hybrid);
                                    let q_diff_hybrid =
                                        mount_q.conjugate() * cam_diff_hybrid * mount_q;

                                    current_quat_opt = Some(prev_quat * q_diff_hybrid);

                                    gyro_vec_deg_opt =
                                        Some(gyro_vec_rad * (180.0 / std::f64::consts::PI));

                                    motion_state_opt = Some(if is_warming_up {
                                        MotionState::Initializing
                                    } else if gyro_mag > 0.05 {
                                        MotionState::Moving
                                    } else {
                                        MotionState::Stable
                                    });
                                }
                            }
                        }
                        _ => {
                            let quat_result = match rotation_mode {
                                ImuRotationMode::Standard => imu.rotation_quaternion(),
                                ImuRotationMode::Game => imu.game_rotation_quaternion(),
                                ImuRotationMode::ArvrStabilized => {
                                    imu.arvr_stabilized_rotation_quaternion()
                                }
                                ImuRotationMode::ArvrStabilizedGame => {
                                    imu.arvr_stabilized_game_rotation_quaternion()
                                }
                                _ => unreachable!(),
                            };

                            if let Ok(quat) = quat_result {
                                let mag_sq = quat[0] * quat[0]
                                    + quat[1] * quat[1]
                                    + quat[2] * quat[2]
                                    + quat[3] * quat[3];
                                if mag_sq > 0.1 {
                                    let current_quat =
                                        UnitQuaternion::new_normalize(nalgebra::Quaternion::new(
                                            quat[3] as f64,
                                            quat[0] as f64,
                                            quat[1] as f64,
                                            quat[2] as f64,
                                        ));

                                    last_msg_time = now; // Kick the watchdog

                                    let q_diff = prev_quat.conjugate() * current_quat;
                                    let angle_rad = q_diff.angle();
                                    let gyro_mag = angle_rad / dt.max(0.001);
                                    gyro_mag_opt = Some(gyro_mag);

                                    let gyro_vec_rad = if let Some(axis) = q_diff.axis() {
                                        axis.into_inner() * gyro_mag
                                    } else {
                                        Vector3::zeros()
                                    };
                                    gyro_vec_deg_opt =
                                        Some(gyro_vec_rad * (180.0 / std::f64::consts::PI));

                                    motion_state_opt = Some(if is_warming_up {
                                        MotionState::Initializing
                                    } else if gyro_mag > 0.05 {
                                        MotionState::Moving
                                    } else {
                                        MotionState::Stable
                                    });

                                    current_quat_opt = Some(current_quat);
                                }
                            }
                        }
                    }

                    if let (
                        Some(current_quat),
                        Some(motion_state),
                        Some(gyro_vec_deg),
                        Some(gyro_mag),
                    ) = (
                        current_quat_opt,
                        motion_state_opt,
                        gyro_vec_deg_opt,
                        gyro_mag_opt,
                    ) {
                        let update = ImuUpdate {
                            timestamp: now,
                            accel: RawSensorData {
                                x: 0.0,
                                y: 0.0,
                                z: 0.0,
                            },
                            gyro: gyro_raw,
                            quaternion: current_quat,
                            jerk_magnitude: 0.0,
                            angular_velocity: gyro_mag,
                            motion_state,
                        };

                        let _ = state_tx.send(Some(update));

                        {
                            let mut hist = history_clone.lock().unwrap();
                            hist.push_back((now, current_quat, motion_state, gyro_vec_deg));
                            if hist.len() > 300 {
                                hist.pop_front();
                            }
                        }

                        prev_quat = current_quat;
                        prev_time = now;
                    }
                } else {
                    // Hardware watchdog: The BNO085 occasionally locks up over I2C.
                    // If we haven't seen a packet in 2 seconds, we re-send the enable command.
                    if last_msg_time.elapsed().unwrap_or_default() > Duration::from_secs(2) {
                        warn!("Sensor unresponsive for 2s. Sending hardware revive command...");
                        match rotation_mode {
                            ImuRotationMode::Standard => {
                                let _ = imu.enable_rotation_vector(report_interval_ms);
                            }
                            ImuRotationMode::Game => {
                                let _ = imu.enable_game_rotation_vector(report_interval_ms);
                            }
                            ImuRotationMode::ArvrStabilized => {
                                let _ =
                                    imu.enable_arvr_stabilized_rotation_vector(report_interval_ms);
                            }
                            ImuRotationMode::ArvrStabilizedGame => {
                                let _ = imu.enable_arvr_stabilized_game_rotation_vector(
                                    report_interval_ms,
                                );
                            }
                            ImuRotationMode::Gyro => {
                                let _ = imu.enable_gyro(report_interval_ms);
                            }
                            ImuRotationMode::GyroHybrid => {
                                let _ = imu.enable_rotation_vector(report_interval_ms);
                                let _ = imu.enable_gyro(report_interval_ms);
                            }
                        }
                        last_msg_time = SystemTime::now();
                    }
                }
            }
        });

        Ok(Self {
            state_rx,
            alignment,
            history,
        })
    }

    // Helper to identify the closest primary IMU axis to a given camera vector
    fn get_closest_imu_axis(v: &Vector3<f64>) -> (String, f64) {
        let mut max_val = 0.0;
        let mut max_idx = 0;
        let mut sign = "+";

        for i in 0..3 {
            if v[i].abs() > max_val {
                max_val = v[i].abs();
                max_idx = i;
                sign = if v[i] >= 0.0 { "+" } else { "-" };
            }
        }

        let axis_name = match max_idx {
            0 => "X",
            1 => "Y",
            _ => "Z",
        };

        let mut pure_axis = Vector3::zeros();
        pure_axis[max_idx] = if sign == "+" { 1.0 } else { -1.0 };

        let misalignment = v.angle(&pure_axis).to_degrees();
        (format!("{}{}", sign, axis_name), misalignment)
    }

    // For real-time UI queries. Finds the literal closest timestamp without bracketing logic.
    fn get_historical_quat(&self, target_time: &SystemTime) -> Option<UnitQuaternion<f64>> {
        let hist = self.history.lock().unwrap();
        if hist.is_empty() {
            return None;
        }

        let oldest_time = hist.front().unwrap().0;

        if *target_time < oldest_time {
            let diff = oldest_time.duration_since(*target_time).unwrap_or_default();
            if diff > Duration::from_secs(2) {
                debug!(
                    "Plate solve is {}s older than our oldest history frame. Data expired.",
                    diff.as_secs()
                );
                return None;
            }
        }

        let mut closest_quat = hist[0].1;
        let mut min_diff = Duration::from_secs(u64::MAX);

        for (time, quat, _, _) in hist.iter() {
            let diff = if time > target_time {
                time.duration_since(*target_time).unwrap_or_default()
            } else {
                target_time.duration_since(*time).unwrap_or_default()
            };

            if diff < min_diff {
                min_diff = diff;
                closest_quat = *quat;
            }
        }

        if min_diff > Duration::from_secs(5) {
            warn!(
                "Nearest IMU frame is {}ms off. Rejecting out-of-sync timestamp.",
                min_diff.as_millis()
            );
            return None;
        }

        Some(closest_quat)
    }

    // State-bracketed extraction: Finds the exact moment the mount transitioned to Motionless.
    fn get_post_slew_quat(&self, target_time: &SystemTime) -> Option<UnitQuaternion<f64>> {
        let hist = self.history.lock().unwrap();
        if hist.is_empty() {
            return None;
        }

        let oldest_time = hist.front().unwrap().0;

        if *target_time < oldest_time {
            let diff = oldest_time.duration_since(*target_time).unwrap_or_default();
            if diff > Duration::from_secs(2) {
                return None;
            }
        }

        let mut closest_idx = 0;
        let mut min_diff = Duration::from_secs(u64::MAX);

        for (i, (time, _, _, _)) in hist.iter().enumerate() {
            let diff = if time > target_time {
                time.duration_since(*target_time).unwrap_or_default()
            } else {
                target_time.duration_since(*time).unwrap_or_default()
            };

            if diff < min_diff {
                min_diff = diff;
                closest_idx = i;
            }
        }

        if min_diff > Duration::from_secs(5) {
            return None;
        }

        // --- STATE-BRACKETED EXTRACTION WITH DECELERATION SETTLING ---
        let mut selected_idx = closest_idx;

        if hist[selected_idx].2 == MotionState::Moving {
            // Target landed during a slew. Scan forward (newer frames) to find the first Stable frame.
            for i in selected_idx..hist.len() {
                if hist[i].2 == MotionState::Stable {
                    selected_idx = i + SETTLE_FRAMES;
                    break;
                }
            }
        } else {
            // Target landed during stillness. Scan backward (older frames) to find the exact moment
            // the mount transitioned from Moving to Stable, isolating the true end of the slew.
            for i in (0..selected_idx).rev() {
                if hist[i].2 == MotionState::Moving {
                    selected_idx = (i + 1) + SETTLE_FRAMES;
                    break;
                }
            }
        }

        // Safety bound check: If the settle period pushes us past the newest frame in the buffer,
        // just grab the absolute newest frame available.
        if selected_idx >= hist.len() {
            selected_idx = hist.len() - 1;
        }

        Some(hist[selected_idx].1)
    }

    fn mount_to_quat(coord: &MountCoordinates) -> UnitQuaternion<f64> {
        UnitQuaternion::from_euler_angles(
            coord.roll.to_radians(),
            coord.pitch.to_radians(),
            coord.yaw.to_radians(),
        )
    }

    fn quat_to_mount(quat: &UnitQuaternion<f64>) -> MountCoordinates {
        let (roll, pitch, yaw) = quat.euler_angles();
        MountCoordinates {
            roll: roll.to_degrees().rem_euclid(360.0),
            pitch: pitch.to_degrees(),
            yaw: yaw.to_degrees().rem_euclid(360.0),
        }
    }

    // Helper to spin up a non-blocking save
    fn save_calibration_to_disk(mount_q: UnitQuaternion<f64>, confidence: f64) {
        tokio::spawn(async move {
            let cal_path = get_calibration_file_path();
            // Appending confidence to the end of the file
            let data = format!(
                "{},{},{},{},{}",
                mount_q[0], mount_q[1], mount_q[2], mount_q[3], confidence
            );
            match tokio::fs::write(&cal_path, data).await {
                Ok(_) => debug!("Successfully wrote calibration back to {:?}", cal_path),
                Err(e) => error!(
                    "CRITICAL: Failed to write calibration to {:?}. Error: {}",
                    cal_path, e
                ),
            }
        });
    }

    pub async fn update_anchor(&self, camera_pointing: &MountCoordinates, timestamp: &SystemTime) {
        let imu_state = *self.state_rx.borrow();

        if imu_state.is_some() {
            // Utilize State-Bracketed extraction to find the exact moment the slew finished
            let historical_imu_q = self.get_post_slew_quat(timestamp);

            if let Some(hist_q) = historical_imu_q {
                let mut align = self.alignment.write().await;
                let new_true_q = Self::mount_to_quat(camera_pointing);

                if let (Some(old_mount), Some(old_quat)) =
                    (align.last_camera_position, align.imu_anchor_state)
                {
                    let old_true_q = Self::mount_to_quat(&old_mount);

                    // Continuous SVD calibration (Wahba's Problem)
                    let q_true_delta = old_true_q.conjugate() * new_true_q;
                    let q_imu_delta = old_quat.conjugate() * hist_q;

                    let angle_moved = q_true_delta.angle().to_degrees();

                    // All distinct movements are now ingested organically to ensure bad matrices
                    // can heal themselves from true physical slews.
                    if angle_moved > 0.5 {
                        if let (Some(axis_true), Some(axis_imu)) =
                            (q_true_delta.axis(), q_imu_delta.axis())
                        {
                            let t_vec = axis_true.into_inner();
                            let i_vec = axis_imu.into_inner();

                            align.calibration_axes.push((t_vec, i_vec));

                            if align.calibration_axes.len() > 100 {
                                align.calibration_axes.remove(0);
                            }

                            let mut is_rank_sufficient = false;
                            for i in 0..align.calibration_axes.len() {
                                for j in (i + 1)..align.calibration_axes.len() {
                                    if align.calibration_axes[i]
                                        .0
                                        .dot(&align.calibration_axes[j].0)
                                        .abs()
                                        < 0.95
                                    {
                                        is_rank_sufficient = true;
                                        break;
                                    }
                                }
                                if is_rank_sufficient {
                                    break;
                                }
                            }

                            if is_rank_sufficient {
                                let mut b = Matrix3::zeros();

                                // SVD Matrix Construction
                                for (t, i) in &align.calibration_axes {
                                    b += t * i.transpose();
                                }

                                let svd = b.svd(true, true);
                                if let (Some(u), Some(v_t)) = (svd.u, svd.v_t) {
                                    // --- CALIBRATION CONFIDENCE SCORING ---
                                    // Extract singular values (sigma_1 is max, sigma_3 is min).
                                    // The ratio of min to max defines the true 3D geometric volume of the calibration.
                                    let sigma_1 = svd.singular_values[0];
                                    let sigma_3 = svd.singular_values[2];

                                    let new_pool_confidence = if sigma_1 > 0.0 {
                                        sigma_3 / sigma_1
                                    } else {
                                        0.0
                                    };

                                    let det = (u * v_t).determinant();
                                    let mut d = Matrix3::identity();
                                    if det < 0.0 {
                                        d[(2, 2)] = -1.0;
                                    }

                                    let r_mount = u * d * v_t;

                                    if r_mount.iter().all(|val| val.is_finite()) {
                                        let calculated_q = UnitQuaternion::from_rotation_matrix(
                                            &Rotation3::from_matrix_unchecked(r_mount),
                                        );

                                        let is_mature =
                                            align.calibration_axes.len() >= SVD_MATURITY_SIZE;
                                        let hardware_shift_deg = (align.mount_q.inverse()
                                            * calculated_q)
                                            .angle()
                                            .to_degrees();

                                        // Condition A: Hardware was physically altered (unscrewed and reattached)
                                        let hardware_altered = is_mature
                                            && new_pool_confidence >= MIN_CALIBRATION_CONFIDENCE
                                            && hardware_shift_deg
                                                > HARDWARE_ALTERATION_THRESHOLD_DEG;

                                        // We only overwrite the active mount_q if the new matrix proves it has
                                        // high 3D confidence (user rolled the tube) OR we started completely fresh
                                        // and need a temporary best-guess to power the UI.
                                        if !align.loaded_from_disk {
                                            // Bootstrapping phase. Update fluidly to get the UI tracking immediately.
                                            align.mount_q = calculated_q;
                                            align.best_calibration_confidence = new_pool_confidence;

                                            // Only lock to disk once it proves baseline 3D volume
                                            if is_mature
                                                && new_pool_confidence >= MIN_CALIBRATION_CONFIDENCE
                                            {
                                                align.loaded_from_disk = true; // Upgrade status to protected
                                                Self::save_calibration_to_disk(
                                                    align.mount_q,
                                                    align.best_calibration_confidence,
                                                );
                                                info!(
                                                    "Initial Calibration Locked! Confidence: {:.1}%",
                                                    align.best_calibration_confidence * 100.0
                                                );
                                            }
                                        } else if hardware_altered {
                                            // Overwrite the locked matrix because the physical structure changed
                                            warn!(
                                                "Hardware alteration detected! New matrix differs by {:.2}°. Resetting calibration constraints.",
                                                hardware_shift_deg
                                            );
                                            align.mount_q = calculated_q;
                                            align.best_calibration_confidence = new_pool_confidence;
                                            Self::save_calibration_to_disk(
                                                align.mount_q,
                                                align.best_calibration_confidence,
                                            );
                                        } else if new_pool_confidence
                                            > align.best_calibration_confidence
                                        {
                                            // Upgrade the locked matrix because EQ tracking naturally built a superior 3D volume
                                            info!(
                                                "Upgrading calibration matrix! Confidence increased from {:.1}% to {:.1}%",
                                                align.best_calibration_confidence * 100.0,
                                                new_pool_confidence * 100.0
                                            );
                                            align.mount_q = calculated_q;
                                            align.best_calibration_confidence = new_pool_confidence;

                                            align.calibration_updates_since_save += 1;
                                            if align.calibration_updates_since_save % 5 == 0 {
                                                Self::save_calibration_to_disk(
                                                    align.mount_q,
                                                    align.best_calibration_confidence,
                                                );
                                            }
                                        } else {
                                            // Coasting Phase. The active hardware is identical, and the new pool is flatter than our historical best.
                                            // Ignore the SVD calculation to protect the High Water Mark matrix.
                                        }
                                    } else {
                                        warn!("SVD generated NaNs. Keeping previous safe mount_q.");
                                    }
                                }
                            } else {
                                debug!(
                                    "Calibration pool lacks distinct axes. Need movement on a different plane to run SVD."
                                );
                            }
                        }
                    } else {
                        debug!(
                            "Movement ({:.2}°) too small to add to calibration pool.",
                            angle_moved
                        );
                    }

                    // Recalculate error metric post-SVD refinement for reporting
                    let imu_local_delta = old_quat.conjugate() * hist_q;
                    let cam_local_delta =
                        align.mount_q * imu_local_delta * align.mount_q.conjugate();
                    let final_expected = old_true_q * cam_local_delta;

                    let final_error_quat = final_expected.inverse() * new_true_q;
                    let final_error_angle = final_error_quat.angle().to_degrees();

                    if angle_moved > 5.0 {
                        align.error_history.push(final_error_angle);
                        if align.error_history.len() > 20 {
                            align.error_history.remove(0);
                        }
                        let avg_error: f64 = align.error_history.iter().sum::<f64>()
                            / (align.error_history.len() as f64);

                        // --- ALT/AZ ERROR COMPONENT LOGGING ---
                        let expected_coords = Self::quat_to_mount(&final_expected);
                        let true_coords = Self::quat_to_mount(&new_true_q);

                        let alt_error = true_coords.pitch - expected_coords.pitch;

                        let mut az_error = true_coords.yaw - expected_coords.yaw;
                        if az_error > 180.0 {
                            az_error -= 360.0;
                        }
                        if az_error < -180.0 {
                            az_error += 360.0;
                        }

                        let mut roll_error = true_coords.roll - expected_coords.roll;
                        if roll_error > 180.0 {
                            roll_error -= 360.0;
                        }
                        if roll_error < -180.0 {
                            roll_error += 360.0;
                        }

                        info!(
                            "Expected vs True error: {:.3}° (Rolling Avg: {:.3}°) | Alt Err: {:.3}°, Az Err: {:.3}°, Roll Err: {:.3}° | Confidence: {:.1}%",
                            final_error_angle,
                            avg_error,
                            alt_error,
                            az_error,
                            roll_error,
                            align.best_calibration_confidence * 100.0
                        );
                    }

                    // Assume standard optical camera axes: View = +Z, Up = +Y.
                    // Rotate the camera axes into the IMU's reference frame using our calibration matrix.
                    let cam_view_in_imu = align.mount_q.conjugate() * Vector3::new(0.0, 0.0, 1.0);
                    let cam_up_in_imu = align.mount_q.conjugate() * Vector3::new(0.0, 1.0, 0.0);

                    let (view_axis, view_misalign) = Self::get_closest_imu_axis(&cam_view_in_imu);
                    let (up_axis, up_misalign) = Self::get_closest_imu_axis(&cam_up_in_imu);

                    align.transform_metrics = Some(TransformMetrics {
                        transform_error_fraction: final_error_angle / angle_moved.max(0.001),
                        camera_view_gyro_axis: view_axis,
                        camera_view_misalignment: view_misalign,
                        camera_up_gyro_axis: up_axis,
                        camera_up_misalignment: up_misalign,
                    });
                } else {
                    info!("Initial plate-solve anchor locked in.");

                    let cam_view_in_imu = align.mount_q.conjugate() * Vector3::new(0.0, 0.0, 1.0);
                    let cam_up_in_imu = align.mount_q.conjugate() * Vector3::new(0.0, 1.0, 0.0);

                    let (view_axis, view_misalign) = Self::get_closest_imu_axis(&cam_view_in_imu);
                    let (up_axis, up_misalign) = Self::get_closest_imu_axis(&cam_up_in_imu);

                    align.transform_metrics = Some(TransformMetrics {
                        transform_error_fraction: 0.0,
                        camera_view_gyro_axis: view_axis,
                        camera_view_misalignment: view_misalign,
                        camera_up_gyro_axis: up_axis,
                        camera_up_misalignment: up_misalign,
                    });
                }

                // Always strictly lock in the most recent plate solve and corresponding historical IMU state as our new anchor.
                align.last_camera_position = Some(*camera_pointing);
                align.imu_anchor_state = Some(hist_q);
            }
        }
    }

    // Clears the active session anchors and telemetry, but strictly preserves
    // the SVD calibration pool, mount_q, and disk file to support file-less recalibration and
    // uninterrupted EQ tracking.
    pub async fn reset_anchors(&self) {
        debug!("reset called. Clearing anchors but preserving calibration matrix.");
        let mut align = self.alignment.write().await;

        // Clear active session anchors
        align.last_camera_position = None;
        align.imu_anchor_state = None;

        // Clear session-specific metrics, but preserve SVD pool, mount_q, and disk status
        align.transform_metrics = None;
        align.error_history.clear();
    }

    // IMU-derived estimate of camera pointing as of the given time.
    pub async fn get_estimated_pointing(
        &self,
        timestamp: &SystemTime,
    ) -> Result<MountCoordinates, &'static str> {
        let align = self.alignment.read().await.clone();

        if let (Some(anchor_horiz), Some(anchor_quat)) =
            (align.last_camera_position, align.imu_anchor_state)
        {
            // 1. Fetch the raw IMU state for the requested time
            let target_q = self.get_historical_quat(timestamp).unwrap_or_else(|| {
                debug!("get_estimated falling back to real-time quaternion");
                self.state_rx.borrow().unwrap().quaternion
            });

            // 2. Calculate the raw physical movement of the IMU chip itself.
            // This rotational delta is strictly in the IMU's local reference frame,
            // meaning it includes any arbitrary physical mounting offsets (roll, pitch, yaw)
            // between how the IMU is mounted versus how the camera sensor is oriented.
            let imu_local_delta = anchor_quat.conjugate() * target_q;

            // 3. Coordinate Transformation (Similarity Transform / Change of Basis).
            // We must convert the IMU's local movement into the camera's optical reference frame.
            // The mathematically universal transform is: Camera_Delta = Mount * IMU_Delta * Mount_Inverse.
            // This isolates the actual telescope movement by "untwisting" whatever arbitrary physical
            // mounting offset exists between the two sensors, preventing movements on one axis
            // from bleeding into another (e.g., preventing Azimuth pans from dipping into Altitude).
            let cam_local_delta = align.mount_q * imu_local_delta * align.mount_q.conjugate();

            // 4. Convert our known optical anchor (from the last successful plate solve) into a quaternion
            let anchor_true_q = Self::mount_to_quat(&anchor_horiz);

            // 5. Apply the correctly transformed movement delta directly to the true sky anchor
            let est_q = anchor_true_q * cam_local_delta;

            Ok(Self::quat_to_mount(&est_q))
        } else {
            Err("No plate solve anchor established yet.")
        }
    }

    pub fn get_motion_state(&self) -> MotionState {
        self.state_rx
            .borrow()
            .map(|s| s.motion_state)
            .unwrap_or(MotionState::Initializing)
    }

    pub fn get_recent_stable_gyro_bias(&self, max_frames: usize) -> Option<Vector3<f64>> {
        let hist = self.history.lock().unwrap();
        let mut bias_sum = Vector3::zeros();
        let mut count = 0;

        for (_, _, state, gyro_deg) in hist.iter().rev() {
            if *state == MotionState::Stable {
                bias_sum += *gyro_deg;
                count += 1;
                if count >= max_frames {
                    break;
                }
            } else {
                // Stop at the first sign of movement to ensure we only measure pure rest data
                break;
            }
        }

        if count > 0 {
            Some(bias_sum / (count as f64))
        } else {
            None
        }
    }

    pub async fn get_calibration_metrics(&self) -> Option<TransformMetrics> {
        self.alignment.read().await.transform_metrics.clone()
    }

    pub fn get_latest_state(&self) -> Option<ImuUpdate> {
        *self.state_rx.borrow()
    }
}
