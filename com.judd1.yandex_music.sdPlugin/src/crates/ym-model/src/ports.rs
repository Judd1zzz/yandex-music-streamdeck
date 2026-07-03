use std::path::PathBuf;

use async_trait::async_trait;

use crate::{ActionResult, MediaState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeAction {
    Up,
    Down,
    Set(u8),
    Mute,
}

#[async_trait]
pub trait MediaController: Send + Sync {
    fn is_connected(&self) -> bool;
    fn snapshot(&self) -> MediaState;
    fn set_local_port(&self, _port: u16) {}
    async fn oauth_token(&self) -> Option<String> {
        None
    }
    async fn play_pause(&self) -> ActionResult;
    async fn next(&self) -> ActionResult;
    async fn prev(&self) -> ActionResult;
    async fn toggle_like(&self) -> ActionResult;
    async fn toggle_dislike(&self) -> ActionResult;
    async fn change_volume(&self, action: VolumeAction) -> ActionResult;
}

pub struct StubController;

#[async_trait]
impl MediaController for StubController {
    fn is_connected(&self) -> bool {
        false
    }
    fn snapshot(&self) -> MediaState {
        MediaState::default()
    }
    async fn play_pause(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn next(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn prev(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn toggle_like(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn toggle_dislike(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn change_volume(&self, _action: VolumeAction) -> ActionResult {
        ActionResult::default()
    }
}

#[async_trait]
pub trait Downloader: Send + Sync {
    async fn download(
        &self,
        track_id: &str,
        token: &str,
        dir_setting: &str,
        format: &str,
    ) -> anyhow::Result<PathBuf>;
}

pub struct StubDownloader;

#[async_trait]
impl Downloader for StubDownloader {
    async fn download(
        &self,
        _track_id: &str,
        _token: &str,
        _dir_setting: &str,
        _format: &str,
    ) -> anyhow::Result<PathBuf> {
        anyhow::bail!("загрузчик не подключён")
    }
}
