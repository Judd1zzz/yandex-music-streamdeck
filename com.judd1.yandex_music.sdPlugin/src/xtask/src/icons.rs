use std::path::Path;

use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::{Rgba, RgbaImage};

pub const ACTION_ICONS: [&str; 10] = [
    "yandex_music_play_pause",
    "yandex_music_next",
    "yandex_music_prev",
    "yandex_music_info",
    "yandex_music_like",
    "yandex_music_dislike",
    "yandex_music_vol_up",
    "yandex_music_vol_down",
    "yandex_music_vol_level",
    "yandex_music_vol_mute",
];

const YELLOW: Rgba<u8> = Rgba([255, 204, 0, 255]);
const SSAA: u32 = 2048;

pub fn force_white(img: &RgbaImage) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        let a = p.0[3];
        *p = Rgba([255, 255, 255, a]);
    }
    out
}

pub fn resize(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    image::imageops::resize(img, w, h, FilterType::Lanczos3)
}

fn rounded_rect_mask(size: u32, radius: f32) -> impl Fn(f32, f32) -> bool {
    let s = size as f32;
    move |x, y| {
        let rx = radius.min(s / 2.0);
        let cx = x.clamp(rx, s - rx);
        let cy = y.clamp(rx, s - rx);
        let dx = x - cx;
        let dy = y - cy;
        dx * dx + dy * dy <= rx * rx
    }
}

fn triangle_mask(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> impl Fn(f32, f32) -> bool {
    move |x, y| {
        let sign = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
            (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
        };
        let d1 = sign((x, y), a, b);
        let d2 = sign((x, y), b, c);
        let d3 = sign((x, y), c, a);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    }
}

pub fn render_plugin_icon(size: u32) -> RgbaImage {
    let s = SSAA as f32;
    let rect = rounded_rect_mask(SSAA, s * 0.18);
    let tri = triangle_mask((s * 0.38, s * 0.28), (s * 0.38, s * 0.72), (s * 0.76, s * 0.50));
    let mut img = RgbaImage::new(SSAA, SSAA);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let fx = x as f32 + 0.5;
        let fy = y as f32 + 0.5;
        if tri(fx, fy) {
            *p = Rgba([255, 255, 255, 255]);
        } else if rect(fx, fy) {
            *p = YELLOW;
        } else {
            *p = Rgba([0, 0, 0, 0]);
        }
    }
    resize(&img, size, size)
}

fn save(img: &RgbaImage, path: &Path) -> Result<()> {
    img.save(path).with_context(|| format!("сохранение {}", path.display()))
}

fn load(path: &Path) -> Result<RgbaImage> {
    Ok(image::open(path).with_context(|| format!("чтение {}", path.display()))?.to_rgba8())
}

pub fn generate(plugin: &Path) -> Result<()> {
    let img_dir = plugin.join("static").join("img");
    let out = img_dir.join("elgato");
    std::fs::create_dir_all(&out).with_context(|| format!("создание {}", out.display()))?;

    for name in ACTION_ICONS {
        let src = load(&img_dir.join(format!("{name}.png")))?;
        let white = force_white(&src);
        save(&resize(&white, 40, 40), &out.join(format!("{name}@2x.png")))?;
        save(&resize(&white, 20, 20), &out.join(format!("{name}.png")))?;
    }

    let ym = force_white(&load(&img_dir.join("ym.png"))?);
    save(&resize(&ym, 56, 56), &out.join("category-icon@2x.png"))?;
    save(&resize(&ym, 28, 28), &out.join("category-icon.png"))?;

    save(&render_plugin_icon(512), &out.join("plugin-icon@2x.png"))?;
    save(&render_plugin_icon(256), &out.join("plugin-icon.png"))?;
    save(&render_plugin_icon(288), &out.join("listing-icon-288.png"))?;

    let empty = load(&img_dir.join("emptiness.png"))?;
    save(&resize(&empty, 144, 144), &out.join("key-empty@2x.png"))?;
    save(&resize(&empty, 72, 72), &out.join("key-empty.png"))?;

    println!("xtask: elgato-иконки → {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_white_keeps_alpha_and_whitens_rgb() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([255, 204, 0, 200]));
        img.put_pixel(1, 0, Rgba([10, 20, 30, 0]));
        let w = force_white(&img);
        assert_eq!(w.get_pixel(0, 0).0, [255, 255, 255, 200]);
        assert_eq!(w.get_pixel(1, 0).0[3], 0);
    }

    #[test]
    fn plugin_icon_sizes_and_shape() {
        for size in [256u32, 288, 512] {
            let icon = render_plugin_icon(size);
            assert_eq!(icon.dimensions(), (size, size));
            let center = icon.get_pixel(size / 2, size / 2);
            assert!(center.0[3] > 200, "центр должен быть непрозрачным");
            let corner = icon.get_pixel(1, 1);
            assert!(corner.0[3] < 30, "скруглённый угол должен быть прозрачным");
            let left_field = icon.get_pixel(size / 5, size / 2);
            for (got, want) in left_field.0[..3].iter().zip([255u8, 204, 0]) {
                assert!(got.abs_diff(want) <= 2, "поле — фирменный жёлтый, получили {:?}", left_field.0);
            }
        }
    }

    #[test]
    fn resize_produces_requested_dimensions() {
        let img = RgbaImage::from_pixel(40, 40, Rgba([255, 255, 255, 255]));
        assert_eq!(resize(&img, 20, 20).dimensions(), (20, 20));
    }
}
