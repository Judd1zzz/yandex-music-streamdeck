use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

pub const FONT_BYTES: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

pub fn load_font() -> FontRef<'static> {
    FontRef::try_from_slice(FONT_BYTES).expect("встроенный DejaVuSans.ttf валиден")
}

pub fn blend(img: &mut RgbaImage, x: i32, y: i32, color: [u8; 4], cov: f32) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let a = (color[3] as f32 / 255.0) * cov.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    let dst_a = px[3] as f32 / 255.0;
    let inv = 1.0 - a;
    let out_a = a + dst_a * inv;
    for i in 0..3 {
        let src = color[i] as f32 / 255.0;
        let dst = px[i] as f32 / 255.0;
        let out = if out_a > 0.0 { (src * a + dst * dst_a * inv) / out_a } else { 0.0 };
        px[i] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    px[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

pub fn text_width(font: &FontRef, scale: f32, text: &str) -> f32 {
    let sf = font.as_scaled(PxScale::from(scale));
    let mut w = 0.0;
    let mut prev = None;
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        if let Some(p) = prev {
            w += sf.kern(p, gid);
        }
        w += sf.h_advance(gid);
        prev = Some(gid);
    }
    w
}

pub fn draw_text(img: &mut RgbaImage, font: &FontRef, scale: f32, x: f32, top_y: f32, text: &str, color: [u8; 4]) {
    let sf = font.as_scaled(PxScale::from(scale));
    let baseline = top_y + sf.ascent();
    let mut cursor = x;
    let mut prev = None;
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        if let Some(p) = prev {
            cursor += sf.kern(p, gid);
        }
        let glyph = gid.with_scale_and_position(PxScale::from(scale), ab_glyph::point(cursor, baseline));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bb = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                blend(img, bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32, color, cov);
            });
        }
        cursor += sf.h_advance(gid);
        prev = Some(gid);
    }
}

pub fn draw_text_centered(img: &mut RgbaImage, font: &FontRef, scale: f32, top_y: f32, text: &str, color: [u8; 4]) {
    let w = text_width(font, scale, text);
    let x = (img.width() as f32 - w) / 2.0;
    draw_text(img, font, scale, x, top_y, text, color);
}

pub fn fill_rect(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    for y in y0.max(0)..y1.min(img.height() as i32) {
        for x in x0.max(0)..x1.min(img.width() as i32) {
            img.put_pixel(x as u32, y as u32, color);
        }
    }
}

pub fn fill_circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: Rgba<u8>) {
    let r2 = r * r;
    let x0 = (cx - r).floor() as i32;
    let x1 = (cx + r).ceil() as i32;
    let y0 = (cy - r).floor() as i32;
    let y1 = (cy + r).ceil() as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                blend(img, x, y, [color[0], color[1], color[2], color[3]], 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_loads_and_measures_cyrillic() {
        let font = load_font();
        let w = text_width(&font, 28.0, "Заглавие");
        assert!(w > 0.0);
        assert!(text_width(&font, 28.0, "AB") < text_width(&font, 28.0, "ABCDEF"));
    }

    #[test]
    fn draw_text_marks_pixels() {
        let font = load_font();
        let mut img = RgbaImage::new(144, 144);
        draw_text(&mut img, &font, 28.0, 5.0, 50.0, "Тест", [255, 255, 255, 255]);
        let any = img.pixels().any(|p| p[3] > 0);
        assert!(any, "текст должен оставить непрозрачные пиксели");
    }
}
