// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.
//
// Note: This implementation is intended for aarch64 only.

use std::{path::Path, sync::Arc};

use cypress_imu::cedar_imu::CedarImuWrapper;
use cypress_solver::Tetra3Solver;
use image::GrayImage;
use olive_imu::{Imu, bmi160::Bmi160Device, bno085::Bno085Device};
use pico_args::Arguments;
use tetra3::Solver;
use tokio::sync::Mutex;

use cedar_elements::{image_utils::ImageRotator, imu_trait::ImuTrait, solver_trait::SolverTrait};
use cedar_server::cedar_server::server_main;

use std::arch::aarch64::*;

unsafe fn unpack_neon(s_ptr: *const u8, d_ptr: *mut u8, total_pixels: usize) -> (usize, usize) {
    unsafe {
        let mask0 = vld1q_u8([0, 1, 2, 3, 5, 6, 7, 8, 10, 11, 12, 13, 15, 16, 17, 18].as_ptr());
        let mask1 = vld1q_u8([4, 5, 6, 7, 9, 10, 11, 12, 14, 15, 16, 17, 19, 20, 21, 22].as_ptr());
        let mask2 =
            vld1q_u8([8, 9, 10, 11, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26].as_ptr());
        let mask3 = vld1q_u8(
            [
                12, 13, 14, 15, 17, 18, 19, 20, 22, 23, 24, 25, 27, 28, 29, 30,
            ]
            .as_ptr(),
        );

        let mut in_x = 0;
        let mut out_x = 0;

        // Process 64 pixels (80 bytes input, 64 bytes output) per iteration
        let neon_iters = total_pixels / 64;
        for _ in 0..neon_iters {
            let v0 = vld1q_u8(s_ptr.add(in_x));
            let v1 = vld1q_u8(s_ptr.add(in_x + 16));
            let v2 = vld1q_u8(s_ptr.add(in_x + 32));
            let v3 = vld1q_u8(s_ptr.add(in_x + 48));
            let v4 = vld1q_u8(s_ptr.add(in_x + 64));

            let p0 = vqtbl2q_u8(uint8x16x2_t(v0, v1), mask0);
            let p1 = vqtbl2q_u8(uint8x16x2_t(v1, v2), mask1);
            let p2 = vqtbl2q_u8(uint8x16x2_t(v2, v3), mask2);
            let p3 = vqtbl2q_u8(uint8x16x2_t(v3, v4), mask3);

            vst1q_u8(d_ptr.add(out_x), p0);
            vst1q_u8(d_ptr.add(out_x + 16), p1);
            vst1q_u8(d_ptr.add(out_x + 32), p2);
            vst1q_u8(d_ptr.add(out_x + 48), p3);

            in_x += 80;
            out_x += 64;
        }
        (in_x, out_x)
    }
}

unsafe fn unpack_neon_12bit(
    s_ptr: *const u8,
    d_ptr: *mut u8,
    total_pixels: usize,
) -> (usize, usize) {
    unsafe {
        let mask0 = vld1q_u8([0, 1, 3, 4, 6, 7, 9, 10, 12, 13, 15, 16, 18, 19, 21, 22].as_ptr());
        let mask1 =
            vld1q_u8([8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 23, 24, 26, 27, 29, 30].as_ptr());

        let mut in_x = 0;
        let mut out_x = 0;

        // Process 32 pixels (48 bytes input, 32 bytes output) per iteration
        let neon_iters = total_pixels / 32;
        for _ in 0..neon_iters {
            let v0 = vld1q_u8(s_ptr.add(in_x));
            let v1 = vld1q_u8(s_ptr.add(in_x + 16));
            let v2 = vld1q_u8(s_ptr.add(in_x + 32));

            let p0 = vqtbl2q_u8(uint8x16x2_t(v0, v1), mask0);
            let p1 = vqtbl2q_u8(uint8x16x2_t(v1, v2), mask1);

            vst1q_u8(d_ptr.add(out_x), p0);
            vst1q_u8(d_ptr.add(out_x + 16), p1);

            in_x += 48;
            out_x += 32;
        }
        (in_x, out_x)
    }
}

fn convert_to_8bit_optimized(
    stride: usize,
    buf_data: &[u8],
    image_data: &mut [u8],
    binned_data: &mut [u8],
    width: usize,
    height: usize,
    is_10_bit: bool,
    is_12_bit: bool,
    is_packed: bool,
) {
    let out_width = width / 2;
    let out_height = height / 2;

    if !is_10_bit && !is_12_bit {
        let is_contiguous = stride == width;
        if is_contiguous {
            let total_pixels = width * height;
            image_data[..total_pixels].copy_from_slice(&buf_data[..total_pixels]);
        } else {
            for row in 0..height {
                let src_start = row * stride;
                let dst_start = row * width;
                image_data[dst_start..dst_start + width]
                    .copy_from_slice(&buf_data[src_start..src_start + width]);
            }
        }

        for out_y in 0..out_height {
            let row1_start = out_y * 2 * width;
            let row2_start = row1_start + width;
            let out_row_start = out_y * out_width;

            let s1_slice = &image_data[row1_start..row1_start + width];
            let s2_slice = &image_data[row2_start..row2_start + width];
            let d_slice = &mut binned_data[out_row_start..out_row_start + out_width];

            let mut in_x = 0;
            let mut out_x = 0;
            while in_x + 2 <= width && out_x < out_width {
                unsafe {
                    let v1 = std::ptr::read_unaligned(s1_slice.as_ptr().add(in_x) as *const u16);
                    let v2 = std::ptr::read_unaligned(s2_slice.as_ptr().add(in_x) as *const u16);
                    let sum1 = (v1 & 0xFF) + (v1 >> 8);
                    let sum2 = (v2 & 0xFF) + (v2 >> 8);
                    let total = sum1 + sum2;
                    *d_slice.as_mut_ptr().add(out_x) = (total >> 2) as u8;
                }
                in_x += 2;
                out_x += 1;
            }
        }
        return;
    }

    if !is_packed {
        panic!("Unpacked raw format not yet supported");
    }

    if is_10_bit {
        let is_contiguous = stride == (width * 5) / 4;

        if is_contiguous {
            let total_pixels = width * height;
            let mut in_x = 0;
            let mut out_x = 0;
            let s_ptr = buf_data.as_ptr();
            let d_ptr = image_data.as_mut_ptr();

            unsafe {
                let (neon_in, neon_out) = unpack_neon(s_ptr, d_ptr, total_pixels);
                in_x += neon_in;
                out_x += neon_out;
            }

            for (s, d) in buf_data[in_x..total_pixels * 5 / 4]
                .chunks_exact(5)
                .zip(image_data[out_x..total_pixels].chunks_exact_mut(4))
            {
                d[0] = s[0];
                d[1] = s[1];
                d[2] = s[2];
                d[3] = s[3];
            }
        } else {
            for row in 0..height {
                let buf_row_start = row * stride;
                let buf_row_end = buf_row_start + width * 5 / 4;
                let pix_row_start = row * width;
                let pix_row_end = pix_row_start + width;

                let mut in_x = 0;
                let mut out_x = 0;
                let s_ptr = buf_data[buf_row_start..buf_row_end].as_ptr();
                let d_ptr = image_data[pix_row_start..pix_row_end].as_mut_ptr();

                unsafe {
                    let (neon_in, neon_out) = unpack_neon(s_ptr, d_ptr, width);
                    in_x += neon_in;
                    out_x += neon_out;
                }

                for (buf_chunk, pix_chunk) in buf_data[buf_row_start + in_x..buf_row_end]
                    .chunks_exact(5)
                    .zip(image_data[pix_row_start + out_x..pix_row_end].chunks_exact_mut(4))
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
            let mut in_x = 0;
            let mut out_x = 0;
            let s_ptr = buf_data.as_ptr();
            let d_ptr = image_data.as_mut_ptr();

            unsafe {
                let (neon_in, neon_out) = unpack_neon_12bit(s_ptr, d_ptr, total_pixels);
                in_x += neon_in;
                out_x += neon_out;
            }

            let src_chunks = buf_data[in_x..total_pixels * 3 / 2].chunks_exact(3);
            let dst_chunks = image_data[out_x..total_pixels].chunks_exact_mut(2);
            for (s, d) in src_chunks.zip(dst_chunks) {
                d[0] = s[0];
                d[1] = s[1];
            }
        } else {
            for row in 0..height {
                let buf_row_start = row * stride;
                let buf_row_end = buf_row_start + width * 3 / 2;
                let pix_row_start = row * width;
                let pix_row_end = pix_row_start + width;

                let mut in_x = 0;
                let mut out_x = 0;
                let s_ptr = buf_data[buf_row_start..buf_row_end].as_ptr();
                let d_ptr = image_data[pix_row_start..pix_row_end].as_mut_ptr();

                unsafe {
                    let (neon_in, neon_out) = unpack_neon_12bit(s_ptr, d_ptr, width);
                    in_x += neon_in;
                    out_x += neon_out;
                }

                let src_chunks = buf_data[buf_row_start + in_x..buf_row_end].chunks_exact(3);
                let dst_chunks = image_data[pix_row_start + out_x..pix_row_end].chunks_exact_mut(2);
                for (s, d) in src_chunks.zip(dst_chunks) {
                    d[0] = s[0];
                    d[1] = s[1];
                }
            }
        }
    }

    for out_y in 0..out_height {
        let row1_start = out_y * 2 * width;
        let row2_start = row1_start + width;
        let out_row_start = out_y * out_width;

        let s1_slice = &image_data[row1_start..row1_start + width];
        let s2_slice = &image_data[row2_start..row2_start + width];
        let d_slice = &mut binned_data[out_row_start..out_row_start + out_width];

        let mut in_x = 0;
        let mut out_x = 0;
        while in_x + 2 <= width && out_x < out_width {
            unsafe {
                let v1 = std::ptr::read_unaligned(s1_slice.as_ptr().add(in_x) as *const u16);
                let v2 = std::ptr::read_unaligned(s2_slice.as_ptr().add(in_x) as *const u16);
                let sum1 = (v1 & 0xFF) + (v1 >> 8);
                let sum2 = (v2 & 0xFF) + (v2 >> 8);
                let total = sum1 + sum2;
                *d_slice.as_mut_ptr().add(out_x) = (total >> 2) as u8;
            }
            in_x += 2;
            out_x += 1;
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
    assert!(
        w >= h,
        "rotate_image_and_crop requires width >= height, got {}x{}",
        w,
        h
    );
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
        |_pargs: Arguments| {
            let db_path = Path::new("../cedar/data/default_database.npz");
            let solver = Tetra3Solver::new(
                Solver::load_database(db_path).expect("Failed to load Tetra3 database"),
            );
            let solver_arc: Arc<Mutex<dyn SolverTrait + Send + Sync>> =
                Arc::new(Mutex::new(solver));

            println!("Probing I2C bus for IMU sensors...");

            let try_init = |name: &str,
                            result: Option<Arc<Mutex<dyn ImuTrait + Send>>>|
             -> Option<Arc<Mutex<dyn ImuTrait + Send>>> {
                if result.is_some() {
                    println!("{} successfully initialized!", name);
                }
                result
            };

            let imu: Option<Arc<Mutex<dyn ImuTrait + Send>>> = None
                .or_else(|| {
                    Bno085Device::new(10, 0x4B)
                        .ok()
                        .and_then(|device| {
                            Imu::start(device, None).ok().map(|engine| {
                                let wrapper = CedarImuWrapper::new(Arc::new(engine));

                                Arc::new(Mutex::new(wrapper)) as Arc<Mutex<dyn ImuTrait + Send>>
                            })
                        })
                        .and_then(|r| try_init("BNO085 (0x4B)", Some(r)))
                })
                .or_else(|| {
                    Bno085Device::new(10, 0x4A)
                        .ok()
                        .and_then(|device| {
                            Imu::start(device, None).ok().map(|engine| {
                                let wrapper = CedarImuWrapper::new(Arc::new(engine));

                                Arc::new(Mutex::new(wrapper)) as Arc<Mutex<dyn ImuTrait + Send>>
                            })
                        })
                        .and_then(|r| try_init("BNO085 (0x4A)", Some(r)))
                })
                .or_else(|| {
                    Bmi160Device::new(0x68)
                        .ok()
                        .and_then(|device| {
                            Imu::start(device, None).ok().map(|engine| {
                                let wrapper = CedarImuWrapper::new(Arc::new(engine));

                                Arc::new(Mutex::new(wrapper)) as Arc<Mutex<dyn ImuTrait + Send>>
                            })
                        })
                        .and_then(|r| try_init("BMI160 (0x68)", Some(r)))
                })
                .or_else(|| {
                    Bmi160Device::new(0x69)
                        .ok()
                        .and_then(|device| {
                            Imu::start(device, None).ok().map(|engine| {
                                let wrapper = CedarImuWrapper::new(Arc::new(engine));

                                Arc::new(Mutex::new(wrapper)) as Arc<Mutex<dyn ImuTrait + Send>>
                            })
                        })
                        .and_then(|r| try_init("BMI160 (0x69)", Some(r)))
                });

            if imu.is_none() {
                println!("No IMU sensor found on standard I2C addresses. Running without IMU.");
            }
            (None, None, imu, None, Some(solver_arc))
        },
        Some(2),
    );
}
