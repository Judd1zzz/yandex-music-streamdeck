use std::path::Path;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::Tag;

pub struct TrackTags {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub cover_jpeg: Option<Vec<u8>>,
}

pub fn write_tags(path: &Path, t: &TrackTags) -> Result<(), String> {
    let mut tagged = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    let tag_type = tagged.primary_tag_type();
    if tagged.primary_tag_mut().is_none() {
        tagged.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged.primary_tag_mut().ok_or_else(|| "no primary tag".to_owned())?;
    if !t.title.trim().is_empty() {
        tag.set_title(t.title.clone());
    }
    if !t.artist.trim().is_empty() {
        tag.set_artist(t.artist.clone());
    }
    if let Some(a) = t.album.as_deref().filter(|a| !a.trim().is_empty()) {
        tag.set_album(a.to_owned());
    }
    if let Some(y) = t.year {
        tag.set_year(y);
    }
    if let Some(g) = t.genre.as_deref().filter(|g| !g.trim().is_empty()) {
        tag.set_genre(g.to_owned());
    }
    if let Some(c) = &t.cover_jpeg {
        tag.push_picture(Picture::new_unchecked(PictureType::CoverFront, Some(MimeType::Jpeg), None, c.clone()));
    }
    tag.save_to_path(path, WriteOptions::default()).map_err(|e| e.to_string())
}
