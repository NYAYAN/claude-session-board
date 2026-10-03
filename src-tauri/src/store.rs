//! SessionBoard'un kendi durumu: gruplar, atamalar, sıralama, görünüm tercihleri.
//! Claude'un dosyalarından tamamen ayrıdır: `%USERPROFILE%\.sessionboard\state.json`.
//!
//! Neden AppData DEĞİL: SessionBoard Claude'un içinden (ör. Claude'un terminalinden) başlatılırsa
//! Claude'un MSIX paketinin içinde çalışır ve AppData'ya yazdığı her şey paketin özel klasörüne
//! yönlendirilir. Başlat menüsünden açılan SessionBoard o dosyayı göremez → iki ayrı grup listesi
//! oluşur. Kullanıcı profilinin kökü sanallaştırılmaz; iki durumda da aynı dosya kullanılır.
//!
//! İçerik ön yüzün sözleşmesidir (bkz. `src/app.js` → `normalizeState`); burada yalnız
//! güvenli okuma/yazma yapılır: geçici dosyaya yaz → eski sürümü `.bak`'a al → yerine taşı.

use serde_json::Value;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .home_dir()
        .map_err(|e| e.to_string())?
        .join(".sessionboard");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("state.json"))
}

/// İlk sürümün kullandığı konum (AppData); yeni dosya yoksa bir kez oradan okunur.
fn legacy_state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("state.json"))
}

pub fn load(app: &AppHandle) -> Result<Value, String> {
    let path = state_path(app)?;
    let source = if path.exists() {
        path
    } else {
        match legacy_state_path(app) {
            Some(old) if old.exists() => old,
            _ => return Ok(Value::Object(Default::default())),
        }
    };
    match fs::read(&source) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
            format!(
                "{} okunamadı ({e}). Önceki sürüm: state.json.bak",
                source.display()
            )
        }),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Value::Object(Default::default())),
        Err(e) => Err(e.to_string()),
    }
}

pub fn save(app: &AppHandle, state: &Value) -> Result<(), String> {
    let path = state_path(app)?;
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    if path.exists() {
        // Yedek best-effort: başarısız olursa asıl kayıt yine yapılır.
        let _ = fs::copy(&path, path.with_extension("json.bak"));
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
