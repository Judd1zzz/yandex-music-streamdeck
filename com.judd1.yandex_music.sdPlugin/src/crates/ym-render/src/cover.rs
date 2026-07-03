use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use image::imageops::FilterType;
use image::RgbaImage;
use lru::LruCache;

pub struct CoverCache {
    lru: Mutex<LruCache<String, Arc<RgbaImage>>>,
    http: reqwest::Client,
}

impl CoverCache {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();
        Self {
            lru: Mutex::new(LruCache::new(NonZeroUsize::new(10).unwrap())),
            http,
        }
    }

    pub fn cached(&self, url: &str) -> Option<Arc<RgbaImage>> {
        self.lru.lock().unwrap().get(url).cloned()
    }

    pub async fn fetch(&self, url: &str) -> Option<Arc<RgbaImage>> {
        if let Some(c) = self.cached(url) {
            return Some(c);
        }
        let full = match url.strip_prefix("//") {
            Some(rest) => format!("https://{rest}"),
            None => url.to_owned(),
        };
        let bytes = self.http.get(&full).send().await.ok()?.bytes().await.ok()?;
        let img = tokio::task::spawn_blocking(move || decode_resize(&bytes)).await.ok()??;
        let arc = Arc::new(img);
        self.lru.lock().unwrap().put(url.to_owned(), arc.clone());
        Some(arc)
    }
}

impl Default for CoverCache {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn decode_resize(bytes: &[u8]) -> Option<RgbaImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(image::imageops::resize(&img, 144, 144, FilterType::Lanczos3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn tiny_png() -> Vec<u8> {
        let mut img = RgbaImage::new(4, 4);
        for p in img.pixels_mut() {
            *p = image::Rgba([10, 20, 30, 255]);
        }
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        buf
    }

    #[test]
    fn decode_and_resize_to_144() {
        let out = decode_resize(&tiny_png()).expect("декод");
        assert_eq!(out.dimensions(), (144, 144));
    }

    #[test]
    fn invalid_bytes_none() {
        assert!(decode_resize(b"not an image").is_none());
    }

    #[test]
    fn cache_put_get() {
        let c = CoverCache::new();
        assert!(c.cached("u").is_none());
        let img = Arc::new(RgbaImage::new(144, 144));
        c.lru.lock().unwrap().put("u".into(), img.clone());
        assert!(Arc::ptr_eq(&c.cached("u").unwrap(), &img));
    }
}
