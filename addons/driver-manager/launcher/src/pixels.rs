use std::collections::HashMap;
use std::path::Path;

pub use swofty_formats::texture::Image;

pub type Rgb = [u8; 3];

pub const NEUTRAL: [Rgb; 4] = [
    [0x2a, 0x34, 0x42],
    [0xcf, 0xd3, 0xd9],
    [0x7d, 0x88, 0x96],
    [0x4c, 0x52, 0x5b],
];

pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const WHOLE: Crop = Crop {
    x: 0.,
    y: 0.,
    w: 1.,
    h: 1.,
};

pub fn resize(src: &Image, crop: &Crop, width: u32, height: u32) -> Image {
    sample(src, crop, width, height, true)
}

fn sample(src: &Image, crop: &Crop, width: u32, height: u32, weighted: bool) -> Image {
    let (sw, sh) = (src.width as f32, src.height as f32);
    let x0 = (crop.x * sw).clamp(0., sw - 1.);
    let y0 = (crop.y * sh).clamp(0., sh - 1.);
    let cw = (crop.w * sw).clamp(1., sw - x0);
    let ch = (crop.h * sh).clamp(1., sh - y0);
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for oy in 0..height {
        let sy0 = ((y0 + ch * oy as f32 / height as f32) as u32).min(src.height - 1);
        let sy1 = ((y0 + ch * (oy + 1) as f32 / height as f32) as u32)
            .max(sy0 + 1)
            .min(src.height);
        for ox in 0..width {
            let sx0 = ((x0 + cw * ox as f32 / width as f32) as u32).min(src.width - 1);
            let sx1 = ((x0 + cw * (ox + 1) as f32 / width as f32) as u32)
                .max(sx0 + 1)
                .min(src.width);
            let mut acc = [0u64; 4];
            let mut n = 0u64;
            let mut alpha = 0u64;
            for sy in sy0..sy1 {
                let row = (sy * src.width) as usize * 4;
                for sx in sx0..sx1 {
                    let p = &src.rgba[row + sx as usize * 4..][..4];
                    let a = if weighted { u64::from(p[3]) } else { 1 };
                    for k in 0..3 {
                        acc[k] += u64::from(p[k]) * a;
                    }
                    acc[3] += a;
                    alpha += u64::from(p[3]);
                    n += 1;
                }
            }
            let w = if weighted { acc[3] } else { n };
            let c = |k: usize| acc[k].checked_div(w).map_or(0, |v| v as u8);
            let a = if w == 0 { 0 } else { (alpha / n.max(1)) as u8 };
            rgba.extend([c(0), c(1), c(2), a]);
        }
    }
    Image { width, height, rgba }
}

pub fn opaque(mut image: Image) -> Image {
    for p in image.rgba.chunks_exact_mut(4) {
        p[3] = 255;
    }
    image
}

pub fn tint(diffuse: &Image, mask: &Image, palette: &[Rgb; 4]) -> Image {
    let resized;
    let mask = if (mask.width, mask.height) == (diffuse.width, diffuse.height) {
        mask
    } else {
        resized = sample(mask, &WHOLE, diffuse.width, diffuse.height, false);
        &resized
    };
    let mut rgba = diffuse.rgba.clone();
    for (p, m) in rgba.chunks_exact_mut(4).zip(mask.rgba.chunks_exact(4)) {
        let mut t = [255f32; 3];
        for (c, colour) in palette.iter().enumerate() {
            let w = f32::from(m[c]) / 255.;
            for k in 0..3 {
                t[k] += (f32::from(colour[k]) - t[k]) * w;
            }
        }
        for k in 0..3 {
            p[k] = (f32::from(p[k]) * t[k] / 255.).round() as u8;
        }
        p[3] = 255;
    }
    Image {
        width: diffuse.width,
        height: diffuse.height,
        rgba,
    }
}

fn distance(a: Rgb, b: Rgb) -> u32 {
    let d = |i: usize| (i32::from(a[i]) - i32::from(b[i])).unsigned_abs();
    d(0) * 3 + d(1) * 4 + d(2) * 2
}

pub fn dominant(image: &Image, n: usize) -> Vec<(Rgb, u32)> {
    let mut buckets: HashMap<u16, [u64; 4]> = HashMap::new();
    let mut total = 0u64;
    for p in image.rgba.chunks_exact(4).filter(|p| p[3] >= 128) {
        let key = (u16::from(p[0] >> 4) << 8) | (u16::from(p[1] >> 4) << 4) | u16::from(p[2] >> 4);
        let b = buckets.entry(key).or_default();
        b[0] += 1;
        for k in 0..3 {
            b[k + 1] += u64::from(p[k]);
        }
        total += 1;
    }
    let mut sorted: Vec<[u64; 4]> = buckets.into_values().collect();
    sorted.sort_by(|a, b| b[0].cmp(&a[0]).then(a[1..].cmp(&b[1..])));
    let mut picked: Vec<(Rgb, u32)> = Vec::new();
    for b in sorted {
        if picked.len() == n || b[0] * 200 < total {
            break;
        }
        let c = [(b[1] / b[0]) as u8, (b[2] / b[0]) as u8, (b[3] / b[0]) as u8];
        match picked.iter_mut().find(|(p, _)| distance(*p, c) < 160) {
            Some(p) => p.1 += b[0] as u32,
            None => picked.push((c, b[0] as u32)),
        }
    }
    picked
}

pub fn chips(colours: &[(Rgb, u32)], width: u32, height: u32) -> Image {
    let colours = if colours.is_empty() {
        vec![([0x80; 3], 1)]
    } else {
        colours.to_vec()
    };
    let total: f32 = colours.iter().map(|c| c.1 as f32).sum();
    let floor = 0.2;
    let spare = 1. - floor * colours.len() as f32;
    let mut edges = Vec::new();
    let mut at = 0.;
    for (_, n) in &colours {
        at += floor + spare * *n as f32 / total;
        edges.push((at * width as f32).round() as u32);
    }
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for _ in 0..height {
        for x in 0..width {
            let i = edges.iter().position(|e| x < *e).unwrap_or(colours.len() - 1);
            let c = colours[i].0;
            rgba.extend([c[0], c[1], c[2], 255]);
        }
    }
    Image { width, height, rgba }
}

pub fn write_png(path: &Path, image: &Image) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
    }
    let tmp = path.with_extension("png.tmp");
    let file = std::fs::File::create(&tmp).map_err(|e| fail(&e))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Balanced);
    let mut writer = encoder.write_header().map_err(|e| fail(&e))?;
    writer.write_image_data(&image.rgba).map_err(|e| fail(&e))?;
    writer.finish().map_err(|e| fail(&e))?;
    std::fs::rename(&tmp, path).map_err(|e| fail(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, p: [u8; 4]) -> Image {
        Image {
            width,
            height,
            rgba: p.repeat((width * height) as usize),
        }
    }

    #[test]
    fn resize_averages_and_ignores_transparent_colour() {
        let mut src = solid(2, 1, [255, 255, 255, 255]);
        src.rgba[4..].copy_from_slice(&[0, 0, 0, 0]);
        let out = resize(&src, &WHOLE, 1, 1);
        assert_eq!(out.rgba, [255, 255, 255, 127]);
        let crop = resize(
            &solid(8, 8, [9, 9, 9, 255]),
            &Crop {
                x: 0.25,
                y: 0.25,
                w: 0.5,
                h: 0.5,
            },
            3,
            3,
        );
        assert_eq!(crop.rgba, [9, 9, 9, 255].repeat(9));
    }

    #[test]
    fn tint_blends_mask_channels_over_white() {
        let diffuse = solid(2, 2, [255, 255, 255, 255]);
        let mut mask = solid(2, 2, [0, 0, 0, 0]);
        mask.rgba[..4].copy_from_slice(&[255, 0, 0, 0]);
        let out = tint(&diffuse, &mask, &NEUTRAL);
        assert_eq!(&out.rgba[..4], &[0x2a, 0x34, 0x42, 255]);
        assert_eq!(&out.rgba[4..8], &[255, 255, 255, 255]);
        let small = tint(&diffuse, &solid(1, 1, [0, 255, 0, 0]), &NEUTRAL);
        assert!(small.rgba.chunks(4).all(|p| p == [0xcf, 0xd3, 0xd9, 255]));
    }

    #[test]
    fn dominant_merges_near_shades_and_keeps_trim() {
        let mut img = solid(10, 10, [10, 20, 60, 255]);
        img.rgba[..4].copy_from_slice(&[12, 22, 62, 255]);
        for i in 90..96 {
            img.rgba[i * 4..i * 4 + 4].copy_from_slice(&[220, 10, 30, 255]);
        }
        let colours = dominant(&img, 3);
        assert_eq!(colours.len(), 2);
        assert_eq!(colours[0].1, 94);
        assert!(colours[1].0[0] > 200);
        let image = chips(&colours, 10, 2);
        assert_eq!(&image.rgba[..3], &[10, 20, 60]);
        assert_eq!(&image.rgba[9 * 4..9 * 4 + 3], &colours[1].0);
    }
}
