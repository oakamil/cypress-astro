// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.
//
// Note: This implementation is intended for aarch64 only.

use std::{path::Path, sync::Arc};

use cypress_imu::{
    bno085::{Bno085Imu, ImuRotationMode},
    cedar_bno085::CedarBno085Wrapper,
};
use cypress_solver::Tetra3Solver;
use image::GrayImage;
use pico_args::Arguments;
use tetra3::Solver;
use tokio::sync::Mutex;

use cedar_elements::{image_utils::ImageRotator, imu_trait::ImuTrait, solver_trait::SolverTrait};
use cedar_server::cedar_server::server_main;

use std::arch::aarch64::*;

unsafe fn bin_neon(
    s1_ptr: *const u8,
    s2_ptr: *const u8,
    d_ptr: *mut u8,
    width: usize,
) -> (usize, usize) {
    unsafe {
    let mask0 = vld1q_u8([0, 1, 2, 3, 5, 6, 7, 8, 10, 11, 12, 13, 15, 16, 17, 18].as_ptr());
    let mask1 = vld1q_u8([4, 5, 6, 7, 9, 10, 11, 12, 14, 15, 16, 17, 19, 20, 21, 22].as_ptr());
    let mask2 = vld1q_u8([8, 9, 10, 11, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26].as_ptr());
    let mask3 = vld1q_u8(
        [
            12, 13, 14, 15, 17, 18, 19, 20, 22, 23, 24, 25, 27, 28, 29, 30,
        ]
        .as_ptr(),
    );

    let mut in_x = 0;
    let mut out_x = 0;

    // Process 64 pixels (80 bytes) per iteration
    let neon_iters = width / 64;
    for _ in 0..neon_iters {
        let r1_v0 = vld1q_u8(s1_ptr.add(in_x));
        let r1_v1 = vld1q_u8(s1_ptr.add(in_x + 16));
        let r1_v2 = vld1q_u8(s1_ptr.add(in_x + 32));
        let r1_v3 = vld1q_u8(s1_ptr.add(in_x + 48));
        let r1_v4 = vld1q_u8(s1_ptr.add(in_x + 64));

        let r2_v0 = vld1q_u8(s2_ptr.add(in_x));
        let r2_v1 = vld1q_u8(s2_ptr.add(in_x + 16));
        let r2_v2 = vld1q_u8(s2_ptr.add(in_x + 32));
        let r2_v3 = vld1q_u8(s2_ptr.add(in_x + 48));
        let r2_v4 = vld1q_u8(s2_ptr.add(in_x + 64));

        let r1_p0 = vqtbl2q_u8(uint8x16x2_t(r1_v0, r1_v1), mask0);
        let r2_p0 = vqtbl2q_u8(uint8x16x2_t(r2_v0, r2_v1), mask0);

        let r1_p1 = vqtbl2q_u8(uint8x16x2_t(r1_v1, r1_v2), mask1);
        let r2_p1 = vqtbl2q_u8(uint8x16x2_t(r2_v1, r2_v2), mask1);

        let r1_p2 = vqtbl2q_u8(uint8x16x2_t(r1_v2, r1_v3), mask2);
        let r2_p2 = vqtbl2q_u8(uint8x16x2_t(r2_v2, r2_v3), mask2);

        let r1_p3 = vqtbl2q_u8(uint8x16x2_t(r1_v3, r1_v4), mask3);
        let r2_p3 = vqtbl2q_u8(uint8x16x2_t(r2_v3, r2_v4), mask3);

        let sum_p0 = vaddq_u16(vpaddlq_u8(r1_p0), vpaddlq_u8(r2_p0));
        let out_p0 = vmovn_u16(vshrq_n_u16::<2>(sum_p0));
        vst1_u8(d_ptr.add(out_x), out_p0);

        let sum_p1 = vaddq_u16(vpaddlq_u8(r1_p1), vpaddlq_u8(r2_p1));
        let out_p1 = vmovn_u16(vshrq_n_u16::<2>(sum_p1));
        vst1_u8(d_ptr.add(out_x + 8), out_p1);

        let sum_p2 = vaddq_u16(vpaddlq_u8(r1_p2), vpaddlq_u8(r2_p2));
        let out_p2 = vmovn_u16(vshrq_n_u16::<2>(sum_p2));
        vst1_u8(d_ptr.add(out_x + 16), out_p2);

        let sum_p3 = vaddq_u16(vpaddlq_u8(r1_p3), vpaddlq_u8(r2_p3));
        let out_p3 = vmovn_u16(vshrq_n_u16::<2>(sum_p3));
        vst1_u8(d_ptr.add(out_x + 24), out_p3);

        in_x += 80;
        out_x += 32;
    }
    (in_x, out_x)
    }
}

unsafe fn bin_neon_12bit(
    s1_ptr: *const u8,
    s2_ptr: *const u8,
    d_ptr: *mut u8,
    width: usize,
) -> (usize, usize) {
    unsafe {
    let mask0 = vld1q_u8([0, 1, 3, 4, 6, 7, 9, 10, 12, 13, 15, 16, 18, 19, 21, 22].as_ptr());
    let mask1 = vld1q_u8([8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 23, 24, 26, 27, 29, 30].as_ptr());

    let mut in_x = 0;
    let mut out_x = 0;

    // Process 32 pixels per row (48 bytes per row, 16 bytes output)
    let neon_iters = width / 32;
    for _ in 0..neon_iters {
        let r1_v0 = vld1q_u8(s1_ptr.add(in_x));
        let r1_v1 = vld1q_u8(s1_ptr.add(in_x + 16));
        let r1_v2 = vld1q_u8(s1_ptr.add(in_x + 32));

        let r2_v0 = vld1q_u8(s2_ptr.add(in_x));
        let r2_v1 = vld1q_u8(s2_ptr.add(in_x + 16));
        let r2_v2 = vld1q_u8(s2_ptr.add(in_x + 32));

        let r1_p0 = vqtbl2q_u8(uint8x16x2_t(r1_v0, r1_v1), mask0);
        let r1_p1 = vqtbl2q_u8(uint8x16x2_t(r1_v1, r1_v2), mask1);

        let r2_p0 = vqtbl2q_u8(uint8x16x2_t(r2_v0, r2_v1), mask0);
        let r2_p1 = vqtbl2q_u8(uint8x16x2_t(r2_v1, r2_v2), mask1);

        let sum_p0 = vaddq_u16(vpaddlq_u8(r1_p0), vpaddlq_u8(r2_p0));
        let out_p0 = vmovn_u16(vshrq_n_u16::<2>(sum_p0));
        vst1_u8(d_ptr.add(out_x), out_p0);

        let sum_p1 = vaddq_u16(vpaddlq_u8(r1_p1), vpaddlq_u8(r2_p1));
        let out_p1 = vmovn_u16(vshrq_n_u16::<2>(sum_p1));
        vst1_u8(d_ptr.add(out_x + 8), out_p1);

        in_x += 48;
        out_x += 16;
    }
    (in_x, out_x)
    }
}

unsafe fn unpack_neon(s_ptr: *const u8, d_ptr: *mut u8, total_pixels: usize) -> (usize, usize) {
    unsafe {
    let mask0 = vld1q_u8([0, 1, 2, 3, 5, 6, 7, 8, 10, 11, 12, 13, 15, 16, 17, 18].as_ptr());
    let mask1 = vld1q_u8([4, 5, 6, 7, 9, 10, 11, 12, 14, 15, 16, 17, 19, 20, 21, 22].as_ptr());
    let mask2 = vld1q_u8([8, 9, 10, 11, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26].as_ptr());
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
    let mask1 = vld1q_u8([8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 23, 24, 26, 27, 29, 30].as_ptr());

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
    width: usize,
    height: usize,
    is_10_bit: bool,
    is_12_bit: bool,
    is_packed: bool,
    do_bin_2x2: bool,
) {
    if do_bin_2x2 {
        let out_width = width / 2;
        let out_height = height / 2;

        if !is_10_bit && !is_12_bit {
            for out_y in 0..out_height {
                let row1_start = out_y * 2 * stride;
                let row2_start = row1_start + stride;
                let out_row_start = out_y * out_width;

                let s1_slice = &buf_data[row1_start..row1_start + width];
                let s2_slice = &buf_data[row2_start..row2_start + width];
                let d_slice = &mut image_data[out_row_start..out_row_start + out_width];

                let mut in_x = 0;
                let mut out_x = 0;
                while in_x + 2 <= width && out_x < out_width {
                    unsafe {
                        let v1 =
                            std::ptr::read_unaligned(s1_slice.as_ptr().add(in_x) as *const u16);
                        let v2 =
                            std::ptr::read_unaligned(s2_slice.as_ptr().add(in_x) as *const u16);
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
            for out_y in 0..out_height {
                let row1_start = out_y * 2 * stride;
                let row2_start = row1_start + stride;
                let out_row_start = out_y * out_width;

                let s1_slice = &buf_data[row1_start..row1_start + width * 5 / 4];
                let s2_slice = &buf_data[row2_start..row2_start + width * 5 / 4];
                let d_slice = &mut image_data[out_row_start..out_row_start + out_width];

                let mut in_x = 0;
                let mut out_x = 0;
                let s1_ptr = s1_slice.as_ptr();
                let s2_ptr = s2_slice.as_ptr();
                let d_ptr = d_slice.as_mut_ptr();

                let (neon_in, neon_out) = unsafe { bin_neon(s1_ptr, s2_ptr, d_ptr, width) };
                in_x += neon_in;
                out_x += neon_out;

                // Handle remaining pixels
                while in_x + 4 <= width * 5 / 4 && out_x < out_width {
                    unsafe {
                        let mut sum0 = *s1_ptr.add(in_x) as u16 + *s1_ptr.add(in_x + 1) as u16;
                        sum0 += *s2_ptr.add(in_x) as u16 + *s2_ptr.add(in_x + 1) as u16;
                        *d_ptr.add(out_x) = (sum0 >> 2) as u8;
                        out_x += 1;

                        if out_x < out_width {
                            let mut sum1 =
                                *s1_ptr.add(in_x + 2) as u16 + *s1_ptr.add(in_x + 3) as u16;
                            sum1 += *s2_ptr.add(in_x + 2) as u16 + *s2_ptr.add(in_x + 3) as u16;
                            *d_ptr.add(out_x) = (sum1 >> 2) as u8;
                            out_x += 1;
                        }
                    }
                    in_x += 5;
                }
            }
        } else {
            assert!(is_12_bit, "Expected 12-bit format");
            for out_y in 0..out_height {
                let row1_start = out_y * 2 * stride;
                let row2_start = row1_start + stride;
                let out_row_start = out_y * out_width;

                let s1_slice = &buf_data[row1_start..row1_start + width * 3 / 2];
                let s2_slice = &buf_data[row2_start..row2_start + width * 3 / 2];
                let d_slice = &mut image_data[out_row_start..out_row_start + out_width];

                let mut in_x = 0;
                let mut out_x = 0;
                let s1_ptr = s1_slice.as_ptr();
                let s2_ptr = s2_slice.as_ptr();
                let d_ptr = d_slice.as_mut_ptr();

                let (neon_in, neon_out) = unsafe { bin_neon_12bit(s1_ptr, s2_ptr, d_ptr, width) };
                in_x += neon_in;
                out_x += neon_out;

                while in_x + 2 <= width * 3 / 2 && out_x < out_width {
                    unsafe {
                        let v1 = std::ptr::read_unaligned(s1_ptr.add(in_x) as *const u16);
                        let v2 = std::ptr::read_unaligned(s2_ptr.add(in_x) as *const u16);
                        let sum1 = (v1 & 0xFF) + (v1 >> 8);
                        let sum2 = (v2 & 0xFF) + (v2 >> 8);
                        let total = sum1 + sum2;
                        *d_ptr.add(out_x) = (total >> 2) as u8;
                    }
                    in_x += 3;
                    out_x += 1;
                }
            }
        }
        return;
    }

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
            // Fallback to original if not contiguous
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
            // Fallback: Original row-by-row
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
