// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use std::time::{Duration, SystemTime};
use tokio::time;

use cedar_elements::imu_trait::{HorizonCoordinates, ImuTrait, TrackerState};
use cypress_imu::{
    bno085::{Bno085Imu, ImuRotationMode},
    cedar_bno085::CedarBno085Wrapper,
};
use env_logger;
use pico_args::Arguments;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Parse command line arguments
    let mut pargs = Arguments::from_env();

    // Parse the IMU rotation mode argument (-i or --imu-rotation-mode)
    // Available modes:
    // 1: Standard Rotation Mode (9-axis, default)
    // 2: Game Rotation Mode (6-axis, no compass)
    // 3: AR/VR Stabilized Rotation Mode (9-axis, stabilized)
    // 4: AR/VR Stabilized Game Rotation Mode (6-axis, stabilized)
    let mode_val: u8 = pargs
        .opt_value_from_str(["-i", "--imu-rotation-mode"])
        .unwrap_or(None)
        .unwrap_or(1); // Default to 1 (Standard) if not provided

    let rotation_mode = match mode_val {
        1 => ImuRotationMode::Standard,
        2 => ImuRotationMode::Game,
        3 => ImuRotationMode::ArvrStabilized,
        4 => ImuRotationMode::ArvrStabilizedGame,
        _ => {
            println!(
                "Invalid IMU rotation mode provided ({}). Defaulting to Standard (1).",
                mode_val
            );
            ImuRotationMode::Standard
        }
    };

    println!("Initializing BNO085 over I2C...");

    // Pass the parsed rotation mode instead of the hardcoded boolean
    let engine = Bno085Imu::start(rotation_mode)?;
    let imu = CedarBno085Wrapper { engine };

    println!("Waiting for sensor fusion algorithm to converge (3 seconds)...");

    // We block the plate solve until the IMU drops its 'Lost' state,
    // which guarantees both hardware initialization and gravity alignment are complete.
    while imu.get_tracker_state().await == TrackerState::Lost {
        time::sleep(Duration::from_millis(50)).await;
    }

    println!("IMU successfully initialized and gravity-aligned!");

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
