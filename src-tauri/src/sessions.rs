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

const SESSIONS_DIR: &str = "claude-code-sessions";

/// Oturum kayıtlarının olası kök klasörleri (var olanlar). Yeri Claude'un sürümüne, kurulum türüne
/// (Store/MSIX, klasik, kurumsal) ve bilgisayara göre değişiyor; bilinen düzenler:
///
/// - `%LOCALAPPDATA%\Claude-Data\<paket>\claude-code-sessions`: Claude Ekim 2026 güncellemesiyle
///   Store/MSIX kurulumunun verisini sanallaştırılmış AppData'dan buraya taşıdı. Eski yerlerde
///   yalnız boş bir `.user-data-left.<paket>` işaret dosyası kalır.
/// - `%LOCALAPPDATA%\Packages\Claude_<yayıncı>\LocalCache\Roaming\Claude\claude-code-sessions`:
///   o güncellemeden önceki Store/MSIX kurulumu. AppData'sı sanallaştırıldığı için kayıtlar fiziksel
///   olarak buradadır; `%APPDATA%\Claude\...` yolu yalnız paketin İÇİNDEN (ör. Claude'un açtığı
///   terminal) görünür.
/// - `%APPDATA%\Claude\claude-code-sessions`: klasik kurulum.
///
/// Bunları tek tek yazmak yerine aranır: `%LOCALAPPDATA%` ve `%APPDATA%`'da adında "claude" ya da
/// "anthropic" geçen her klasörün kendisi ve bir alt seviyesi, Store paketlerinde de
/// `LocalCache\{Roaming,Local}\*`. Böylece kurumsal sürüm (`Claude-3p`) ya da Claude'un ileride
/// yapacağı benzer bir taşıma da bulunur. Hiçbiri tutmazsa kullanıcı klasörü elle seçer (`list`'in
/// `custom` argümanı). Birden çok kök varsa `list` tek bir hesap (en son etkinliğin olduğu) seçtiği
/// için çift sayılmaz.
fn session_roots() -> Vec<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    roots_in(local.as_deref(), appdata.as_deref())
}

fn roots_in(local: Option<&Path>, appdata: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for base in [local, appdata].into_iter().flatten() {
        for dir in sub_dirs(base).into_iter().filter(|d| is_claude_named(d)) {
            roots.push(dir.join(SESSIONS_DIR));
            roots.extend(sub_dirs(&dir).into_iter().map(|d| d.join(SESSIONS_DIR)));
        }
    }
    if let Some(local) = local {
        for pkg in sub_dirs(&local.join("Packages")).into_iter().filter(|d| is_claude_named(d)) {
            for cache in ["Roaming", "Local"] {
                let dirs = sub_dirs(&pkg.join("LocalCache").join(cache));
                roots.extend(dirs.into_iter().map(|d| d.join(SESSIONS_DIR)));
            }
        }
    }
    roots.retain(|r| r.is_dir());
    roots.sort();
    roots.dedup();
    roots
}

fn is_claude_named(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|n| n.contains("claude") || n.contains("anthropic"))
}

/// Elle seçilen klasörü kayıt köküne çevirir. Kullanıcı `claude-code-sessions`'ı, onu içeren
/// klasörü ya da içindeki bir hesap/org klasörünü seçmiş olabilir; içinde gerçekten oturum kaydı
/// olan ilk aday kök sayılır.
fn resolve_custom(dir: &Path) -> Option<PathBuf> {
    let parent = dir.parent();
    let candidates = [
        Some(dir.join(SESSIONS_DIR)),
        Some(dir.to_path_buf()),
        parent.map(Path::to_path_buf),
        parent.and_then(Path::parent).map(Path::to_path_buf),
    ];
    candidates.into_iter().flatten().find(|c| has_sessions(c))
}

/// Kökün altında `<hesap>\<org>\local_*.json` düzeninde en az bir kayıt var mı.
fn has_sessions(root: &Path) -> bool {
    sub_dirs(root).iter().flat_map(|a| sub_dirs(a)).any(|org| {
        fs::read_dir(&org)
            .map(|entries| entries.flatten().any(|e| is_session_file(&e.path())))
            .unwrap_or(false)
    })
}

/// Oturumları okur. `custom` doluysa yalnız o klasöre bakılır (kullanıcının seçtiği yer);
/// boşsa kök kendiliğinden aranır (bkz. `session_roots`).
pub fn list(custom: Option<&str>) -> Result<SessionList, String> {
    let custom = custom.map(str::trim).filter(|c| !c.is_empty());
    let roots = match custom {
        Some(dir) => vec![resolve_custom(Path::new(dir)).ok_or_else(|| {
            format!("Seçilen klasörde Claude oturum kaydı yok: {dir}. ⋯ → Otomatik bul ile geri dönebilirsin.")
        })?],
        None => session_roots(),
    };
    if roots.is_empty() {
        return Err(
            "Claude oturum kayıtları bulunamadı (%LOCALAPPDATA% ve %APPDATA% altındaki Claude klasörleri tarandı). ⋯ → Oturum klasörünü seç ile yerini gösterebilirsin."
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

    fn temp_base(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("sessionboard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        base
    }

    fn sorted(mut v: Vec<PathBuf>) -> Vec<PathBuf> {
        v.sort();
        v
    }

    /// Bilinen bütün düzenler bulunur: Claude-Data (Ekim 2026 sonrası Store), eski Store paketi,
    /// klasik kurulum ve adında Claude geçen başka bir klasör (kurumsal sürüm). Verisi taşınmış
    /// (yalnız işaret dosyası kalmış) eski yer, Claude'a ait olmayan paketler ve klasörler atlanır.
    #[test]
    fn kok_klasorler_bulunur() {
        let base = temp_base("roots");
        let local = base.join("Local");
        let roaming = base.join("Roaming");
        let data = local.join("Claude-Data").join("claude_abc123").join(SESSIONS_DIR);
        let enterprise = local.join("Claude-3p").join(SESSIONS_DIR);
        let old_pkg = local.join("Packages").join("Claude_abc123").join("LocalCache").join("Roaming").join("Claude");
        let other_pkg = local.join("Packages").join("Other_xyz").join("LocalCache").join("Roaming").join("Claude");
        let unrelated = local.join("SomeTool").join(SESSIONS_DIR);
        for d in [&data, &enterprise, &other_pkg.join(SESSIONS_DIR), &unrelated] {
            fs::create_dir_all(d).unwrap();
        }
        fs::create_dir_all(&old_pkg).unwrap();
        fs::write(old_pkg.join(".user-data-left.claude_abc123"), b"").unwrap();
        fs::create_dir_all(roaming.join("Claude")).unwrap();
        let after_move = roots_in(Some(&local), Some(&roaming));

        let old_root = old_pkg.join(SESSIONS_DIR);
        let classic = roaming.join("Claude").join(SESSIONS_DIR);
        fs::create_dir_all(&old_root).unwrap();
        fs::create_dir_all(&classic).unwrap();
        let all = roots_in(Some(&local), Some(&roaming));
        let none = roots_in(None, None);
        let _ = fs::remove_dir_all(&base);

        assert_eq!(after_move, sorted(vec![data.clone(), enterprise.clone()]));
        assert_eq!(all, sorted(vec![data, enterprise, old_root, classic]));
        assert!(none.is_empty());
    }

    /// Elle seçilen klasör: kök, onu içeren klasör ya da içindeki hesap/org klasörü seçilmiş olsa
    /// da aynı köke çözülür; içinde kayıt olmayan klasör reddedilir.
    #[test]
    fn elle_secilen_klasor_cozulur() {
        let base = temp_base("custom");
        let parent = base.join("Claude-Data").join("claude_abc123");
        let root = parent.join(SESSIONS_DIR);
        let org = root.join("hesap").join("org");
        fs::create_dir_all(&org).unwrap();
        fs::write(org.join("local_3f2a9c4e.json"), b"{}").unwrap();
        let empty = base.join("Bos");
        fs::create_dir_all(empty.join(SESSIONS_DIR)).unwrap();

        let picks = [&parent, &root, &root.join("hesap"), &org].map(|p| resolve_custom(p));
        let rejected = resolve_custom(&empty);
        let _ = fs::remove_dir_all(&base);

        for got in picks {
            assert_eq!(got.as_ref(), Some(&root));
        }
        assert_eq!(rejected, None);
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
        let list = list(None).expect("kayıtlar okunamadı");
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
