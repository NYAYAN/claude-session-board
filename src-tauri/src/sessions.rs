//! Claude masaüstü uygulamasının oturum kayıtlarını okur.
//!
//! Kaynak: `<kök>\<hesap>\<org>\local_*.json`; kök için bkz. `session_roots`.
//! Bu dosyalar Claude'un iç (belgelenmemiş) biçimidir: yalnız OKUNUR, asla yazılmaz.
//! Biçim bir Claude güncellemesinde değişirse en kötü ihtimalle liste boş/eksik gelir;
//! Claude'un kendisi etkilenmez.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// `workspace`'in karşılaştırma anahtarı (Windows'ta ve macOS'un varsayılan APFS biçiminde
    /// yollar büyük/küçük harfe duyarsız).
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
///
/// macOS'ta kayıtlar `~/Library/Application Support/Claude/claude-code-sessions` altında, düzen
/// Windows'takiyle aynı. Aynı arama `%APPDATA%` yerine Application Support'ta yapılır; Store paketi
/// olmadığı için `Packages` taraması yok.
fn session_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
        roots_in(local.as_deref(), appdata.as_deref())
    }
    #[cfg(not(windows))]
    {
        roots_in(None, data_dir().as_deref())
    }
}

/// Claude'un verisinin durduğu üst klasör; klasör seçme penceresi de buradan açılır.
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }
}

/// Kullanıcı profilinin kökü (`%USERPROFILE%`, macOS'ta `$HOME`).
pub fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// "Bulunamadı" iletisinde nerelere bakıldığı.
#[cfg(windows)]
const SEARCHED: &str = "%LOCALAPPDATA% ve %APPDATA% altındaki Claude klasörleri tarandı";
#[cfg(not(windows))]
const SEARCHED: &str = "~/Library/Application Support altındaki Claude klasörleri tarandı";

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
        return Err(format!(
            "Claude oturum kayıtları bulunamadı ({SEARCHED}). ⋯ → Oturum klasörünü seç ile yerini gösterebilirsin."
        ));
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
    let ids: HashSet<&str> = sessions
        .iter()
        .filter_map(|s| s.cli_session_id.as_deref())
        .collect();
    let times = transcript_times(&ids);
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

/// Okunmuş bir konuşma dosyası: boyut, değiştirilme zamanı, son mesajın zamanı.
type Tail = (u64, SystemTime, Option<f64>);

/// Okunan konuşma dosyaları, yola göre. Dosya değişmedikçe yeniden okunmaz; yenileme her 20
/// saniyede bir yüzlerce dosyaya bakıyor.
static TAILS: Mutex<BTreeMap<PathBuf, Tail>> = Mutex::new(BTreeMap::new());

/// Konuşmalardaki son mesajın zamanı: `~\.claude\projects\<proje>\<cliSessionId>.jsonl` → ms.
/// Yalnız `ids`'teki konuşmalara bakılır. Profil kökü sanallaştırılmaz; Claude'un içinden ve
/// dışından aynı klasör görünür.
///
/// Dosyanın değiştirilme zamanı kullanılamaz: Claude kapanmış konuşmaların sonuna da sohbet dışı
/// satırlar (`cost-state`, `last-prompt`, `custom-title` …) ekliyor; aylar önceki oturumlar "10 dk"
/// görünüyordu. Bu yüzden dosyanın sonundaki son `user`/`assistant` satırının zamanı okunur.
fn transcript_times(ids: &HashSet<&str>) -> HashMap<String, f64> {
    let mut times = HashMap::new();
    let Some(home) = home_dir() else {
        return times;
    };
    let mut cache = TAILS.lock().unwrap_or_else(|e| e.into_inner());
    let mut seen = BTreeMap::new();
    for project in sub_dirs(&home.join(".claude").join("projects")) {
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
            if !ids.contains(stem) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let (len, modified) = (meta.len(), meta.modified().unwrap_or(UNIX_EPOCH));
            let at = match cache.get(&path) {
                Some(&(l, m, at)) if l == len && m == modified => at,
                _ => last_message_time(&path, len),
            };
            if let Some(t) = at {
                let best = times.entry(stem.to_string()).or_insert(t);
                *best = best.max(t);
            }
            seen.insert(path, (len, modified, at));
        }
    }
    // Silinen ya da artık listede olmayan konuşmalar önbellekten düşer.
    *cache = seen;
    times
}

/// Konuşmanın son `user`/`assistant` satırının zamanı (ms). Dosya yüzlerce MB olabilir; sondan
/// geriye önce 64 KB okunur (son mesaj neredeyse hep oradadır), bulunamazsa 1 MB, sonra 4 MB.
fn last_message_time(path: &Path, len: u64) -> Option<f64> {
    let mut file = fs::File::open(path).ok()?;
    for window in [64 << 10, 1 << 20, 4 << 20] {
        let start = len.saturating_sub(window);
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut buf = Vec::new();
        (&mut file).take(len - start).read_to_end(&mut buf).ok()?;
        // Pencere bir satırın ortasından başlıyorsa o yarım satır atlanır.
        let text = if start == 0 {
            &buf[..]
        } else {
            match buf.iter().position(|&b| b == b'\n') {
                Some(nl) => &buf[nl + 1..],
                None => continue,
            }
        };
        if let Some(t) = text.rsplit(|&b| b == b'\n').find_map(message_time) {
            return Some(t);
        }
        if start == 0 {
            return None;
        }
    }
    None
}

/// Satır bir konuşma mesajıysa (`user`/`assistant`) zamanı (ms).
fn message_time(line: &[u8]) -> Option<f64> {
    #[derive(Deserialize)]
    struct Entry {
        #[serde(rename = "type")]
        kind: Option<String>,
        timestamp: Option<String>,
    }
    let entry: Entry = serde_json::from_slice(line).ok()?;
    match entry.kind.as_deref()? {
        "user" | "assistant" => parse_timestamp(&entry.timestamp?),
        _ => None,
    }
}

/// `2026-11-08T08:25:38.123Z` (Claude Code'un yazdığı UTC biçimi) → ms; başka biçim `None`.
fn parse_timestamp(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[b.len() - 1] != b'Z'
    {
        return None;
    }
    let num = |from: usize, to: usize| s.get(from..to)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    // Saniyenin kesri (".123"), varsa.
    let frac = match s.get(19..s.len() - 1)? {
        "" => 0.0,
        f if f.starts_with('.') => f.parse::<f64>().ok()?,
        _ => return None,
    };
    // Gregoryen tarih → 1970-01-01'den bu yana gün (H. Hinnant, days_from_civil).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((mo + 9) % 12) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some((days * 86_400 + h * 3_600 + mi * 60 + sec) as f64 * 1000.0 + (frac * 1000.0).round())
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

/// Yol ayırıcısı. macOS'ta `\` geçerli bir dosya adı karakteri olduğu için çevrilmez.
#[cfg(windows)]
const SEP: char = '\\';
#[cfg(not(windows))]
const SEP: char = '/';

/// Yolu normalleştirir ve worktree'yi reposuna bağlar:
/// `...\Repo\.claude\worktrees\ad` → `...\Repo` (macOS'ta `/` ile).
pub fn workspace_root(path: &str) -> String {
    let normalized = if cfg!(windows) {
        path.trim().replace('/', "\\")
    } else {
        path.trim().to_string()
    };
    let trimmed = normalized.trim_end_matches(SEP);
    let marker = format!("{SEP}.claude{SEP}worktrees{SEP}");
    // ASCII küçültme bayt uzunluğunu korur → bulunan indeks orijinal dizgede de geçerli.
    match trimmed.to_ascii_lowercase().find(&marker) {
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

    #[cfg(windows)]
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

    #[cfg(not(windows))]
    #[test]
    fn worktree_reposuna_baglanir() {
        assert_eq!(
            workspace_root("/Users/ali/Work - Local/my-app/.claude/worktrees/brave-otter-1a2b3c"),
            "/Users/ali/Work - Local/my-app"
        );
        assert_eq!(workspace_root("/Users/ali/App/.Claude/Worktrees/x/sub"), "/Users/ali/App");
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

    #[cfg(windows)]
    #[test]
    fn duz_klasor_normallesir() {
        assert_eq!(workspace_root(r"C:\Work\My Project\"), r"C:\Work\My Project");
        assert_eq!(workspace_root("C:/Work/Şube/Proje"), r"C:\Work\Şube\Proje");
    }

    /// macOS: sondaki `/` atılır, `\` dosya adının parçası olduğu için olduğu gibi kalır.
    #[cfg(not(windows))]
    #[test]
    fn duz_klasor_normallesir() {
        assert_eq!(workspace_root("/Users/ali/My Project/"), "/Users/ali/My Project");
        assert_eq!(workspace_root("/Users/ali/Şube/Proje"), "/Users/ali/Şube/Proje");
        assert_eq!(workspace_root(r"/Users/ali/a\b"), r"/Users/ali/a\b");
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
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as f64;
        for s in list.sessions.iter().filter(|s| !s.archived).take(8) {
            let minutes = (now - s.last_activity_at) / 60_000.0;
            println!("  {minutes:>8.0} dk | {} | {}", s.title, s.workspace);
        }
        assert!(!list.sessions.is_empty());
        assert!(list.sessions.iter().all(|s| is_valid_id(&s.id)));
    }

    #[test]
    fn zaman_damgasi_okunur() {
        assert_eq!(parse_timestamp("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(parse_timestamp("2026-11-08T08:25:38.123Z"), Some(1_794_126_338_123.0));
        assert_eq!(parse_timestamp("2000-02-29T23:59:59Z"), Some(951_868_799_000.0));
        assert_eq!(parse_timestamp("2026-11-08T08:25:38.123+03:00"), None);
        assert_eq!(parse_timestamp("2026-13-08T08:25:38Z"), None);
        assert_eq!(parse_timestamp("dün"), None);
    }

    /// Son mesaj, arkasından gelen sohbet dışı satırlara (Claude kapanmış konuşmalara da ekliyor),
    /// 64 KB'tan uzun satırlara ve yarım yazılmış son satıra rağmen bulunur.
    #[test]
    fn son_mesaj_zamani_okunur() {
        let base = temp_base("tail");
        fs::create_dir_all(&base).unwrap();
        let blob = "x".repeat(200 << 10);
        let lines = [
            r#"{"type":"user","timestamp":"2026-11-01T10:00:00.000Z","message":{"role":"user"}}"#.to_string(),
            r#"{"type":"assistant","timestamp":"2026-11-01T10:00:05.500Z","message":{"content":[{"type":"text"}]}}"#.to_string(),
            r#"{"type":"system","timestamp":"2026-11-01T10:00:06.000Z"}"#.to_string(),
            format!(r#"{{"type":"api-request-blob","data":"{blob}"}}"#),
            r#"{"type":"last-prompt","lastPrompt":"selam"}"#.to_string(),
            r#"{"type":"cost-state","timestamp":"2026-11-08T09:00:00.000Z"}"#.to_string(),
            r#"{"type":"user","timestamp":"2026-11-08T09:"#.to_string(),
        ];
        let path = base.join("a.jsonl");
        fs::write(&path, lines.join("\n")).unwrap();
        let empty = base.join("b.jsonl");
        fs::write(
            &empty,
            "{\"type\":\"custom-title\",\"customTitle\":\"x\"}\n",
        )
        .unwrap();

        let got = last_message_time(&path, fs::metadata(&path).unwrap().len());
        let none = last_message_time(&empty, fs::metadata(&empty).unwrap().len());
        let _ = fs::remove_dir_all(&base);

        assert_eq!(got, parse_timestamp("2026-11-01T10:00:05.500Z"));
        assert_eq!(none, None);
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
