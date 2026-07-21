// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time;

use cedar_elements::imu_trait::{HorizonCoordinates, ImuTrait, TrackerState};
use cypress_imu::cedar_imu::CedarImuWrapper;
use env_logger;
use olive_imu::{Imu, bno085::Bno085Device};

use pico_args::Arguments;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut pargs = Arguments::from_env();
    let use_calibrated: bool = pargs.contains(["-c", "--calibrated"]);

    println!("Initializing gyro-only BNO085 over I2C...");

    let device = Bno085Device::new(10, 0x4B, use_calibrated)?;
    let imu_storage: Option<std::sync::Arc<dyn olive_imu::PersistentStorage>> =
        Some(std::sync::Arc::new(olive_imu::FileStorage::new(std::path::PathBuf::from("."))));

    let engine = Imu::start(device, imu_storage)?;
    let imu = CedarImuWrapper::new(Arc::new(engine));

    println!("Waiting for sensor calibration to complete (5 seconds of stability)...");

    // We block the plate solve until the IMU drops its 'Lost' state,
    // which guarantees both hardware initialization and gyro zero-bias are complete.
    while imu.get_tracker_state().await == TrackerState::Lost {
        time::sleep(Duration::from_millis(50)).await;
    }

    println!("IMU successfully initialized and gyro zero-bias calibrated!");

    // 1. Report the initial "True" Camera Pointing
    let initial_pointing = HorizonCoordinates {
        azimuth: 0.0,
        altitude: 0.0,
        zenith_roll_angle: 0.0,
    };

    println!("Reporting true camera pointing: {:?}", initial_pointing);
    imu.report_true_camera_pointing(&initial_pointing, &SystemTime::now())
        .await;

    let mut interval = time::interval(Duration::from_millis(100));

    println!("--------------------------------------------------");
    println!("Starting 100ms telemetry loop. Press Ctrl-C to exit.");
    println!("--------------------------------------------------");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nCtrl-C received. Shutting down gracefully...");
        }
        _ = async {
            loop {
                interval.tick().await;

                let now = SystemTime::now();

                match imu.get_estimated_camera_pointing(&now).await {
                    Ok(coords) => {
                        println!(
                            "Estimated Position -> Azimuth: {:>7.2}°, Altitude: {:>7.2}°, Roll: {:>7.2}°",
                            coords.azimuth,
                            coords.altitude,
                            coords.zenith_roll_angle
                        );
                    }
                    Err(e) => {
                        println!("IMU Error: {}", e.message);
                    }
                }
            }
        } => {}
    }

    Ok(())
}
