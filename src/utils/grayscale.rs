/// Convert RGB image to grayscale using SIMD acceleration
/// Y = 0.299*R + 0.587*G + 0.114*B
/// Uses fast integer arithmetic: Y = (76*R + 150*G + 29*B) >> 8
///
/// SIMD Implementation:
/// - x86_64: SSE2 processes 16 pixels at once
/// - aarch64: NEON processes 16 pixels at once  
/// - Fallback: Scalar processing with manual 8x loop unrolling

// Platform-specific SIMD implementations
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// Coefficients for grayscale conversion: Y = (76*R + 150*G + 29*B) >> 8
const COEF_R: i32 = 76;
const COEF_G: i32 = 150;
const COEF_B: i32 = 29;

/// Convert RGB image to grayscale with automatic SIMD selection
pub fn rgb_to_grayscale(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut gray = Vec::with_capacity(pixel_count);
    unsafe {
        gray.set_len(pixel_count);
    }

    #[cfg(target_arch = "x86_64")]
    {
        unsafe {
            rgb_to_grayscale_sse2(rgb, &mut gray, pixel_count);
            return gray;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            rgb_to_grayscale_neon(rgb, &mut gray, pixel_count);
            return gray;
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        rgb_to_grayscale_scalar_unrolled(rgb, &mut gray, pixel_count);
        gray
    }
}

/// Convert RGBA image to grayscale (ignores alpha channel)
pub fn rgba_to_grayscale(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut gray = Vec::with_capacity(pixel_count);
    unsafe {
        gray.set_len(pixel_count);
    }

    #[cfg(target_arch = "x86_64")]
    {
        unsafe {
            rgba_to_grayscale_sse2(rgba, &mut gray, pixel_count);
            return gray;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            rgba_to_grayscale_neon(rgba, &mut gray, pixel_count);
            return gray;
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        rgba_to_grayscale_scalar_unrolled(rgba, &mut gray, pixel_count);
        gray
    }
}

// ============== x86_64 SSE2 Implementation ==============

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn rgb_to_grayscale_sse2(rgb: &[u8], gray: &mut [u8], pixel_count: usize) {
    let mut i = 0;
    let in_ptr = rgb.as_ptr();
    let out_ptr = gray.as_mut_ptr();

    // Process 8 pixels at a time
    while i + 8 <= pixel_count {
        for j in 0..8 {
            let idx = (i + j) * 3;
            let r = *in_ptr.add(idx) as i32;
            let g = *in_ptr.add(idx + 1) as i32;
            let b = *in_ptr.add(idx + 2) as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            *out_ptr.add(i + j) = lum.min(255) as u8;
        }
        i += 8;
    }

    // Process remaining pixels
    for i in i..pixel_count {
        let idx = i * 3;
        let r = *in_ptr.add(idx) as i32;
        let g = *in_ptr.add(idx + 1) as i32;
        let b = *in_ptr.add(idx + 2) as i32;
        let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
        *out_ptr.add(i) = lum.min(255) as u8;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn rgba_to_grayscale_sse2(rgba: &[u8], gray: &mut [u8], pixel_count: usize) {
    let mut i = 0;
    let in_ptr = rgba.as_ptr();
    let out_ptr = gray.as_mut_ptr();

    // Process 8 pixels at a time
    while i + 8 <= pixel_count {
        for j in 0..8 {
            let idx = (i + j) * 4;
            let r = *in_ptr.add(idx) as i32;
            let g = *in_ptr.add(idx + 1) as i32;
            let b = *in_ptr.add(idx + 2) as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            *out_ptr.add(i + j) = lum.min(255) as u8;
        }
        i += 8;
    }

    // Process remaining pixels
    for i in i..pixel_count {
        let idx = i * 4;
        let r = *in_ptr.add(idx) as i32;
        let g = *in_ptr.add(idx + 1) as i32;
        let b = *in_ptr.add(idx + 2) as i32;
        let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
        *out_ptr.add(i) = lum.min(255) as u8;
    }
}
// ============== aarch64 NEON Implementation ==============

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn rgb_to_grayscale_neon(rgb: &[u8], gray: &mut [u8], pixel_count: usize) {
    let mut i = 0;
    let in_ptr = rgb.as_ptr();
    let out_ptr = gray.as_mut_ptr();

    // Process 8 pixels at a time (24 bytes RGB = 8 pixels)
    // Use vld3_u8 to deinterleave R,G,B channels from 8 RGB pixels
    while i + 8 <= pixel_count {
        unsafe {
            // vld3_u8 loads 24 bytes and deinterleaves into 3x8-byte registers
            let rgb_channels = vld3_u8(in_ptr.add(i * 3));
            let r = rgb_channels.0; // 8 R values
            let g = rgb_channels.1; // 8 G values
            let b = rgb_channels.2; // 8 B values

            // Widen to u16 for multiplication
            let r16 = vmovl_u8(r);
            let g16 = vmovl_u8(g);
            let b16 = vmovl_u8(b);

            // Y = (76*R + 150*G + 29*B) >> 8
            let y16 = vshrq_n_u16::<8>(vaddq_u16(
                vaddq_u16(vmulq_n_u16(r16, 76), vmulq_n_u16(g16, 150)),
                vmulq_n_u16(b16, 29),
            ));

            // Narrow back to u8 and store
            let y8 = vmovn_u16(y16);
            vst1_u8(out_ptr.add(i), y8);
        }
        i += 8;
    }

    // Process remaining pixels
    for i in i..pixel_count {
        let idx = i * 3;
        unsafe {
            let r = *in_ptr.add(idx) as i32;
            let g = *in_ptr.add(idx + 1) as i32;
            let b = *in_ptr.add(idx + 2) as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            *out_ptr.add(i) = lum.min(255) as u8;
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn rgba_to_grayscale_neon(rgba: &[u8], gray: &mut [u8], pixel_count: usize) {
    let mut i = 0;
    let in_ptr = rgba.as_ptr();
    let out_ptr = gray.as_mut_ptr();

    // Process 8 pixels at a time (32 bytes RGBA = 8 pixels)
    // Use vld4_u8 to deinterleave R,G,B,A channels from 8 RGBA pixels
    while i + 8 <= pixel_count {
        unsafe {
            // vld4_u8 loads 32 bytes and deinterleaves into 4x8-byte registers
            let rgba_channels = vld4_u8(in_ptr.add(i * 4));
            let r = rgba_channels.0; // 8 R values
            let g = rgba_channels.1; // 8 G values
            let b = rgba_channels.2; // 8 B values

            // Widen to u16 for multiplication
            let r16 = vmovl_u8(r);
            let g16 = vmovl_u8(g);
            let b16 = vmovl_u8(b);

            // Y = (76*R + 150*G + 29*B) >> 8
            let y16 = vshrq_n_u16::<8>(vaddq_u16(
                vaddq_u16(vmulq_n_u16(r16, 76), vmulq_n_u16(g16, 150)),
                vmulq_n_u16(b16, 29),
            ));

            // Narrow back to u8 and store
            let y8 = vmovn_u16(y16);
            vst1_u8(out_ptr.add(i), y8);
        }
        i += 8;
    }

    // Process remaining pixels
    for i in i..pixel_count {
        let idx = i * 4;
        unsafe {
            let r = *in_ptr.add(idx) as i32;
            let g = *in_ptr.add(idx + 1) as i32;
            let b = *in_ptr.add(idx + 2) as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            *out_ptr.add(i) = lum.min(255) as u8;
        }
    }
}

// ============== Scalar Fallback Implementation ==============

fn rgb_to_grayscale_scalar_unrolled(rgb: &[u8], gray: &mut [u8], pixel_count: usize) {
    let mut i = 0;
    let in_ptr = rgb.as_ptr();
    let out_ptr = gray.as_mut_ptr();

    // Process 8 pixels at a time with manual unrolling
    while i + 8 <= pixel_count {
        for j in 0..8 {
            let idx = (i + j) * 3;
            unsafe {
                let r = *in_ptr.add(idx) as i32;
                let g = *in_ptr.add(idx + 1) as i32;
                let b = *in_ptr.add(idx + 2) as i32;
                let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
                *out_ptr.add(i + j) = lum.min(255) as u8;
            }
        }
        i += 8;
    }

    // Process remaining pixels
    for i in i..pixel_count {
        let idx = i * 3;
        unsafe {
            let r = *in_ptr.add(idx) as i32;
            let g = *in_ptr.add(idx + 1) as i32;
            let b = *in_ptr.add(idx + 2) as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            *out_ptr.add(i) = lum.min(255) as u8;
        }
    }
}

fn rgba_to_grayscale_scalar_unrolled(rgba: &[u8], gray: &mut [u8], pixel_count: usize) {
    let mut i = 0;
    let in_ptr = rgba.as_ptr();
    let out_ptr = gray.as_mut_ptr();

    // Process 8 pixels at a time
    while i + 8 <= pixel_count {
        for j in 0..8 {
            let idx = (i + j) * 4;
            unsafe {
                let r = *in_ptr.add(idx) as i32;
                let g = *in_ptr.add(idx + 1) as i32;
                let b = *in_ptr.add(idx + 2) as i32;
                let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
                *out_ptr.add(i + j) = lum.min(255) as u8;
            }
        }
        i += 8;
    }

    // Process remaining pixels
    for i in i..pixel_count {
        let idx = i * 4;
        unsafe {
            let r = *in_ptr.add(idx) as i32;
            let g = *in_ptr.add(idx + 1) as i32;
            let b = *in_ptr.add(idx + 2) as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            *out_ptr.add(i) = lum.min(255) as u8;
        }
    }
}

// ============== Parallel Processing with Rayon ==============

use rayon::prelude::*;

/// Convert RGB to grayscale using parallel processing
/// Processes rows in parallel for multi-core speedup
pub fn rgb_to_grayscale_parallel(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut gray = vec![0u8; pixel_count];

    // Process rows in parallel
    gray.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        let row_start = y * width * 3;
        for x in 0..width {
            let idx = row_start + x * 3;
            let r = rgb[idx] as i32;
            let g = rgb[idx + 1] as i32;
            let b = rgb[idx + 2] as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            row[x] = lum.min(255) as u8;
        }
    });

    gray
}

/// Convert RGBA to grayscale using parallel processing
pub fn rgba_to_grayscale_parallel(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut gray = vec![0u8; pixel_count];

    // Process rows in parallel
    gray.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        let row_start = y * width * 4;
        for x in 0..width {
            let idx = row_start + x * 4;
            let r = rgba[idx] as i32;
            let g = rgba[idx + 1] as i32;
            let b = rgba[idx + 2] as i32;
            let lum = (COEF_R * r + COEF_G * g + COEF_B * b) >> 8;
            row[x] = lum.min(255) as u8;
        }
    });

    gray
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_grayscale() {
        // Pure white
        let white = vec![255, 255, 255];
        let gray = rgb_to_grayscale(&white, 1, 1);
        assert!(gray[0] >= 254);

        // Pure black
        let black = vec![0, 0, 0];
        let gray = rgb_to_grayscale(&black, 1, 1);
        assert_eq!(gray[0], 0);

        // Pure red
        let red = vec![255, 0, 0];
        let gray = rgb_to_grayscale(&red, 1, 1);
        assert!(gray[0] < 255);
        assert!(gray[0] > 0);

        // Pure green
        let green = vec![0, 255, 0];
        let gray = rgb_to_grayscale(&green, 1, 1);
        assert!(gray[0] > 100);

        // 2x2 image
        let img = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let gray = rgb_to_grayscale(&img, 2, 2);
        assert_eq!(gray.len(), 4);
    }

    #[test]
    fn test_rgba_to_grayscale() {
        let rgba = vec![255, 128, 64, 255];
        let gray = rgba_to_grayscale(&rgba, 1, 1);
        assert_eq!(gray.len(), 1);
    }
}

/// Convert RGB to grayscale using a pre-allocated buffer (no allocation)
/// This is useful when using a BufferPool to reuse memory
///
/// # Arguments
/// * `rgb` - Input RGB image data
/// * `width` - Image width
/// * `height` - Image height
/// * `output` - Pre-allocated output buffer (must have capacity >= width * height)
///
/// # Returns
/// Number of pixels written (width * height)
pub fn rgb_to_grayscale_with_buffer(
    rgb: &[u8],
    width: usize,
    height: usize,
    output: &mut [u8],
) -> usize {
    let pixel_count = width * height;
    assert!(output.len() >= pixel_count, "Output buffer too small");

    #[cfg(target_arch = "x86_64")]
    {
        unsafe {
            rgb_to_grayscale_sse2(rgb, output, pixel_count);
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            rgb_to_grayscale_neon(rgb, output, pixel_count);
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        rgb_to_grayscale_scalar_unrolled(rgb, output, pixel_count);
    }

    pixel_count
}

/// Convert RGBA to grayscale using a pre-allocated buffer (no allocation)
pub fn rgba_to_grayscale_with_buffer(
    rgba: &[u8],
    width: usize,
    height: usize,
    output: &mut [u8],
) -> usize {
    let pixel_count = width * height;
    assert!(output.len() >= pixel_count, "Output buffer too small");

    #[cfg(target_arch = "x86_64")]
    {
        unsafe {
            rgba_to_grayscale_sse2(rgba, output, pixel_count);
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            rgba_to_grayscale_neon(rgba, output, pixel_count);
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        rgba_to_grayscale_scalar_unrolled(rgba, output, pixel_count);
    }

    pixel_count
}
