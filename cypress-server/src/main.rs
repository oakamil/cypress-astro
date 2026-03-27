use std::{path::Path, sync::Arc};

use cypress_imu::{
    bno085::{Bno085Imu, ImuRotationMode},
    cedar_bno085::CedarBno085Wrapper,
};
use cypress_solver::Tetra3Solver;
use pico_args::Arguments;
use tetra3::Solver;
use tokio::sync::Mutex;

use cedar_elements::{imu_trait::ImuTrait, solver_trait::SolverTrait};
use cedar_server::cedar_server::server_main;

fn main() {
    server_main(
        "Copyright (c) 2026 Steven Rosenthal smr@dt3.org.\n\
         Licensed for non-commercial use.\n\
         See LICENSE.md at https://github.com/smroid/cedar-server",
        /*flutter_app_path=*/ "../cedar/cedar-aim/cedar_flutter/build/web",
        /*get_dependencies=*/
        |mut pargs: Arguments| {
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

            let db_path = Path::new("../cedar/data/default_database.npz");
            let solver = Tetra3Solver::new(
                Solver::load_database(db_path).expect("Failed to load Tetra3 database"),
            );
            let solver_arc: Arc<Mutex<dyn SolverTrait + Send + Sync>> =
                Arc::new(Mutex::new(solver));

            println!("Initializing BNO085 IMU over I2C...");
            let imu: Option<Arc<Mutex<dyn ImuTrait + Send>>> = match Bno085Imu::start(rotation_mode)
            {
                Ok(imu) => {
                    println!("IMU successfully initialized!");
                    let cedar_imu = CedarBno085Wrapper { engine: imu };
                    Some(Arc::new(Mutex::new(cedar_imu)))
                }
                Err(_) => {
                    println!("Could not start BNO085 IMU");
                    None
                }
            };
            (None, None, imu, Some(solver_arc))
        },
    );
}
