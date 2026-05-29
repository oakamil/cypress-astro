// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use async_trait::async_trait;
use std::time::SystemTime;

use canonical_error::{CanonicalError, CanonicalErrorCode};
use cedar_elements::imu_trait::{
    AccelData, GyroData, HorizonCoordinates, ImuState, ImuTrait, TrackerState,
    TransformCalibration, ZeroBias,
};

use crate::bno085::{Bno085Imu, MotionState, MountCoordinates};

pub struct CedarBno085Wrapper {
    pub engine: Bno085Imu,
}

#[async_trait]
impl ImuTrait for CedarBno085Wrapper {
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

    // Ignored. Dead reckoning continues based on the last known anchors.
    async fn report_camera_pointing_lost(&self, _timestamp: &SystemTime) {}

    async fn reset(&self) {
        self.engine.reset_anchors().await;
    }

    async fn get_estimated_camera_pointing(
        &self,
        timestamp: &SystemTime,
    ) -> Result<HorizonCoordinates, CanonicalError> {
        match self.engine.get_estimated_pointing(timestamp).await {
            Ok(coords) => Ok(HorizonCoordinates {
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
        // Calculate dynamic zero bias from the most recent contiguous motionless frames (up to 50)
        let bias_vec = self.engine.get_recent_stable_gyro_bias(50);
        let bias = if let Some(vec) = bias_vec {
            Some(ZeroBias {
                x: vec.x,
                y: vec.y,
                z: vec.z,
            })
        } else {
            // If currently moving or buffer is empty, default to zero
            Some(ZeroBias {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            })
        };

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
        if let Some(state) = self.engine.get_latest_state() {
            Ok((
                ImuState {
                    timestamp: state.timestamp,
                    accel: AccelData {
                        x: state.accel.x,
                        y: state.accel.y,
                        z: state.accel.z,
                    },
                    gyro: GyroData {
                        x: state.gyro.x,
                        y: state.gyro.y,
                        z: state.gyro.z,
                    },
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
        if let Some(state) = self.engine.get_latest_state() {
            Ok((state.jerk_magnitude, state.timestamp))
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
        "BNO085".to_string()
    }

    fn start(&self) {
        // The BNO085 engine is started during wrapper construction in this implementation,
        // so start() is a no-op here.
    }

    fn save_state(&self) -> Result<(), CanonicalError> {
        self.engine
            .save_calibration_sync()
            .map_err(|e| CanonicalError {
                code: CanonicalErrorCode::Internal,
                message: e,
            })
    }
}
