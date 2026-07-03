use ab_glyph::FontRef;
use image::{Rgba, RgbaImage};

use crate::text::{draw_text, draw_text_centered, fill_circle, fill_rect, text_width};
use crate::to_data_uri;

const W: u32 = 144;
const H: u32 = 144;
const TITLE_SIZE: f32 = 28.0;
const ARTIST_SIZE: f32 = 20.0;
const WHITE: [u8; 4] = [255, 255, 255, 255];
const GRAY: [u8; 4] = [180, 180, 180, 255];
const AMBER: [u8; 4] = [255, 208, 0, 255];

pub struct ProgressInput {
    pub progress_ms: f64,
    pub duration_ms: f64,
    pub mode: String,
}

pub fn format_time(ms: f64) -> String {
    if ms.is_nan() || ms < 0.0 {
        return "0:00".to_owned();
    }
    let total = (ms / 1000.0).floor() as i64;
    format!("{}:{:02}", total / 60, total % 60)
}

pub fn render(font: &FontRef, input: ProgressInput) -> String {
    let mut img = RgbaImage::new(W, H);
    let ratio = if input.duration_ms > 0.0 {
        (input.progress_ms / input.duration_ms).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let cur = format_time(input.progress_ms);
    let tot = format_time(input.duration_ms);

    match input.mode.as_str() {
        "inline" => {
            draw_text_centered(&mut img, font, ARTIST_SIZE, ((H - 20) / 2) as f32, &format!("{cur} | {tot}"), WHITE);
        }
        "current_only" => {
            draw_text_centered(&mut img, font, TITLE_SIZE, ((H - 26) / 2) as f32, &cur, WHITE);
        }
        "total_only" => {
            draw_text_centered(&mut img, font, TITLE_SIZE, ((H - 26) / 2) as f32, &tot, WHITE);
        }
        "bar_cli" => {
            let filled = (12.0 * ratio) as usize;
            let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(12 - filled));
            draw_text_centered(&mut img, font, ARTIST_SIZE, ((H - 20) / 2) as f32, &bar, AMBER);
        }
        "bar_modern" => {
            let margin = 15i32;
            let bar_y = (H as i32) / 2 + 10;
            let bar_w = (W as i32) - 2 * margin;
            fill_rect(&mut img, margin, bar_y - 1, margin + bar_w, bar_y + 1, Rgba([60, 60, 60, 255]));
            let filled_w = (bar_w as f64 * ratio) as i32;
            if filled_w > 0 {
                fill_rect(&mut img, margin, bar_y - 2, margin + filled_w, bar_y + 2, Rgba(AMBER));
                fill_circle(&mut img, (margin + filled_w) as f32, bar_y as f32, 5.0, Rgba(WHITE));
            }
            draw_text(&mut img, font, ARTIST_SIZE, margin as f32, 52.0, &cur, WHITE);
            let tw = text_width(font, ARTIST_SIZE, &tot);
            draw_text(&mut img, font, ARTIST_SIZE, W as f32 - margin as f32 - tw, 52.0, &tot, GRAY);
        }
        _ => {
            draw_text_centered(&mut img, font, TITLE_SIZE, (H as i32 / 2 - 30) as f32, &cur, WHITE);
            draw_text_centered(&mut img, font, ARTIST_SIZE, (H as i32 / 2 + 5) as f32, &tot, GRAY);
        }
    }

    to_data_uri(&img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::load_font;

    fn decode(uri: &str) -> RgbaImage {
        use base64::Engine;
        let png = base64::engine::general_purpose::STANDARD
            .decode(uri.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        image::load_from_memory(&png).unwrap().to_rgba8()
    }

    #[test]
    fn time_format() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(38_000.0), "0:38");
        assert_eq!(format_time(179_000.0), "2:59");
        assert_eq!(format_time(-1.0), "0:00");
        assert_eq!(format_time(f64::NAN), "0:00");
        assert_eq!(format_time(38_900.0), "0:38");
        assert_eq!(format_time(179_600.0), "2:59");
    }

    #[test]
    fn all_modes_render_valid_144_png() {
        let font = load_font();
        for mode in ["stacked", "inline", "current_only", "total_only", "bar_cli", "bar_modern"] {
            let uri = render(
                &font,
                ProgressInput { progress_ms: 60_000.0, duration_ms: 180_000.0, mode: mode.into() },
            );
            let img = decode(&uri);
            assert_eq!(img.dimensions(), (144, 144), "режим {mode}");
            assert!(img.pixels().any(|p| p[3] > 0), "режим {mode} должен что-то нарисовать");
        }
    }

    #[test]
    fn bar_modern_has_amber_filled_segment() {
        let font = load_font();
        let uri = render(
            &font,
            ProgressInput { progress_ms: 90_000.0, duration_ms: 180_000.0, mode: "bar_modern".into() },
        );
        let img = decode(&uri);
        let mut amber = false;
        for x in 16..40u32 {
            let p = img.get_pixel(x, 82);
            if p[0] > 200 && p[1] > 150 && p[2] < 80 && p[3] > 0 {
                amber = true;
                break;
            }
        }
        assert!(amber, "bar_modern должен иметь янтарную заливку");
    }
}
