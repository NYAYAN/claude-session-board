// Release derlemesinde Windows'ta ek konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pty;
mod sessions;
mod store;

use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, LogicalSize, Manager, RunEvent, WebviewWindow};

/// Claude'un oturum kayıtlarını okur (salt-okuma). Yüzlerce dosya okunduğu için
/// ana iş parçacığını bloklamamak adına ayrı iş parçacığında çalışır.
#[tauri::command]
async fn list_sessions() -> Result<sessions::SessionList, String> {
    tauri::async_runtime::spawn_blocking(sessions::list)
        .await
        .map_err(|e| e.to_string())?
}

/// Oturumu Claude'da açar: `claude://claude.ai/epitaxy/<id>` bağlantısı Windows'ta kayıtlı
/// protokol işleyicisine (Claude.exe) gider, Claude da o oturuma geçer. Kabuk çağrısı bir iki
/// saniye sürebildiği için ana iş parçacığında DEĞİL (pencere donar, yükleniyor göstergesi çizilemez).
#[tauri::command]
async fn open_session(id: String) -> Result<(), String> {
    if !sessions::is_valid_id(&id) {
        return Err(format!("Geçersiz oturum kimliği: {id}"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_opener::open_url(format!("claude://claude.ai/epitaxy/{id}"), None::<&str>)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn load_state(app: AppHandle) -> Result<Value, String> {
    store::load(&app)
}

#[tauri::command]
fn save_state(app: AppHandle, state: Value) -> Result<(), String> {
    store::save(&app, &state)
}

#[tauri::command]
fn set_always_on_top(window: WebviewWindow, on: bool) -> Result<(), String> {
    window.set_always_on_top(on).map_err(|e| e.to_string())
}

/// İlk terminal açılınca dar pencereyi terminale yer açacak kadar genişletir (daraltmaz).
#[tauri::command]
fn ensure_window_width(window: WebviewWindow, width: f64) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    if size.width < width {
        window
            .set_size(LogicalSize::new(width, size.height))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Oturumu gömülü terminalde `claude --resume` ile başlatır; terminal kimliğini döndürür.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn pty_open(
    app: AppHandle,
    resume_id: String,
    cwd: String,
    permission_mode: Option<String>,
    fork: bool,
    cols: u16,
    rows: u16,
    on_event: Channel<pty::PtyEvent>,
) -> Result<u32, String> {
    let args = pty::OpenArgs {
        resume_id,
        cwd,
        permission_mode,
        fork,
        cols,
        rows,
    };
    pty::open(&app, args, on_event)
}

#[tauri::command]
fn pty_write(app: AppHandle, id: u32, data: String) -> Result<(), String> {
    pty::write(&app, id, &data)
}

#[tauri::command]
fn pty_resize(app: AppHandle, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    pty::resize(&app, id, cols, rows)
}

#[tauri::command]
fn pty_close(app: AppHandle, id: u32) {
    pty::close(&app, id)
}

fn main() {
    let app = tauri::Builder::default()
        // İkinci kez başlatılırsa yeni pencere açılmaz, mevcut pencere öne gelir.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .manage(pty::Ptys::default())
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            open_session,
            load_state,
            save_state,
            set_always_on_top,
            ensure_window_width,
            pty_open,
            pty_write,
            pty_resize,
            pty_close
        ])
        .build(tauri::generate_context!())
        .expect("SessionBoard başlatılamadı");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            pty::close_all(app);
        }
    });
}
