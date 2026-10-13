use openh264::formats::YUVBuffer;

#[inline(always)]
fn rgb_to_y(r: i32, g: i32, b: i32) -> u8 {
    let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
    y.clamp(0, 255) as u8
}

#[inline(always)]
fn rgb_to_u(r: i32, g: i32, b: i32) -> u8 {
    let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
    u.clamp(0, 255) as u8
}

#[inline(always)]
fn rgb_to_v(r: i32, g: i32, b: i32) -> u8 {
    let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
    v.clamp(0, 255) as u8
}

/// Converts BGRA 32-bit pixel data directly into standard I420 YUV format.
///
/// Uses fast integer arithmetic (BT.601 standard) with 2x2 chroma subsampling.
pub fn bgra_to_yuv420p(
    width: usize,
    height: usize,
    bgra: &[u8],
    y_plane: &mut [u8],
    u_plane: &mut [u8],
    v_plane: &mut [u8],
) {
    let w = width & !1;
    let h = height & !1;
    let half_w = w / 2;

    for j in 0..h / 2 {
        let y0_idx = (j * 2) * w;
        let y1_idx = (j * 2 + 1) * w;
        let uv_idx = j * half_w;

        for i in 0..half_w {
            let px0 = i * 2;
            let px1 = px0 + 1;

            let row0_offset = (j * 2 * width + px0) * 4;
            let row1_offset = ((j * 2 + 1) * width + px0) * 4;

            // Pixel (0, 0)
            let b0 = bgra[row0_offset] as i32;
            let g0 = bgra[row0_offset + 1] as i32;
            let r0 = bgra[row0_offset + 2] as i32;

            // Pixel (1, 0)
            let b1 = bgra[row0_offset + 4] as i32;
            let g1 = bgra[row0_offset + 5] as i32;
            let r1 = bgra[row0_offset + 6] as i32;

            // Pixel (0, 1)
            let b2 = bgra[row1_offset] as i32;
            let g2 = bgra[row1_offset + 1] as i32;
            let r2 = bgra[row1_offset + 2] as i32;

            // Pixel (1, 1)
            let b3 = bgra[row1_offset + 4] as i32;
            let g3 = bgra[row1_offset + 5] as i32;
            let r3 = bgra[row1_offset + 6] as i32;

            // Compute Y for 4 pixels
            y_plane[y0_idx + px0] = rgb_to_y(r0, g0, b0);
            y_plane[y0_idx + px1] = rgb_to_y(r1, g1, b1);
            y_plane[y1_idx + px0] = rgb_to_y(r2, g2, b2);
            y_plane[y1_idx + px1] = rgb_to_y(r3, g3, b3);

            // Compute average RGB for 2x2 block
            let r_avg = (r0 + r1 + r2 + r3 + 2) >> 2;
            let g_avg = (g0 + g1 + g2 + g3 + 2) >> 2;
            let b_avg = (b0 + b1 + b2 + b3 + 2) >> 2;

            // Compute U and V
            u_plane[uv_idx + i] = rgb_to_u(r_avg, g_avg, b_avg);
            v_plane[uv_idx + i] = rgb_to_v(r_avg, g_avg, b_avg);
        }
    }
}

/// Converts BGRA 32-bit pixel data directly into an openh264 `YUVBuffer`.
pub fn bgra_to_yuv_buffer(width: usize, height: usize, bgra: &[u8]) -> YUVBuffer {
    let w = width & !1;
    let h = height & !1;
    let total_y = w * h;
    let total_uv = total_y / 4;
    let mut yuv = vec![0u8; total_y + 2 * total_uv];

    let (y_plane, uv_planes) = yuv.split_at_mut(total_y);
    let (u_plane, v_plane) = uv_planes.split_at_mut(total_uv);

    bgra_to_yuv420p(w, h, bgra, y_plane, u_plane, v_plane);

    YUVBuffer::from_vec(yuv, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openh264::formats::YUVSource;

    #[test]
    fn test_bgra_to_yuv_dimensions() {
        let width = 64;
        let height = 64;
        let bgra = vec![128u8; width * height * 4];
        let yuv = bgra_to_yuv_buffer(width, height, &bgra);
        assert_eq!(yuv.dimensions(), (width, height));
    }

    #[test]
    fn test_pure_white_black() {
        // Pure White BGRA: B=255, G=255, R=255, A=255
        let white_bgra = vec![255u8; 4 * 4 * 4];
        let mut y = vec![0u8; 16];
        let mut u = vec![0u8; 4];
        let mut v = vec![0u8; 4];
        bgra_to_yuv420p(4, 4, &white_bgra, &mut y, &mut u, &mut v);

        // In BT.601, pure white Y should be ~235, U ~128, V ~128
        assert!(y[0] >= 230 && y[0] <= 240);
        assert_eq!(u[0], 128);
        assert_eq!(v[0], 128);
    }
}
