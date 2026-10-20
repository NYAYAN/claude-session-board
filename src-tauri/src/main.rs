// Release derlemesinde Windows'ta ek konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pty;
mod sessions;
mod store;
mod updater;

use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, LogicalSize, Manager, RunEvent, State, WebviewWindow};

/// Claude'un oturum kayıtlarını okur (salt-okuma). Yüzlerce dosya okunduğu için
/// ana iş parçacığını bloklamamak adına ayrı iş parçacığında çalışır. `sessions_dir` kullanıcının
/// elle seçtiği klasör; boşsa kayıtların yeri kendiliğinden aranır.
#[tauri::command]
async fn list_sessions(sessions_dir: Option<String>) -> Result<sessions::SessionList, String> {
    tauri::async_runtime::spawn_blocking(move || sessions::list(sessions_dir.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// Claude'un oturum klasörünü elle seçtirir; vazgeçilirse `None`. Pencere kapanana kadar bekleyen
/// bir çağrı olduğu için ana iş parçacığında değil.
#[tauri::command]
async fn pick_sessions_dir(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file().set_title("Claude oturum klasörünü seç");
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dialog = dialog.set_directory(local);
        }
        dialog
            .blocking_pick_folder()
            .map(|p| p.into_path().map(|p| p.display().to_string()).map_err(|e| e.to_string()))
            .transpose()
    })
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

/// GitHub'daki son sürümü denetler; yeni sürüm varsa kuruluma hazır bekletir.
#[tauri::command]
async fn check_update(app: AppHandle, pending: State<'_, updater::Pending>) -> Result<updater::UpdateInfo, String> {
    updater::check(&app, &pending).await
}

/// Bekleyen sürümü indirip kurar; kurulum başlayınca uygulama kapanır ve yeni sürüm açılır.
#[tauri::command]
async fn install_update(
    app: AppHandle,
    pending: State<'_, updater::Pending>,
    on_progress: Channel<updater::Progress>,
) -> Result<(), String> {
    updater::install(&app, &pending, on_progress).await
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(pty::Ptys::default())
        .manage(updater::Pending::default())
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            pick_sessions_dir,
            open_session,
            load_state,
            save_state,
            set_always_on_top,
            ensure_window_width,
            pty_open,
            pty_write,
            pty_resize,
            pty_close,
            check_update,
            install_update
        ])
        .build(tauri::generate_context!())
        .expect("SessionBoard başlatılamadı");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            pty::close_all(app);
        }
    });
}
