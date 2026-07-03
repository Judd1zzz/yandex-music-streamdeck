use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ab_glyph::FontRef;
use base64::Engine;
use image::RgbaImage;
use rust_embed::RustEmbed;

mod cover;
pub mod info;
pub mod progress;
mod text;

pub use cover::CoverCache;
pub use info::InfoInput;
pub use progress::{format_time, ProgressInput};

#[derive(RustEmbed)]
#[folder = "../../../static/img"]
struct Img;

pub struct Renderers {
    static_cache: Mutex<HashMap<String, Arc<str>>>,
    font: FontRef<'static>,
    covers: CoverCache,
}

impl Renderers {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            static_cache: Mutex::new(HashMap::new()),
            font: text::load_font(),
            covers: CoverCache::new(),
        })
    }

    pub fn icon_b64(&self, filename: &str) -> Option<Arc<str>> {
        if let Some(c) = self.static_cache.lock().expect("cache lock").get(filename) {
            return Some(c.clone());
        }
        let file = Img::get(filename)?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(file.data.as_ref());
        let uri: Arc<str> = Arc::from(format!("data:image/png;base64,{b64}"));
        self.static_cache
            .lock()
            .expect("cache lock")
            .insert(filename.to_owned(), uri.clone());
        Some(uri)
    }

    pub fn exists(&self, filename: &str) -> bool {
        Img::get(filename).is_some()
    }

    pub fn render_info(&self, input: InfoInput) -> (String, bool) {
        info::render(&self.font, input)
    }

    pub fn render_progress(&self, input: ProgressInput) -> String {
        progress::render(&self.font, input)
    }

    pub async fn cover(&self, url: &str) -> Option<Arc<RgbaImage>> {
        self.covers.fetch(url).await
    }

    pub fn cover_cached(&self, url: &str) -> Option<Arc<RgbaImage>> {
        self.covers.cached(url)
    }
}

pub(crate) fn to_data_uri(img: &RgbaImage) -> String {
    use std::io::Cursor;
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("png encode");
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&buf))
}

pub(crate) fn embedded_image(name: &str) -> Option<RgbaImage> {
    let f = Img::get(name)?;
    cover::decode_resize(f.data.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_icon_embeds_and_caches() {
        let r = Renderers::new();
        let uri = r.icon_b64("btn_yandex_music_next_v1.png").expect("next_v1");
        assert!(uri.starts_with("data:image/png;base64,"));
        let uri2 = r.icon_b64("btn_yandex_music_next_v1.png").unwrap();
        assert!(Arc::ptr_eq(&uri, &uri2));
        assert!(r.icon_b64("nope.png").is_none());
        assert!(r.exists("emptiness_black.png"));
    }

    #[test]
    fn dynamic_renderers_produce_pngs() {
        let r = Renderers::new();
        let (info_uri, _) = r.render_info(InfoInput {
            cover: None,
            title: "T".into(),
            artist: "A".into(),
            marquee_offset: 0,
            show_cover: false,
            show_title: true,
            show_artist: true,
        });
        assert!(info_uri.starts_with("data:image/png;base64,"));
        let prog = r.render_progress(ProgressInput { progress_ms: 1000.0, duration_ms: 2000.0, mode: "stacked".into() });
        assert!(prog.starts_with("data:image/png;base64,"));
    }
}
