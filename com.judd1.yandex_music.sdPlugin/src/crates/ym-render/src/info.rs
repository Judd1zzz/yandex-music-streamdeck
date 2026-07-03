use std::sync::Arc;

use ab_glyph::FontRef;
use image::{Rgba, RgbaImage};

use crate::text::{blend, draw_text, text_width};
use crate::{embedded_image, to_data_uri};

const W: u32 = 144;
const H: u32 = 144;
const RADIUS: f32 = 20.0;
const SAFE: f32 = (W - 10) as f32;
const TITLE_SIZE: f32 = 28.0;
const ARTIST_SIZE: f32 = 20.0;
const WHITE: [u8; 4] = [255, 255, 255, 255];
const GRAY: [u8; 4] = [180, 180, 180, 255];

pub struct InfoInput {
    pub cover: Option<Arc<RgbaImage>>,
    pub title: String,
    pub artist: String,
    pub marquee_offset: u32,
    pub show_cover: bool,
    pub show_title: bool,
    pub show_artist: bool,
}

pub fn render(font: &FontRef, input: InfoInput) -> (String, bool) {
    let mut img = RgbaImage::new(W, H);

    let has_cover = input.show_cover && paste_cover(&mut img, &input);

    if has_cover && (input.show_title || input.show_artist) {
        darken_bottom(&mut img);
    }

    let mut needs_scroll = false;
    let (title_y, artist_y) = layout(has_cover, input.show_title, input.show_artist);

    if input.show_title && !input.title.is_empty() {
        needs_scroll |= draw_line(font, &mut img, TITLE_SIZE, title_y, &input.title, WHITE, input.marquee_offset as f32);
    }
    if input.show_artist && !input.artist.is_empty() {
        needs_scroll |= draw_line(font, &mut img, ARTIST_SIZE, artist_y, &input.artist, GRAY, input.marquee_offset as f32 * 0.8);
    }

    if has_cover {
        round_corners(&mut img);
    }

    (to_data_uri(&img), needs_scroll)
}

fn paste_cover(img: &mut RgbaImage, input: &InfoInput) -> bool {
    if let Some(cover) = &input.cover {
        image::imageops::overlay(img, cover.as_ref(), 0, 0);
        return true;
    }
    if let Some(fallback) = embedded_image("emptiness_black.png") {
        image::imageops::overlay(img, &fallback, 0, 0);
        return true;
    }
    for p in img.pixels_mut() {
        *p = Rgba([0, 0, 0, 255]);
    }
    true
}

fn darken_bottom(img: &mut RgbaImage) {
    let start = (H as f32 * 0.4) as u32;
    let span = H as f32 * 0.6;
    for y in start..H {
        let a = (240.0 * (y as f32 - H as f32 * 0.4) / span).round().clamp(0.0, 240.0);
        let cov = a / 255.0;
        for x in 0..W {
            blend(img, x as i32, y as i32, [0, 0, 0, 255], cov);
        }
    }
}

fn layout(has_cover: bool, show_title: bool, show_artist: bool) -> (f32, f32) {
    if has_cover {
        let mut title_y = (H as f32 * 0.61) as i32;
        let mut artist_y = (H as f32 * 0.81) as i32;
        if show_title && !show_artist {
            title_y = (H as f32 * 0.72) as i32;
        }
        if show_artist && !show_title {
            artist_y = (H as f32 * 0.72) as i32;
        }
        (title_y as f32, artist_y as f32)
    } else if show_title && show_artist {
        let start = (H as i32 - 56) / 2;
        (start as f32, (start + 36) as f32)
    } else if show_title {
        (((H as i32 - 26) / 2) as f32, 0.0)
    } else {
        (0.0, ((H as i32 - 20) / 2) as f32)
    }
}

fn draw_line(font: &FontRef, img: &mut RgbaImage, size: f32, top_y: f32, text: &str, color: [u8; 4], offset: f32) -> bool {
    let w = text_width(font, size, text);
    if w > SAFE {
        let cycle = w + 50.0;
        let x = 5.0 - (offset % cycle);
        draw_text(img, font, size, x, top_y, text, color);
        if x + w < W as f32 {
            draw_text(img, font, size, x + cycle, top_y, text, color);
        }
        true
    } else {
        let x = (W as f32 - w) / 2.0;
        draw_text(img, font, size, x, top_y, text, color);
        false
    }
}

fn round_corners(img: &mut RgbaImage) {
    for y in 0..H {
        for x in 0..W {
            let cov = corner_coverage(x, y);
            if cov < 1.0 {
                let p = img.get_pixel_mut(x, y);
                p[3] = (p[3] as f32 * cov).round() as u8;
            }
        }
    }
}

fn corner_coverage(x: u32, y: u32) -> f32 {
    let fx = x as f32 + 0.5;
    let fy = y as f32 + 0.5;
    let cx = fx.clamp(RADIUS, W as f32 - RADIUS);
    let cy = fy.clamp(RADIUS, H as f32 - RADIUS);
    let dx = fx - cx;
    let dy = fy - cy;
    if dx == 0.0 && dy == 0.0 {
        return 1.0;
    }
    let dist = (dx * dx + dy * dy).sqrt();
    (RADIUS + 0.5 - dist).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::load_font;

    fn solid_cover() -> Arc<RgbaImage> {
        let mut c = RgbaImage::new(144, 144);
        for p in c.pixels_mut() {
            *p = Rgba([50, 60, 70, 255]);
        }
        Arc::new(c)
    }

    #[test]
    fn renders_valid_png_with_cover() {
        let font = load_font();
        let (uri, scroll) = render(
            &font,
            InfoInput {
                cover: Some(solid_cover()),
                title: "Short".into(),
                artist: "Artist".into(),
                marquee_offset: 0,
                show_cover: true,
                show_title: true,
                show_artist: true,
            },
        );
        assert!(uri.starts_with("data:image/png;base64,"));
        assert!(!scroll, "короткий заголовок не должен скроллиться");
        use base64::Engine;
        let png = base64::engine::general_purpose::STANDARD
            .decode(uri.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (144, 144));
        assert_eq!(decoded.get_pixel(0, 0)[3], 0, "угол должен быть прозрачным");
        assert!(decoded.get_pixel(72, 72)[3] > 0, "центр непрозрачный");
    }

    #[test]
    fn long_title_scrolls() {
        let font = load_font();
        let (_uri, scroll) = render(
            &font,
            InfoInput {
                cover: None,
                title: "Очень длинное название трека которое точно не влезает".into(),
                artist: String::new(),
                marquee_offset: 10,
                show_cover: false,
                show_title: true,
                show_artist: false,
            },
        );
        assert!(scroll, "длинный заголовок должен скроллиться");
    }
}
