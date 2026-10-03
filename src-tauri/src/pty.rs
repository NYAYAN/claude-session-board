//! Gömülü terminal: bir oturumu `claude --resume <id>` ile sözde terminalde (Windows'ta ConPTY)
//! çalıştırır. Çıktı ön yüze kanal (Channel) ile akar, girdi/boyut komutlarla gelir.
//!
//! Ön koşul: bağımsız Claude Code CLI kurulu ve giriş yapılmış olmalı. Masaüstü uygulamanın
//! kendi içindeki kopyası kullanılmaz (oturum açmayı masaüstü uygulama sağlıyor).

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

pub const CLI_NOT_FOUND: &str = "CLI_NOT_FOUND";

/// Ön yüze akan olaylar.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PtyEvent {
    Data { data: String },
    Exit { code: Option<u32> },
}

struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(Default)]
pub struct Ptys {
    next: AtomicU32,
    map: Mutex<HashMap<u32, Pty>>,
}

impl Ptys {
    fn lock(&self) -> MutexGuard<'_, HashMap<u32, Pty>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// CLI'ye aynen geçirilen izin modları. `bypassPermissions` bilerek yok: terminalde izinsiz
/// çalışmayı kullanıcı kendisi seçmeli (Shift+Tab).
const PERMISSION_MODES: [&str; 4] = ["default", "acceptEdits", "plan", "auto"];

pub struct OpenArgs {
    pub resume_id: String,
    pub cwd: String,
    pub permission_mode: Option<String>,
    pub fork: bool,
    pub cols: u16,
    pub rows: u16,
}

pub fn open(app: &AppHandle, args: OpenArgs, channel: Channel<PtyEvent>) -> Result<u32, String> {
    if !is_uuid(&args.resume_id) {
        return Err(format!("Geçersiz konuşma kimliği: {}", args.resume_id));
    }
    let cwd = PathBuf::from(&args.cwd);
    if !cwd.is_dir() {
        return Err(format!("Oturumun klasörü artık yok: {}", args.cwd));
    }
    let claude = find_claude().ok_or_else(|| CLI_NOT_FOUND.to_string())?;

    let mut cmd = command_for(&claude);
    cmd.arg("--resume");
    cmd.arg(&args.resume_id);
    if args.fork {
        cmd.arg("--fork-session");
    }
    if let Some(mode) = args
        .permission_mode
        .as_deref()
        .filter(|m| PERMISSION_MODES.contains(m))
    {
        cmd.arg("--permission-mode");
        cmd.arg(mode);
    }
    cmd.cwd(&cwd);
    scrub_host_env(&mut cmd);

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: args.rows.max(5),
            cols: args.cols.max(20),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Claude başlatılamadı: {e}"))?;
    drop(pair.slave);
    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let killer = child.clone_killer();

    let ptys = app.state::<Ptys>();
    let id = ptys.next.fetch_add(1, Ordering::Relaxed) + 1;
    ptys.lock().insert(
        id,
        Pty {
            master: pair.master,
            writer,
            killer,
        },
    );

    // ConPTY süreç bitince okuyucuya kendiliğinden EOF vermez: çıkışı bekleyen iş parçacığı kodu
    // saklar ve PTY'yi kapatır; okuyucu kalan çıktıyı boşaltıp EOF alınca Exit olayını yollar.
    let exit_code = Arc::new(Mutex::new(None::<u32>));
    {
        let app = app.clone();
        let exit_code = exit_code.clone();
        std::thread::spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code());
            *exit_code.lock().unwrap_or_else(|e| e.into_inner()) = code;
            let removed = app.state::<Ptys>().lock().remove(&id);
            drop(removed); // kilit dışında kapat (ConPTY kapanırken çıktı boşalmasını bekleyebilir)
        });
    }
    std::thread::spawn(move || {
        pump(reader, |ev| channel.send(ev).is_ok());
        let code = *exit_code.lock().unwrap_or_else(|e| e.into_inner());
        let _ = channel.send(PtyEvent::Exit { code });
    });
    Ok(id)
}

pub fn write(app: &AppHandle, id: u32, data: &str) -> Result<(), String> {
    let ptys = app.state::<Ptys>();
    let mut map = ptys.lock();
    let pty = map.get_mut(&id).ok_or("Terminal kapalı")?;
    pty.writer
        .write_all(data.as_bytes())
        .and_then(|_| pty.writer.flush())
        .map_err(|e| e.to_string())
}

pub fn resize(app: &AppHandle, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let ptys = app.state::<Ptys>();
    let map = ptys.lock();
    let Some(pty) = map.get(&id) else {
        return Ok(()); // kapanmış terminal: yoksay
    };
    pty.master
        .resize(PtySize {
            rows: rows.max(5),
            cols: cols.max(20),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())
}

pub fn close(app: &AppHandle, id: u32) {
    let removed = app.state::<Ptys>().lock().remove(&id);
    if let Some(mut pty) = removed {
        let _ = pty.killer.kill();
    }
}

/// Uygulama kapanırken: açık tüm Claude süreçlerini sonlandır (konuşma dosyası anlık yazıldığı
/// için yarıda kalan tur dışında kayıp olmaz).
pub fn close_all(app: &AppHandle) {
    let all: Vec<Pty> = app.state::<Ptys>().lock().drain().map(|(_, p)| p).collect();
    for mut pty in all {
        let _ = pty.killer.kill();
    }
}

/// Çıktıyı EOF'a kadar okur, UTF-8 parçalar halinde `emit`'e verir; `emit` false dönerse durur.
fn pump(mut reader: Box<dyn Read + Send>, mut emit: impl FnMut(PtyEvent) -> bool) {
    let mut buf = [0u8; 16 * 1024];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.extend_from_slice(&buf[..n]);
        let text = take_utf8(&mut pending);
        if !text.is_empty() && !emit(PtyEvent::Data { data: text }) {
            break;
        }
    }
    if !pending.is_empty() {
        emit(PtyEvent::Data {
            data: String::from_utf8_lossy(&pending).into_owned(),
        });
    }
}

/// Tamamlanmış UTF-8 kısmını döndürür; okuma sınırında yarım kalan çok baytlı karakteri
/// (ör. "ş" = 2 bayt) bir sonraki okumaya bırakır, yoksa ekranda � görünür.
fn take_utf8(pending: &mut Vec<u8>) -> String {
    match std::str::from_utf8(pending) {
        Ok(s) => {
            let s = s.to_owned();
            pending.clear();
            s
        }
        Err(e) if e.error_len().is_none() => {
            let valid = e.valid_up_to();
            let s = String::from_utf8_lossy(&pending[..valid]).into_owned();
            pending.drain(..valid);
            s
        }
        Err(_) => {
            let s = String::from_utf8_lossy(pending).into_owned();
            pending.clear();
            s
        }
    }
}

/// Bağımsız Claude CLI: önce PATH, sonra resmi yükleyicinin (`~\.local\bin`) ve npm'in yerleri.
/// Son ikisi, SessionBoard kurulumdan ÖNCE açıldıysa PATH'i eski kalacağı için de gerekir.
fn find_claude() -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("USERPROFILE") {
        dirs.push(PathBuf::from(home).join(".local").join("bin"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs.iter()
        .flat_map(|d| ["claude.exe", "claude.cmd"].map(|n| d.join(n)))
        .find(|p| p.is_file())
}

fn command_for(claude: &Path) -> CommandBuilder {
    let is_script = claude
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd"));
    if is_script {
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.args(["/d", "/c"]);
        cmd.arg(claude.as_os_str());
        cmd
    } else {
        CommandBuilder::new(claude.as_os_str())
    }
}

/// SessionBoard Claude'un içinden (ör. Claude'un terminalinden) başlatıldıysa masaüstü uygulamanın
/// kendi Claude Code sürecine verdiği değişkenleri miras alır: `CLAUDECODE` (iç içe oturum),
/// mesajlaşma soketi, ana makine kimlik doğrulaması, `ANTHROPIC_BASE_URL`… Bunlarla başlayan CLI
/// kendini masaüstüne bağlı sanır. Bağımsız CLI kendi girişiyle çalışmalı → temizlenir.
/// Gezgin/Başlat menüsünden açılışta bu değişkenler yoktur; kullanıcının kendi ayarlarına dokunulmaz.
fn scrub_host_env(cmd: &mut CommandBuilder) {
    let from_desktop =
        std::env::var_os("CLAUDECODE").is_some() || std::env::var_os("CLAUDE_CODE_ENTRYPOINT").is_some();
    if !from_desktop {
        return;
    }
    for (key, _) in std::env::vars_os() {
        let Some(k) = key.to_str() else { continue };
        let upper = k.to_ascii_uppercase();
        if upper.starts_with("CLAUDE")
            || upper == "ANTHROPIC_BASE_URL"
            || upper.starts_with("MCP_CONNECTION")
            || upper.starts_with("MCP_SERVER_CONNECTION")
        {
            cmd.env_remove(k);
        }
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yarim_utf8_sonraki_okumaya_kalir() {
        let s = "aş".as_bytes(); // 'ş' = 0xC5 0x9F
        let mut pending = s[..2].to_vec(); // "a" + ilk bayt
        assert_eq!(take_utf8(&mut pending), "a");
        assert_eq!(pending, vec![0xC5]);
        pending.push(0x9F);
        assert_eq!(take_utf8(&mut pending), "ş");
        assert!(pending.is_empty());
    }

    /// ConPTY uçtan uca: süreç çıktısı okunur ve süreç bitip PTY kapatılınca okuyucu EOF alır
    /// (open() içindeki bekleme iş parçacığının yaptığı gibi). ConPTY açılışta imleç konumunu sorar
    /// (ESC[6n) ve yanıt gelene kadar süreci bekletir; uygulamada xterm.js yanıtlar, burada test.
    /// Hiçbir adım sonsuza kadar beklemez: takılırsa 15 sn'de başarısız olur.
    #[cfg(windows)]
    #[test]
    fn conpty_ciktiyi_okur_ve_kapanir() {
        use std::sync::mpsc;
        use std::time::Duration;

        let pair = native_pty_system()
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .unwrap();
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.args(["/d", "/c", "echo", "sessionboard-pty-ok"]);
        let mut child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();
        let master = pair.master;

        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut out = String::new();
            pump(reader, |ev| {
                if let PtyEvent::Data { data } = ev {
                    if data.contains("\x1b[6n") {
                        let _ = writer.write_all(b"\x1b[1;1R");
                        let _ = writer.flush();
                    }
                    out.push_str(&data);
                }
                true
            });
            let _ = out_tx.send(out);
        });
        let (exit_tx, exit_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = exit_tx.send(child.wait().map(|s| s.success()).unwrap_or(false));
        });

        let exited_ok = exit_rx
            .recv_timeout(Duration::from_secs(15))
            .expect("süreç bitmedi (ConPTY yanıt bekliyor olabilir)");
        drop(master);
        let out = out_rx
            .recv_timeout(Duration::from_secs(15))
            .expect("okuyucu EOF almadı (ConPTY kapanmadı)");
        assert!(exited_ok);
        assert!(out.contains("sessionboard-pty-ok"), "çıktı: {out:?}");
    }

    #[test]
    fn uuid_dogrulama() {
        assert!(is_uuid("9d4e1b7a-2c3f-4a5b-8e6d-0f1a2b3c4d5e"));
        assert!(!is_uuid("9d4e1b7a-2c3f-4a5b-8e6d-0f1a2b3c4d5"));
        assert!(!is_uuid("9d4e1b7a-2c3f-4a5b-8e6d-0f1a2b3c4d5&"));
    }
}
