//! Kendini güncelleme. GitHub'daki son sürümün `latest.json`'ı denetlenir (adres:
//! `tauri.conf.json` → `plugins.updater.endpoints`); yeni sürüm varsa NSIS kurulum paketi indirilir,
//! imzası `plugins.updater.pubkey` ile doğrulanır ve sessiz kurulum başlatılır. Paketler ve
//! `latest.json` sürüm çıkarılırken CI'da imzalanıp yayınlanır (bkz. `.github/workflows/release.yml`).
//!
//! Denetim ile kurulum ayrı komutlardır: kullanıcı onaylayana kadar bulunan sürüm burada bekler.

use serde::Serialize;
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

/// Son denetimde bulunan, henüz kurulmamış sürüm.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// Çalışan sürüm.
    current: String,
    /// Yeni sürüm; `None` ise güncel.
    version: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Progress {
    /// İndirme sürüyor; `total` sunucu boyut bildirmezse `None`.
    Download { received: u64, total: Option<u64> },
    /// İndirme bitti, kurulum başlıyor (ardından uygulama kapanır).
    Install,
}

pub async fn check(app: &AppHandle, pending: &Pending) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let found = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let info = UpdateInfo {
        current,
        version: found.as_ref().map(|u| u.version.clone()),
    };
    *pending.0.lock().unwrap() = found;
    Ok(info)
}

/// Bekleyen sürümü indirip kurar. Windows'ta kurulum başlayınca eklenti uygulamayı hemen
/// sonlandırır (`RunEvent::Exit` gelmez); bu yüzden terminaller ve pencere durumu önceden kapatılır.
/// İndirme yarıda kalırsa bekleyen sürüm silinmez, yeniden denenebilir.
pub async fn install(app: &AppHandle, pending: &Pending, progress: Channel<Progress>) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .unwrap()
        .clone()
        .ok_or("Kurulacak güncelleme yok; önce denetle.")?;
    let mut received = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                let _ = progress.send(Progress::Download { received, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    let _ = progress.send(Progress::Install);
    let _ = app.save_window_state(StateFlags::all());
    crate::pty::close_all(app);
    update.install(bytes).map_err(|e| e.to_string())
}
