//! Claude masaüstü uygulamasının oturum kayıtlarını okur.
//!
//! Kaynak: `<kök>\<hesap>\<org>\local_*.json`; kök için bkz. `session_roots`.
//! Bu dosyalar Claude'un iç (belgelenmemiş) biçimidir: yalnız OKUNUR, asla yazılmaz.
//! Biçim bir Claude güncellemesinde değişirse en kötü ihtimalle liste boş/eksik gelir;
//! Claude'un kendisi etkilenmez.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Kayıttan ihtiyaç duyulan alanlar. Geri kalanı (MCP ayarları vb.) serde tarafından atlanır.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSession {
    session_id: String,
    #[serde(default)]
    cli_session_id: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    origin_cwd: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    is_archived: Option<bool>,
    #[serde(default)]
    created_at: Option<f64>,
    #[serde(default)]
    last_activity_at: Option<f64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub cwd: String,
    /// Oturumun ait olduğu klasör (worktree ise reponun kökü), görüntü biçiminde.
    pub workspace: String,
    /// `workspace`'in karşılaştırma anahtarı (Windows yolları büyük/küçük harfe duyarsız).
    pub workspace_key: String,
    pub branch: Option<String>,
    /// Konuşma dosyasının kimliği (`~\.claude\projects\…\<id>.jsonl`); `claude --resume` bunu alır.
    pub cli_session_id: Option<String>,
    /// Masaüstündeki izin modu (default/acceptEdits/plan/auto/bypassPermissions).
    pub permission_mode: Option<String>,
    pub archived: bool,
    pub created_at: f64,
    pub last_activity_at: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionList {
    pub sessions: Vec<SessionInfo>,
    /// Okunan klasör (hata/boş durum mesajında gösterilir).
    pub source: String,
    /// Okunamayan (bozuk/biçimi değişmiş) kayıt sayısı.
    pub skipped: usize,
}

/// Oturum kayıtlarının olası kök klasörleri (var olanlar).
///
/// Claude Store/MSIX paketi olarak kuruluysa AppData'sı sanallaştırılır: kayıtlar fiziksel olarak
/// `%LOCALAPPDATA%\Packages\Claude_<yayıncı>\LocalCache\Roaming\Claude\claude-code-sessions`
/// altındadır; `%APPDATA%\Claude\...` yolu yalnız paketin İÇİNDEN (ör. Claude'un açtığı terminal)
/// görünür, dışarıdan açılan SessionBoard orada hiçbir şey bulamaz. Klasik kurulumda ise kayıtlar
/// doğrudan `%APPDATA%\Claude\...`'dadır. İkisi de denenir.
fn session_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        for pkg in sub_dirs(&PathBuf::from(local).join("Packages")) {
            let is_claude = pkg
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("Claude_"));
            if is_claude {
                roots.push(
                    pkg.join("LocalCache")
                        .join("Roaming")
                        .join("Claude")
                        .join("claude-code-sessions"),
                );
            }
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(appdata).join("Claude").join("claude-code-sessions"));
    }
    roots.retain(|r| r.is_dir());
    roots
}

pub fn list() -> Result<SessionList, String> {
    let roots = session_roots();
    if roots.is_empty() {
        return Err(
            "Claude oturum klasörü bulunamadı (%LOCALAPPDATA%\\Packages\\Claude_*\\LocalCache\\Roaming\\Claude ve %APPDATA%\\Claude denendi)"
                .into(),
        );
    }

    // Kayıtlar hesap klasörlerine ayrılır. Kenar çubuğu yalnız oturum açık hesabı gösterir;
    // onu dosyadan bilemediğimiz için en son etkinliğin olduğu hesap seçilir. (Paket içinden
    // çalışırken iki kök aynı fiziksel klasörü gösterebilir; tek hesap seçildiği için çift sayılmaz.)
    let mut best: Option<(f64, Vec<SessionInfo>, usize, &PathBuf)> = None;
    for root in &roots {
        for account in sub_dirs(root) {
            let (sessions, skipped) = read_account(&account);
            let latest = sessions
                .iter()
                .map(|s| s.last_activity_at)
                .fold(0.0, f64::max);
            if best.as_ref().map_or(true, |(b, ..)| latest > *b) {
                best = Some((latest, sessions, skipped, root));
            }
        }
    }

    let (mut sessions, skipped, source) = match best {
        Some((_, sessions, skipped, root)) => (sessions, skipped, root.display().to_string()),
        None => (Vec::new(), 0, roots[0].display().to_string()),
    };
    // Oturum terminalde sürdürülünce masaüstünün kaydı güncellenmez; konuşma dosyası güncellenir.
    let times = transcript_times();
    for s in &mut sessions {
        if let Some(t) = s.cli_session_id.as_ref().and_then(|c| times.get(c)) {
            s.last_activity_at = s.last_activity_at.max(*t);
        }
    }
    sessions.sort_by(|a, b| b.last_activity_at.total_cmp(&a.last_activity_at));
    Ok(SessionList {
        sessions,
        source,
        skipped,
    })
}

/// Konuşma dosyalarının son yazılma zamanı: `~\.claude\projects\<proje>\<cliSessionId>.jsonl`
/// → ms. Profil kökü sanallaştırılmaz; Claude'un içinden ve dışından aynı klasör görünür.
fn transcript_times() -> HashMap<String, f64> {
    let mut times = HashMap::new();
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return times;
    };
    for project in sub_dirs(&PathBuf::from(home).join(".claude").join("projects")) {
        let Ok(entries) = fs::read_dir(&project) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e == "jsonl") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            if let Some(d) = modified {
                times.insert(stem.to_string(), d.as_millis() as f64);
            }
        }
    }
    times
}

/// Bir hesap klasörünün (`<hesap>\<org>\local_*.json`) oturumları + okunamayan kayıt sayısı.
fn read_account(account: &Path) -> (Vec<SessionInfo>, usize) {
    let mut sessions = Vec::new();
    let mut skipped = 0;
    for org in sub_dirs(account) {
        let Ok(entries) = fs::read_dir(&org) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_session_file(&path) {
                continue;
            }
            match parse(&path) {
                Some(s) => sessions.push(s),
                None => skipped += 1,
            }
        }
    }
    (sessions, skipped)
}

fn sub_dirs(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

fn is_session_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "json")
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("local_"))
}

fn parse(path: &Path) -> Option<SessionInfo> {
    let bytes = fs::read(path).ok()?;
    let raw: RawSession = serde_json::from_slice(&bytes).ok()?;
    let cwd = raw.cwd.unwrap_or_default();
    // Worktree oturumunda originCwd reponun köküdür; yoksa cwd'den türetilir.
    let origin = raw
        .origin_cwd
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| cwd.clone());
    let workspace = workspace_root(&origin);
    Some(SessionInfo {
        workspace_key: workspace.to_lowercase(),
        workspace,
        id: raw.session_id,
        title: raw.title.unwrap_or_default().trim().to_string(),
        cwd,
        branch: raw.branch.filter(|b| !b.is_empty()),
        cli_session_id: raw.cli_session_id.filter(|c| !c.is_empty()),
        permission_mode: raw.permission_mode.filter(|m| !m.is_empty()),
        archived: raw.is_archived.unwrap_or(false),
        created_at: raw.created_at.unwrap_or(0.0),
        last_activity_at: raw.last_activity_at.or(raw.created_at).unwrap_or(0.0),
    })
}

/// Yolu normalleştirir ve worktree'yi reposuna bağlar:
/// `...\Repo\.claude\worktrees\ad` → `...\Repo`.
pub fn workspace_root(path: &str) -> String {
    let normalized = path.trim().replace('/', "\\");
    let trimmed = normalized.trim_end_matches('\\');
    // ASCII küçültme bayt uzunluğunu korur → bulunan indeks orijinal dizgede de geçerli.
    match trimmed
        .to_ascii_lowercase()
        .find("\\.claude\\worktrees\\")
    {
        Some(i) => trimmed[..i].to_string(),
        None => trimmed.to_string(),
    }
}

/// Oturum kimliğini doğrular: yalnız `local_` + onaltılık/tire. Bağlantıya gömülmeden önce
/// çağrılır; başka bir şey (ör. `&`, boşluk, başka protokol) asla dışarı açılmaz.
pub fn is_valid_id(id: &str) -> bool {
    id.len() <= 64
        && id
            .strip_prefix("local_")
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit() || c == '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_reposuna_baglanir() {
        assert_eq!(
            workspace_root(r"C:\Work\my-app\.claude\worktrees\brave-otter-1a2b3c"),
            r"C:\Work\my-app"
        );
        assert_eq!(
            workspace_root(r"C:\Work\App\.Claude\Worktrees\x\sub"),
            r"C:\Work\App"
        );
    }

    #[test]
    fn duz_klasor_normallesir() {
        assert_eq!(workspace_root(r"C:\Work\My Project\"), r"C:\Work\My Project");
        assert_eq!(workspace_root("C:/Work/Şube/Proje"), r"C:\Work\Şube\Proje");
    }

    /// Bilgisayardaki Claude kayıtlarıyla duman testi (Claude masaüstü kurulu olmalı):
    /// `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn gercek_kayitlar_okunur() {
        let list = list().expect("kayıtlar okunamadı");
        let mut workspaces: Vec<&str> = list.sessions.iter().map(|s| s.workspace.as_str()).collect();
        workspaces.sort();
        workspaces.dedup();
        println!("oturum={} atlanan={} klasör={}", list.sessions.len(), list.skipped, workspaces.len());
        for s in list.sessions.iter().take(3) {
            println!("  {} | {} | {}", s.id, s.title, s.workspace);
        }
        assert!(!list.sessions.is_empty());
        assert!(list.sessions.iter().all(|s| is_valid_id(&s.id)));
    }

    #[test]
    fn kimlik_dogrulama() {
        assert!(is_valid_id("local_3f2a9c4e-8b1d-4e6f-9a7c-2d5b8e1f0a63"));
        assert!(!is_valid_id("local_"));
        assert!(!is_valid_id("local_abc&calc"));
        assert!(!is_valid_id("cse_3f2a9c4e"));
        assert!(!is_valid_id("local_3f2a9c4e 8b1d"));
    }
}
