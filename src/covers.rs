//! 从 Steam CDN 拉取游戏封面，缓存到磁盘并在 UI 里复用纹理。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui;

enum CoverState {
    Downloading(Receiver<Result<Vec<u8>, String>>),
    Ready(egui::TextureHandle),
    Failed,
}

#[derive(Default)]
pub struct Covers {
    covers: HashMap<u32, CoverState>,
}

impl Covers {
    /// 请求封面；已请求过（成功、失败或下载中）就直接复用。
    pub fn request(&mut self, app_id: u32) {
        if self.covers.contains_key(&app_id) {
            return;
        }
        self.covers
            .insert(app_id, CoverState::Downloading(spawn_download(app_id)));
    }

    /// 每帧调用：把下载完的字节解码成纹理。
    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut ready = Vec::new();
        for (app_id, state) in self.covers.iter_mut() {
            let CoverState::Downloading(receiver) = state else {
                continue;
            };
            match receiver.try_recv() {
                Ok(Ok(bytes)) => ready.push((*app_id, bytes)),
                Ok(Err(_)) => *state = CoverState::Failed,
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
                Err(TryRecvError::Disconnected) => *state = CoverState::Failed,
            }
        }
        for (app_id, bytes) in ready {
            let texture = decode(&bytes).map(|image| {
                ctx.load_texture(
                    format!("cover-{app_id}"),
                    image,
                    egui::TextureOptions::LINEAR,
                )
            });
            match texture {
                Some(texture) => {
                    self.covers.insert(app_id, CoverState::Ready(texture));
                }
                None => {
                    self.covers.insert(app_id, CoverState::Failed);
                }
            }
        }
    }

    pub fn texture(&self, app_id: u32) -> Option<&egui::TextureHandle> {
        match self.covers.get(&app_id) {
            Some(CoverState::Ready(texture)) => Some(texture),
            _ => None,
        }
    }
}

fn decode(bytes: &[u8]) -> Option<egui::ColorImage> {
    let image = image::load_from_memory(bytes).ok()?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        &rgba.into_raw(),
    ))
}

fn spawn_download(app_id: u32) -> Receiver<Result<Vec<u8>, String>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let _ = thread::Builder::new()
        .name("steam-cover".into())
        .spawn(move || {
            let _ = sender.send(download(app_id));
        });
    receiver
}

fn download(app_id: u32) -> Result<Vec<u8>, String> {
    if let Some(path) = cache_path(app_id) {
        if let Ok(bytes) = fs::read(&path) {
            return Ok(bytes);
        }
    }
    let url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let mut response = agent
        .get(&url)
        .call()
        .map_err(|error| format!("封面请求失败：{error}"))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_vec()
        .map_err(|error| format!("封面读取失败：{error}"))?;
    if let Some(path) = cache_path(app_id) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, &bytes);
    }
    Ok(bytes)
}

fn cache_path(app_id: u32) -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    Some(
        home.join(".cache/gamehaven/covers")
            .join(format!("{app_id}.jpg")),
    )
}

/// Steam header 封面（460×215）的显示尺寸。
pub const COVER_SIZE: egui::Vec2 = egui::vec2(96.0, 45.0);
