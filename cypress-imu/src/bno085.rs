use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use canonical_error::{CanonicalError, CanonicalErrorCode};
use cedar_elements::imu_trait::{
    AccelData, GyroData, HorizonCoordinates, ImuState, ImuTrait, TrackerState,
    TransformCalibration, ZeroBias,
};
use log::{debug, info, warn};
use nalgebra::{Matrix3, Rotation3, UnitQuaternion, Vector3};
use tokio::sync::{RwLock, watch};

const CALIBRATION_FILE: &str = "imu_calibration.txt";

#[derive(Clone, Copy, Debug)]
struct ImuUpdate {
    timestamp: SystemTime,
    accel: AccelData,
    gyro: GyroData,
    quaternion: UnitQuaternion<f64>,
    jerk_magnitude: f64,
    angular_velocity: f64,
    tracker_state: TrackerState,
}

#[derive(Clone)]
struct AlignmentState {
    // The known true camera pointing from the most recent successful plate solve.
    last_camera_position: Option<HorizonCoordinates>,
    // The IMU's raw quaternion recorded at the exact time the plate solve image was taken.
    imu_anchor_state: Option<UnitQuaternion<f64>>,
    // The dynamically calculated physical mounting rotation of the IMU relative to the camera.
    mount_q: UnitQuaternion<f64>,
    // The calculated calibration health between the camera and IMU.
    transform_calibration: Option<TransformCalibration>,
    // Store distinct rotational axes to continuously refine the 3D calibration via SVD
    calibration_axes: Vec<(Vector3<f64>, Vector3<f64>)>,
}

impl Default for AlignmentState {
    fn default() -> Self {
        Self {
            last_camera_position: None,
            imu_anchor_state: None,
            mount_q: UnitQuaternion::identity(),
            transform_calibration: None,
            calibration_axes: Vec::new(),
        }
    }
}

pub struct Bno085Imu {
    state_rx: watch::Receiver<Option<ImuUpdate>>,
    alignment: Arc<RwLock<AlignmentState>>,
    history: Arc<Mutex<VecDeque<(SystemTime, UnitQuaternion<f64>)>>>,
}

impl Bno085Imu {
    pub fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let (state_tx, state_rx) = watch::channel(None);

        let mut initial_alignment = AlignmentState::default();

        // Load previous calibration
        if let Ok(data) = std::fs::read_to_string(CALIBRATION_FILE) {
            let parts: Vec<&str> = data.trim().split(',').collect();
            if parts.len() == 4 {
                if let (Ok(x), Ok(y), Ok(z), Ok(w)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                    parts[3].parse::<f64>(),
                ) {
                    // nalgebra::Quaternion::new takes (w, i, j, k) -> (w, x, y, z)
                    initial_alignment.mount_q =
                        UnitQuaternion::new_normalize(nalgebra::Quaternion::new(w, x, y, z));
                    info!(
                        "Successfully loaded saved calibration: {:.3}, {:.3}, {:.3}, {:.3}",
                        x, y, z, w
                    );
                } else {
                    info!("Could not parse calibration file. Starting fresh.");
                }
            }
        }

        let alignment = Arc::new(RwLock::new(initial_alignment));

        // 5 seconds of IMU history
        let history = Arc::new(Mutex::new(VecDeque::with_capacity(250)));
        let history_clone = Arc::clone(&history);

        std::thread::spawn(move || {
            use bno080::interface::i2c::I2cInterface;
            use bno080::wrapper::BNO080;
            use linux_embedded_hal::{Delay, I2cdev};

            info!("Spawning I2C polling thread...");
            let i2c = I2cdev::new("/dev/i2c-1").expect("Failed to open Raspberry Pi I2C bus");
            let interface = I2cInterface::new(i2c, 0x4B);
            let mut imu = BNO080::new_with_interface(interface);

            let mut delay = Delay {};
            imu.init(&mut delay)
                .expect("Failed to initialize BNO085 over I2C");

            let report_interval_ms = 20; // 50Hz
            imu.enable_game_rotation_vector(report_interval_ms).unwrap();
            info!("Hardware initialized at 50Hz.");

            let mut prev_quat = UnitQuaternion::identity();
            let mut prev_time = SystemTime::now();
            let boot_time = SystemTime::now();
            let warm_up_duration = Duration::from_secs(3);

            let mut last_debug_print = Instant::now();
            let mut last_msg_time = SystemTime::now(); // Hardware watchdog tracker

            loop {
                std::thread::sleep(Duration::from_millis(5));

                let msg_count = imu.handle_all_messages(&mut delay, 5);

                if msg_count > 0 {
                    if let Ok(quat) = imu.game_rotation_quaternion() {
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

                            let now = SystemTime::now();
                            last_msg_time = now; // Kick the watchdog

                            let dt = now
                                .duration_since(prev_time)
                                .unwrap_or(Duration::from_millis(20))
                                .as_secs_f64();

                            let q_diff = prev_quat.conjugate() * current_quat;
                            let angle_rad = q_diff.angle();
                            let gyro_mag = angle_rad / dt.max(0.001);

                            let is_warming_up =
                                boot_time.elapsed().unwrap_or_default() < warm_up_duration;

                            let tracker_state = if is_warming_up {
                                TrackerState::Lost
                            } else if gyro_mag > 0.05 {
                                TrackerState::Moving
                            } else {
                                TrackerState::Motionless
                            };

                            let update = ImuUpdate {
                                timestamp: now,
                                accel: AccelData {
                                    x: 0.0,
                                    y: 0.0,
                                    z: 0.0,
                                },
                                gyro: GyroData {
                                    x: 0.0,
                                    y: 0.0,
                                    z: 0.0,
                                },
                                quaternion: current_quat,
                                jerk_magnitude: 0.0,
                                angular_velocity: gyro_mag,
                                tracker_state,
                            };

                            let _ = state_tx.send(Some(update));

                            {
                                let mut hist = history_clone.lock().unwrap();
                                hist.push_back((now, current_quat));
                                if hist.len() > 3000 {
                                    hist.pop_front();
                                }
                            }

                            if last_debug_print.elapsed() >= Duration::from_secs(1) {
                                debug!("Alive @ 50Hz. TrackerState: {:?}", tracker_state);
                                last_debug_print = Instant::now();
                            }

                            prev_quat = current_quat;
                            prev_time = now;
                        }
                    }
                } else {
                    // Hardware watchdog
                    if last_msg_time.elapsed().unwrap_or_default() > Duration::from_secs(2) {
                        warn!("Sensor unresponsive for 2s. Sending hardware revive command...");
                        let _ = imu.enable_game_rotation_vector(report_interval_ms);
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

        for (time, quat) in hist.iter() {
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

    fn horizon_to_quat(coord: &HorizonCoordinates) -> UnitQuaternion<f64> {
        UnitQuaternion::from_euler_angles(
            coord.zenith_roll_angle.to_radians(),
            coord.altitude.to_radians(),
            coord.azimuth.to_radians(),
        )
    }

    fn quat_to_horizon(quat: &UnitQuaternion<f64>) -> HorizonCoordinates {
        let (roll, pitch, yaw) = quat.euler_angles();
        HorizonCoordinates {
            zenith_roll_angle: roll.to_degrees().rem_euclid(360.0),
            altitude: pitch.to_degrees(),
            azimuth: yaw.to_degrees().rem_euclid(360.0),
        }
    }

    // Helper to spin up a non-blocking save
    fn save_calibration_to_disk(mount_q: UnitQuaternion<f64>) {
        tokio::spawn(async move {
            // Write out simple CSV format: x,y,z,w
            let data = format!(
                "{},{},{},{}",
                mount_q[0], mount_q[1], mount_q[2], mount_q[3]
            );
            let _ = tokio::fs::write(CALIBRATION_FILE, data).await;
        });
    }
}

#[async_trait]
impl ImuTrait for Bno085Imu {
    async fn report_true_camera_pointing(
        &self,
        camera_pointing: &HorizonCoordinates,
        timestamp: &SystemTime,
    ) {
        debug!("report_true_camera_pointing called!");

        let imu_state = *self.state_rx.borrow();

        if imu_state.is_some() {
            let historical_imu_q = self.get_historical_quat(timestamp);

            if let Some(hist_q) = historical_imu_q {
                let mut align = self.alignment.write().await;
                let new_true_q = Self::horizon_to_quat(camera_pointing);

                if let (Some(old_horizon), Some(old_quat)) =
                    (align.last_camera_position, align.imu_anchor_state)
                {
                    let old_true_q = Self::horizon_to_quat(&old_horizon);

                    let aligned_old_quat = old_quat * align.mount_q;
                    let aligned_hist_q = hist_q * align.mount_q;

                    // Before we add anything to the SVD pool, we check if our current mount_q is totally wrong.
                    let imu_delta = aligned_old_quat.conjugate() * aligned_hist_q;
                    let expected_new_true_q = old_true_q * imu_delta;

                    let error_quat = expected_new_true_q.inverse() * new_true_q;
                    let error_angle = error_quat.angle().to_degrees();

                    if error_angle > 5.0 && !align.calibration_axes.is_empty() {
                        info!(
                            "Error {:.2}° > 5.0°. Camera may have moved. Wiping calibration.",
                            error_angle
                        );
                        align.mount_q = UnitQuaternion::identity();
                        align.calibration_axes.clear();
                        let _ = std::fs::remove_file(CALIBRATION_FILE);
                        // Reset, treat this solve as a fresh baseline and skip the SVD step
                    } else {
                        // Continuous SVD calibration (Wahba's Problem)
                        let q_true_delta = old_true_q.conjugate() * new_true_q;
                        let q_imu_delta = old_quat.conjugate() * hist_q;

                        let angle_moved = q_true_delta.angle().to_degrees();

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

                                    for (t, i) in &align.calibration_axes {
                                        b += t * i.transpose();
                                    }

                                    let svd = b.svd(true, true);
                                    if let (Some(u), Some(v_t)) = (svd.u, svd.v_t) {
                                        let det = (u * v_t).determinant();
                                        let mut d = Matrix3::identity();

                                        if det < 0.0 {
                                            d[(2, 2)] = -1.0;
                                        }

                                        let r_mount = u * d * v_t;

                                        if r_mount.iter().all(|val| val.is_finite()) {
                                            let rot3 = Rotation3::from_matrix_unchecked(r_mount);
                                            align.mount_q =
                                                UnitQuaternion::from_rotation_matrix(&rot3);
                                            debug!(
                                                "Refined 3D mount via SVD. Pool: {}",
                                                align.calibration_axes.len()
                                            );

                                            // Save the successfully refined matrix to disk in the background
                                            Self::save_calibration_to_disk(align.mount_q);
                                        } else {
                                            debug!(
                                                "SVD generated NaNs. Keeping previous safe mount_q."
                                            );
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
                        let final_aligned_old_quat = old_quat * align.mount_q;
                        let final_aligned_hist_q = hist_q * align.mount_q;

                        let final_imu_delta =
                            final_aligned_old_quat.conjugate() * final_aligned_hist_q;
                        let final_expected = old_true_q * final_imu_delta;

                        let final_error_quat = final_expected.inverse() * new_true_q;
                        let final_error_angle = final_error_quat.angle().to_degrees();

                        debug!(
                            "Calibration metric evaluated. Expected vs True error: {:.3}°",
                            final_error_angle
                        );

                        align.transform_calibration = Some(TransformCalibration {
                            transform_error_fraction: (final_error_angle / 100.0).clamp(0.0, 1.0),
                            camera_view_gyro_axis: "+Z".to_string(),
                            camera_view_misalignment: final_error_angle,
                            camera_up_gyro_axis: "+Y".to_string(),
                            camera_up_misalignment: final_error_angle,
                        });
                    }
                } else {
                    info!("Initial plate-solve anchor locked in.");
                    align.transform_calibration = Some(TransformCalibration {
                        transform_error_fraction: 0.0,
                        camera_view_gyro_axis: "+Z".to_string(),
                        camera_view_misalignment: 0.0,
                        camera_up_gyro_axis: "+Y".to_string(),
                        camera_up_misalignment: 0.0,
                    });
                }

                // Always strictly lock in the most recent plate solve and corresponding historical IMU state as our new anchor.
                align.last_camera_position = Some(*camera_pointing);
                align.imu_anchor_state = Some(hist_q);
                debug!("Anchors successfully updated.");
            } else {
                debug!("Failed to match IMU history to plate solve timestamp. Anchors unchanged.");
            }
        }
    }

    // Ignored. Dead reckoning continues based on the last known anchors.
    async fn report_camera_pointing_lost(&self, _timestamp: &SystemTime) {}

    // Force get_estimated_camera_pointing() to return an error until
    // report_true_camera_pointing() is called again.
    async fn reset(&self) {
        debug!("reset called. Clearing all anchors and calibration data.");
        let mut align = self.alignment.write().await;
        align.last_camera_position = None;
        align.imu_anchor_state = None;
        align.mount_q = UnitQuaternion::identity();
        align.calibration_axes.clear();
        align.transform_calibration = None;
        let _ = std::fs::remove_file(CALIBRATION_FILE); // Wipe disk on explicit reset
    }

    // IMU-derived estimate of camera pointing as of the given time.
    async fn get_estimated_camera_pointing(
        &self,
        timestamp: &SystemTime,
    ) -> Result<HorizonCoordinates, CanonicalError> {
        let align = self.alignment.read().await.clone();

        if let (Some(anchor_horiz), Some(anchor_quat)) =
            (align.last_camera_position, align.imu_anchor_state)
        {
            let target_q = self.get_historical_quat(timestamp).unwrap_or_else(|| {
                debug!("get_estimated falling back to real-time quaternion");
                self.state_rx.borrow().unwrap().quaternion
            });

            // Apply the mount calibration to our raw anchor and raw target to get them in the same frame
            let aligned_anchor_q = anchor_quat * align.mount_q;
            let aligned_target_q = target_q * align.mount_q;

            // Calculate the delta movement from the anchor IMU point to the target IMU point
            let imu_delta = aligned_anchor_q.conjugate() * aligned_target_q;

            let anchor_true_q = Self::horizon_to_quat(&anchor_horiz);

            // Apply that movement delta directly to the true sky anchor
            let est_q = anchor_true_q * imu_delta;

            let coords = Self::quat_to_horizon(&est_q);

            debug!(
                "Engine queried IMU pointing. Returning Alt: {:.2}°, Az: {:.2}°",
                coords.altitude, coords.azimuth
            );

            Ok(coords)
        } else {
            Err(CanonicalError {
                code: CanonicalErrorCode::FailedPrecondition,
                message: "No plate solve anchor established yet.".to_string(),
            })
        }
    }

    async fn get_tracker_state(&self) -> TrackerState {
        self.state_rx
            .borrow()
            .map(|s| s.tracker_state)
            .unwrap_or(TrackerState::Lost)
    }

    async fn get_calibration(&self) -> (Option<ZeroBias>, Option<TransformCalibration>) {
        let align = self.alignment.read().await.clone();
        let state = self
            .state_rx
            .borrow()
            .map(|s| s.tracker_state)
            .unwrap_or(TrackerState::Lost);
        let bias = if state != TrackerState::Lost {
            Some(ZeroBias {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            })
        } else {
            None
        };
        (bias, align.transform_calibration)
    }

    async fn get_state(&self) -> Result<(ImuState, SystemTime), CanonicalError> {
        if let Some(state) = *self.state_rx.borrow() {
            Ok((
                ImuState {
                    timestamp: state.timestamp,
                    accel: state.accel,
                    gyro: state.gyro,
                },
                state.timestamp,
            ))
        } else {
            Err(CanonicalError {
                code: CanonicalErrorCode::Unavailable,
                message: "IMU Not Ready".into(),
            })
        }
    }

    async fn get_jerk_magnitude(&self) -> Result<(f64, SystemTime), CanonicalError> {
        if let Some(state) = *self.state_rx.borrow() {
            Ok((state.jerk_magnitude, state.timestamp))
        } else {
            Err(CanonicalError {
                code: CanonicalErrorCode::Unavailable,
                message: "IMU Not Ready".into(),
            })
        }
    }

    async fn get_angular_velocity_magnitude(&self) -> Result<(f64, SystemTime), CanonicalError> {
        if let Some(state) = *self.state_rx.borrow() {
            Ok((state.angular_velocity, state.timestamp))
        } else {
            Err(CanonicalError {
                code: CanonicalErrorCode::Unavailable,
                message: "IMU Not Ready".into(),
            })
        }
    }

    fn get_model(&self) -> String {
        "BNO085".to_string()
    }
}
