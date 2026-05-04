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

fn convert_to_8bit_optimized(
    stride: usize,
    buf_data: &[u8],
    image_data: &mut [u8],
    width: usize,
    height: usize,
    is_10_bit: bool,
    is_12_bit: bool,
    is_packed: bool,
) {
    if !is_packed {
        panic!("Unpacked raw format not yet supported");
    }

    if is_10_bit {
        let is_contiguous = stride == (width * 5) / 4;

        if is_contiguous {
            let total_pixels = width * height;
            for (s, d) in buf_data[..total_pixels * 5 / 4]
                .chunks_exact(5)
                .zip(image_data[..total_pixels].chunks_exact_mut(4))
            {
                d[0] = s[0];
                d[1] = s[1];
                d[2] = s[2];
                d[3] = s[3];
            }
        } else {
            // Fallback to original if not contiguous
            for row in 0..height {
                let buf_row_start = row * stride;
                let buf_row_end = buf_row_start + width * 5 / 4;
                let pix_row_start = row * width;
                let pix_row_end = pix_row_start + width;
                for (buf_chunk, pix_chunk) in buf_data[buf_row_start..buf_row_end]
                    .chunks_exact(5)
                    .zip(image_data[pix_row_start..pix_row_end].chunks_exact_mut(4))
                {
                    pix_chunk[0] = buf_chunk[0];
                    pix_chunk[1] = buf_chunk[1];
                    pix_chunk[2] = buf_chunk[2];
                    pix_chunk[3] = buf_chunk[3];
                }
            }
        }
    } else {
        assert!(is_12_bit, "Expected 12-bit format");
        let is_contiguous = stride == (width * 3) / 2;

        if is_contiguous {
            let total_pixels = width * height;
            let src_chunks = buf_data[..total_pixels * 3 / 2].chunks_exact(3);
            let dst_chunks = image_data[..total_pixels].chunks_exact_mut(2);
            for (s, d) in src_chunks.zip(dst_chunks) {
                d[0] = s[0];
                d[1] = s[1];
            }
        } else {
            // Fallback: Original row-by-row
            for row in 0..height {
                let buf_row_start = row * stride;
                let buf_row_end = buf_row_start + width * 3 / 2;
                let pix_row_start = row * width;
                let pix_row_end = pix_row_start + width;

                let src_chunks = buf_data[buf_row_start..buf_row_end].chunks_exact(3);
                let dst_chunks = image_data[pix_row_start..pix_row_end].chunks_exact_mut(2);
                for (s, d) in src_chunks.zip(dst_chunks) {
                    d[0] = s[0];
                    d[1] = s[1];
                }
            }
        }
    }
}
fn main() {
    cedar_camera::rpi_camera::set_converter(convert_to_8bit_optimized);

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
            // 5: Gyro Mode (pure gyro integration)
            // 6: GyroHybrid Mode (9-axis Roll/Yaw + Gyro Pitch)
            let mode_val: u8 = pargs
                .opt_value_from_str(["-i", "--imu-rotation-mode"])
                .unwrap_or(None)
                .unwrap_or(1); // Default to 1 (Standard) if not provided

            let rotation_mode = match mode_val {
                1 => ImuRotationMode::Standard,
                2 => ImuRotationMode::Game,
                3 => ImuRotationMode::ArvrStabilized,
                4 => ImuRotationMode::ArvrStabilizedGame,
                5 => ImuRotationMode::Gyro,
                6 => ImuRotationMode::GyroHybrid,
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
