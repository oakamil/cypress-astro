use std::{path::Path, sync::Arc};

use cypress_imu::{
    bno085::{Bno085Imu, ImuRotationMode},
    cedar_bno085::CedarBno085Wrapper,
};
use cypress_solver::Tetra3Solver;
use pico_args::Arguments;
use tetra3::Solver;
use tokio::sync::Mutex;
use image::GrayImage;

use cedar_elements::{
    image_utils::ImageRotator,
    imu_trait::ImuTrait, 
    solver_trait::SolverTrait
};
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
    _do_bin_2x2: bool,
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

/// A highly optimized custom rotation function designed for constrained CPUs
/// like the Cortex-A53 (Raspberry Pi Zero 2W).
/// 
/// Algorithmic improvements:
/// 1. Loop Fusion: We skip allocating a full-width rotated intermediate image
///    and skip the secondary crop pass. We only iterate exactly over the pixels
///    that will end up in the final square crop.
/// 2. Fixed-Point Math: Floating-point math is expensive. We precompute the
///    step sizes for rays across the destination image and use 16.16 fixed-point
///    arithmetic (integers) to traverse the source image.
/// 3. Nearest Neighbor Interpolation: Uses pure single-pixel fetch to avoid 
///    any multiplications and fractional calculations.
fn custom_rotate_image_and_crop(image: &GrayImage, rotator: &ImageRotator) -> GrayImage {
    let (w, h) = image.dimensions();
    assert!(w >= h, "rotate_image_and_crop requires width >= height, got {}x{}", w, h);
    let square_size = h;

    let mut output = GrayImage::new(square_size, square_size);
    let out_buf = output.as_mut();
    let in_buf = image.as_raw();
    let in_w = w as i32;
    let in_h = h as i32;

    // 16.16 fixed point multiplier
    let f_scale = 65536.0;

    // Calculate how much the source coordinate changes when we step 1 pixel in destination X or Y
    let dx_src_x = (rotator.cos_term * f_scale) as i32;
    let dx_src_y = (rotator.sin_term * f_scale) as i32;
    let dy_src_x = (-rotator.sin_term * f_scale) as i32;
    let dy_src_y = (rotator.cos_term * f_scale) as i32;

    // Find the source coordinate that corresponds to the top-left (0,0) of the output image.
    // The default `transform_from_rotated` gives us the exact floating point starting coordinate.
    let (start_src_x_f, start_src_y_f) = rotator.transform_from_rotated(0.0, 0.0, w, h);
    
    let mut row_src_x = (start_src_x_f * f_scale) as i32;
    let mut row_src_y = (start_src_y_f * f_scale) as i32;

    let mut out_idx = 0;

    // NEAREST NEIGHBOR
    for _y in 0..square_size {
        let mut src_x = row_src_x;
        let mut src_y = row_src_y;

        for _x in 0..square_size {
            // Round to nearest integer (+0.5 in fixed point is +32768)
            let px = (src_x + 32768) >> 16;
            let py = (src_y + 32768) >> 16;

            let blended = if px >= 0 && px < in_w && py >= 0 && py < in_h {
                // Safe fetch without internal bounds checks
                unsafe { *in_buf.get_unchecked((py * in_w + px) as usize) }
            } else {
                0 // Default black outside image bounds
            };

            out_buf[out_idx] = blended;
            out_idx += 1;

            // Step forward in X direction
            src_x += dx_src_x;
            src_y += dx_src_y;
        }
        
        // Step forward in Y direction (for the next row)
        row_src_x += dy_src_x;
        row_src_y += dy_src_y;
    }

    output
}

fn main() {
    cedar_camera::rpi_camera::set_converter(convert_to_8bit_optimized);
    cedar_elements::image_utils::set_rotate_crop_fn(custom_rotate_image_and_crop);

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
            (None, None, imu, None, Some(solver_arc))
        },
    );
}
