// The Etapix mark (a rounded deep-teal tile with a milestone spine and three Gantt bars),
// rasterized without dependencies so `build.rs` can turn it into the `.exe` icon and the app
// can use it as its window icon.

/// Brand palette (RGB).
pub const DEEP_TEAL: [u8; 3] = [0x0B, 0x6B, 0x66];
pub const SLATE_NAVY: [u8; 3] = [0x1E, 0x2F, 0x3E];
pub const EMERALD: [u8; 3] = [0x14, 0xC3, 0x8E];
pub const MINT: [u8; 3] = [0x4E, 0xE7, 0xC7];
pub const SOFT_GRAY: [u8; 3] = [0xE6, 0xEE, 0xF2];
pub const OFF_WHITE: [u8; 3] = [0xFA, 0xFC, 0xFD];

/// Signed distance to a rounded rectangle (negative inside).
fn rounded(x: f32, y: f32, (x0, y0, x1, y1): (f32, f32, f32, f32), r: f32) -> f32 {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (hx, hy) = ((x1 - x0) / 2.0 - r, (y1 - y0) / 2.0 - r);
    let (dx, dy) = ((x - cx).abs() - hx, (y - cy).abs() - hy);
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    outside + dx.max(dy).min(0.0) - r
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn rgb(c: [u8; 3]) -> [f32; 3] {
    [c[0] as f32, c[1] as f32, c[2] as f32]
}

/// Colour and opacity of the mark at a point of the unit square.
fn sample(x: f32, y: f32) -> ([f32; 3], f32) {
    if rounded(x, y, (0.0, 0.0, 1.0, 1.0), 0.21) > 0.0 {
        return ([0.0; 3], 0.0);
    }
    // Tile: deep teal at the top, slate navy at the bottom.
    let mut c = mix(rgb(DEEP_TEAL), mix(rgb(SLATE_NAVY), rgb(DEEP_TEAL), 0.25), y);
    let over = |c: &mut [f32; 3], col: [f32; 3], a: f32| *c = mix(*c, col, a);
    // Faint timescale lines.
    for lx in [0.46, 0.75] {
        if (x - lx).abs() < 0.012 && (0.07..0.93).contains(&y) {
            over(&mut c, [255.0; 3], 0.10);
        }
    }
    // Milestone spine and its three dots.
    if (x - 0.17).abs() < 0.014 && (0.07..0.93).contains(&y) {
        over(&mut c, rgb(MINT), 0.55);
    }
    for dy in [0.26, 0.485, 0.71] {
        if ((x - 0.17).powi(2) + (y - dy).powi(2)).sqrt() < 0.055 {
            over(&mut c, mix(rgb(MINT), [255.0; 3], 0.25), 1.0);
        }
    }
    // Three Gantt bars: emerald, muted mint, off-white.
    if rounded(x, y, (0.24, 0.20, 0.585, 0.33), 0.035) < 0.0 {
        over(&mut c, mix(rgb(EMERALD), rgb(MINT), (x - 0.24) / 0.6), 1.0);
    }
    if rounded(x, y, (0.375, 0.42, 0.765, 0.55), 0.035) < 0.0 {
        over(&mut c, mix(rgb(EMERALD), [210.0, 235.0, 228.0], 0.55), 1.0);
    }
    if rounded(x, y, (0.565, 0.645, 0.935, 0.775), 0.035) < 0.0 {
        over(&mut c, mix(rgb(SOFT_GRAY), rgb(OFF_WHITE), 0.6), 1.0);
    }
    (c, 1.0)
}

/// The mark as `size`×`size` RGBA pixels (row-major, top row first), 4×4 supersampled.
pub fn rgba(size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut acc, mut alpha) = ([0.0f32; 3], 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    let (c, a) = sample(x, y);
                    acc.iter_mut().zip(c).for_each(|(s, v)| *s += v * a);
                    alpha += a;
                }
            }
            let n = (SS * SS) as f32;
            out.extend(acc.map(|v| if alpha > 0.0 { (v / alpha).round() as u8 } else { 0 }));
            out.push((alpha / n * 255.0).round() as u8);
        }
    }
    out
}
