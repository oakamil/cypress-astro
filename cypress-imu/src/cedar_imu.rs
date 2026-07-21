// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms

use async_trait::async_trait;
use std::sync::Arc;
use std::time::SystemTime;

use canonical_error::{CanonicalError, CanonicalErrorCode};
use cedar_elements::imu_trait::{
    AccelData, GyroData, HorizonCoordinates, ImuState, ImuTrait, TrackerState,
    TransformCalibration, ZeroBias,
};

use olive_imu::{Imu, MotionState, MountCoordinates};

pub struct CedarImuWrapper {
    pub engine: Arc<Imu>,
}

impl CedarImuWrapper {
    pub fn new(engine: Arc<Imu>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl ImuTrait for CedarImuWrapper {
    async fn report_true_camera_pointing(
        &self,
        camera_pointing: &HorizonCoordinates,
        timestamp: &SystemTime,
    ) {
        let mount_coords = MountCoordinates {
            roll: camera_pointing.zenith_roll_angle,
            pitch: camera_pointing.altitude,
            yaw: camera_pointing.azimuth,
        };

        self.engine.update_anchor(&mount_coords, timestamp).await;
    }

    async fn report_camera_pointing_lost(&self, _timestamp: &SystemTime) {}

    async fn reset(&self) {
        self.engine.reset_anchors().await;
        self.engine.reset_bias_calibration();
    }

    async fn get_estimated_camera_pointing(
        &self,
        timestamp: &SystemTime,
    ) -> Result<HorizonCoordinates, CanonicalError> {
        match self.engine.get_estimated_pointing(timestamp).await {
            Ok((coords, _is_imu_estimate)) => Ok(HorizonCoordinates {
                zenith_roll_angle: coords.roll,
                altitude: coords.pitch,
                azimuth: coords.yaw,
            }),
            Err(e) => Err(CanonicalError {
                code: CanonicalErrorCode::FailedPrecondition,
                message: e.to_string(),
            }),
        }
    }

    async fn get_tracker_state(&self) -> TrackerState {
        match self.engine.get_motion_state() {
            MotionState::Initializing => TrackerState::Lost,
            MotionState::Moving => TrackerState::Moving,
            MotionState::Stable => TrackerState::Motionless,
        }
    }

    async fn get_calibration(&self) -> (Option<ZeroBias>, Option<TransformCalibration>) {
        // Because the olive-imu engine now uses a continuous EMA loop to automatically
        // track and apply zero-bias under the hood, we return the internal EMA bias here
        // so clients can see the current baseline offset.
        let bias_vec = self.engine.get_bias();
        let bias = Some(ZeroBias {
            x: bias_vec.x,
            y: bias_vec.y,
            z: bias_vec.z,
        });

        let metrics = self.engine.get_calibration_metrics().await;
        let calibration = metrics.map(|m| TransformCalibration {
            transform_error_fraction: m.transform_error_fraction,
            camera_view_gyro_axis: m.camera_view_gyro_axis,
            camera_view_misalignment: m.camera_view_misalignment,
            camera_up_gyro_axis: m.camera_up_gyro_axis,
            camera_up_misalignment: m.camera_up_misalignment,
        });

        (bias, calibration)
    }

    async fn get_state(&self) -> Result<(ImuState, SystemTime), CanonicalError> {
        if let Some(update) = self.engine.get_latest_state() {
            Ok((
                ImuState {
                    timestamp: update.timestamp,
                    accel: AccelData {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    gyro: GyroData {
                        x: update.gyro.x,
                        y: update.gyro.y,
                        z: update.gyro.z,
                    },
                },
                update.timestamp,
            ))
        } else {
            Err(CanonicalError {
                code: CanonicalErrorCode::Unavailable,
                message: "IMU Not Ready".into(),
            })
        }
    }

    async fn get_jerk_magnitude(&self) -> Result<(f64, SystemTime), CanonicalError> {
        if let Some(state) = self.engine.get_latest_state() {
            // Gyro-only implementation, jerk is 0.0
            Ok((0.0, state.timestamp))
        } else {
            Err(CanonicalError {
                code: CanonicalErrorCode::Unavailable,
                message: "IMU Not Ready".into(),
            })
        }
    }

    async fn get_angular_velocity_magnitude(&self) -> Result<(f64, SystemTime), CanonicalError> {
        if let Some(state) = self.engine.get_latest_state() {
            Ok((state.angular_velocity, state.timestamp))
        } else {
            Err(CanonicalError {
                code: CanonicalErrorCode::Unavailable,
                message: "IMU Not Ready".into(),
            })
        }
    }

    fn get_model(&self) -> String {
        "Generic/Olive-IMU".to_string()
    }

    fn start(&self) {}

    fn save_state(&self) -> Result<(), CanonicalError> {
        // Handled automatically by the persistent storage trait inside olive-imu
        Ok(())
    }
}
