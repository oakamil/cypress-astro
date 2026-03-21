use std::{path::Path, sync::Arc};

use cypress_imu::bno085::Bno085Imu;
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
        |_pargs: Arguments| {
            let db_path = Path::new("../cedar/data/default_database.npz");
            let solver = Tetra3Solver::new(
                Solver::load_database(db_path).expect("Failed to load Tetra3 database"),
            );
            let solver_arc: Arc<Mutex<dyn SolverTrait + Send + Sync>> =
                Arc::new(Mutex::new(solver));

            println!("Initializing BNO085 IMU over I2C...");
            let imu: Option<Arc<Mutex<dyn ImuTrait + Send>>> = match Bno085Imu::start() {
                Ok(imu) => {
                    println!("IMU successfully initialized!");
                    Some(Arc::new(Mutex::new(imu)))
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
