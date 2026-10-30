// 桌面端可选的 GitHub 同步：OAuth Device Flow 登录，把当前导图以文本推到私密 Gist。
// 不内置 client_secret。Client ID 来自运行时环境变量、github.json 或编译期环境变量。

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Map, Value};

const SCOPE: &str = "gist read:user";
const USER_AGENT: &str = "Mind (https://github.com/phaethix/mind-map)";
const API_VERSION: &str = "2022-11-28";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const API_ROOT: &str = "https://api.github.com";
const DEFAULT_VERIFICATION: &str = "https://github.com/login/device";
const GIST_PREFIX: &str = "Mind: ";
/// GitHub 对 Gist 文本大约按 1MB 截断，超过就直接拒绝，避免传上去变成残缺内容。
const GIST_BYTE_LIMIT: usize = 1_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthStart {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval_secs: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthPoll {
    pub status: String,
    pub interval_secs: u64,
    pub login: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub configured: bool,
    pub signed_in: bool,
    pub login: String,
    pub linked: bool,
    pub gist_id: String,
    pub html_url: String,
    pub format: String,
    pub file_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PushResult {
    pub gist_id: String,
    pub html_url: String,
    pub file_name: String,
    pub created: bool,
    pub recreated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GistSummary {
    pub id: String,
    pub description: String,
    pub html_url: String,
    pub updated_at: String,
    pub file_name: String,
    pub linked_path: String,
}

pub(crate) struct FetchedPull {
    pub gist_id: String,
    pub html_url: String,
    pub description: String,
    pub file_name: String,
    pub format: String,
    pub document: Value,
    pub target: PathBuf,
    pub target_exists: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GistFormat {
    Json,
    Markdown,
}

impl GistFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "markdown",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Json => ".mind.json",
            Self::Markdown => ".mind.md",
        }
    }
}

struct PendingAuth {
    generation: u64,
    client_id: String,
    device_code: String,
    interval: Duration,
    expires_at: SystemTime,
    last_poll: SystemTime,
    token_url: String,
    api_root: String,
}

struct Gate {
    pending: Option<PendingAuth>,
    generation: u64,
}

static GATE: Mutex<Gate> = Mutex::new(Gate {
    pending: None,
    generation: 0,
});

fn lock_gate() -> Result<std::sync::MutexGuard<'static, Gate>, String> {
    GATE.lock()
        .map_err(|_| "GitHub 登录状态异常，请重试".to_string())
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("无法连接 GitHub：{}", clip(&error.to_string(), 300)))
}

pub(crate) fn resolve_client_id(
    stored: &str,
    env_value: Option<&str>,
    compiled: Option<&str>,
) -> String {
    for candidate in [env_value, Some(stored), compiled].into_iter().flatten() {
        let trimmed = candidate.trim();
        if valid_client_id(trimmed) {
            return trimmed.to_string();
        }
    }
    String::new()
}

fn configured_client_id(stored: &str) -> String {
    resolve_client_id(
        stored,
        std::env::var("MIND_GITHUB_CLIENT_ID").ok().as_deref(),
        option_env!("MIND_GITHUB_CLIENT_ID"),
    )
}

fn valid_client_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !value.starts_with("ghp_")
        && !value.starts_with("gho_")
        && !value.starts_with("ghu_")
        && !value.starts_with("ghs_")
        && !value.starts_with("github_pat_")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
}

fn valid_gist_id(value: &str) -> bool {
    (1..=64).contains(&value.len()) && value.chars().all(|ch| ch.is_ascii_alphanumeric())
}

pub(crate) fn allowed_github_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| "链接无效".to_string())?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("链接无效".to_string());
    }
    if let Some(port) = parsed.port() {
        if port != 443 {
            return Err("链接无效".to_string());
        }
    }
    let host = parsed.host_str().unwrap_or("");
    let path = parsed.path();
    let allowed = (host == "github.com" && path.starts_with("/login/device"))
        || host == "gist.github.com"
        || host == "gist.githubusercontent.com";
    if !allowed {
        return Err("只能打开 GitHub 的授权页或 Gist 链接".to_string());
    }
    Ok(())
}

fn auth_path(app_dir: &Path) -> PathBuf {
    app_dir.join("github.json")
}

fn sync_path(app_dir: &Path) -> PathBuf {
    app_dir.join("github-sync.json")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AuthFile {
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    login: String,
    #[serde(default)]
    scope: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SyncFile {
    #[serde(default)]
    documents: BTreeMap<String, DocLink>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocLink {
    gist_id: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    updated_at: String,
}

fn read_auth(app_dir: &Path) -> Result<AuthFile, String> {
    let _gate = lock_gate()?;
    read_auth_unlocked(app_dir)
}

fn read_auth_unlocked(app_dir: &Path) -> Result<AuthFile, String> {
    let path = auth_path(app_dir);
    if !path.exists() {
        return Ok(AuthFile::default());
    }
    let text =
        fs::read_to_string(&path).map_err(|error| format!("无法读取 GitHub 登录信息：{error}"))?;
    serde_json::from_str(&text).map_err(|_| "GitHub 登录信息无法解析".to_string())
}

fn write_auth_unlocked(app_dir: &Path, auth: &AuthFile) -> Result<(), String> {
    let path = auth_path(app_dir);
    let text = serde_json::to_string_pretty(auth).map_err(|error| error.to_string())?;
    write_private(&path, &(text + "\n"))
}

fn clear_token_unlocked(app_dir: &Path) -> Result<(), String> {
    let mut auth = read_auth_unlocked(app_dir)?;
    auth.access_token.clear();
    auth.login.clear();
    auth.scope.clear();
    if auth.client_id.trim().is_empty() && !auth_path(app_dir).exists() {
        return Ok(());
    }
    write_auth_unlocked(app_dir, &auth)
}

fn read_sync(app_dir: &Path) -> Result<SyncFile, String> {
    let _gate = lock_gate()?;
    read_sync_unlocked(app_dir)
}

fn read_sync_unlocked(app_dir: &Path) -> Result<SyncFile, String> {
    let path = sync_path(app_dir);
    if !path.exists() {
        return Ok(SyncFile::default());
    }
    let text =
        fs::read_to_string(&path).map_err(|error| format!("无法读取 GitHub 同步记录：{error}"))?;
    serde_json::from_str(&text).map_err(|_| "GitHub 同步记录无法解析".to_string())
}

fn write_sync(app_dir: &Path, sync: &SyncFile) -> Result<(), String> {
    let _gate = lock_gate()?;
    write_sync_unlocked(app_dir, sync)
}

fn write_sync_unlocked(app_dir: &Path, sync: &SyncFile) -> Result<(), String> {
    let path = sync_path(app_dir);
    let text = serde_json::to_string_pretty(sync).map_err(|error| error.to_string())?;
    write_private(&path, &(text + "\n"))
}

fn write_private(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法保存 GitHub 配置：{error}"))?;
    }
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("无法保存 GitHub 配置：{error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
    }
    file.write_all(text.as_bytes())
        .map_err(|error| format!("无法保存 GitHub 配置：{error}"))?;
    Ok(())
}

fn remember_link(app_dir: &Path, path_key: &str, link: DocLink) -> Result<(), String> {
    let mut sync = read_sync(app_dir)?;
    sync.documents
        .retain(|_, item| item.gist_id != link.gist_id);
    sync.documents.insert(path_key.to_string(), link);
    write_sync(app_dir, &sync)
}

pub(crate) fn remap_links(app_dir: &Path, from: &Path, to: &Path) {
    let Ok(mut sync) = read_sync(app_dir) else {
        return;
    };
    let mut next = BTreeMap::new();
    let mut changed = false;
    for (key, link) in sync.documents {
        if let Some(mapped) = remap_key(&key, from, to) {
            if mapped != key {
                changed = true;
            }
            next.insert(mapped, link);
        } else {
            next.insert(key, link);
        }
    }
    if changed {
        sync.documents = next;
        let _ = write_sync(app_dir, &sync);
    }
}

pub(crate) fn forget_links(app_dir: &Path, prefix: &Path) {
    let Ok(mut sync) = read_sync(app_dir) else {
        return;
    };
    let before = sync.documents.len();
    sync.documents
        .retain(|key, _| !path_under(Path::new(key), prefix));
    if sync.documents.len() != before {
        let _ = write_sync(app_dir, &sync);
    }
}

fn remap_key(key: &str, from: &Path, to: &Path) -> Option<String> {
    let path = PathBuf::from(key);
    // 文件本身被改名时 strip_prefix 得到空路径，join 会多出一个结尾斜杠，
    // 同步记录的键就会对不上后来的打开路径。
    if path == from {
        return Some(to.to_string_lossy().into_owned());
    }
    let rest = path.strip_prefix(from).ok()?;
    if rest.as_os_str().is_empty() {
        return Some(to.to_string_lossy().into_owned());
    }
    Some(to.join(rest).to_string_lossy().into_owned())
}

fn path_under(path: &Path, prefix: &Path) -> bool {
    path == prefix || path.starts_with(prefix)
}

fn parse_format(format: &str) -> Result<GistFormat, String> {
    match format {
        "json" => Ok(GistFormat::Json),
        "markdown" | "md" => Ok(GistFormat::Markdown),
        _ => Err("同步格式无效".to_string()),
    }
}

fn sanitize_stem(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    let trimmed = out.trim().trim_matches('.').to_string();
    let trimmed = if trimmed.is_empty() {
        "mind".to_string()
    } else {
        trimmed
    };
    trimmed.chars().take(80).collect()
}

fn stem_from_path(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("mind");
    let stem = name
        .strip_suffix(".xmind")
        .or_else(|| name.strip_suffix(".smm"))
        .or_else(|| name.strip_suffix(".json"))
        .unwrap_or(name);
    sanitize_stem(stem)
}

fn gist_file_name(stem: &str, format: GistFormat) -> String {
    format!("{}{}", sanitize_stem(stem), format.suffix())
}

fn gist_description(stem: &str) -> String {
    let mut text = format!("{GIST_PREFIX}{}", sanitize_stem(stem));
    if text.chars().count() > 250 {
        text = text.chars().take(250).collect();
    }
    text
}

fn stem_from_gist(description: &str, file_name: &str) -> String {
    let from_description = description
        .trim()
        .strip_prefix(GIST_PREFIX)
        .map(str::trim)
        .unwrap_or("");
    if !from_description.is_empty() {
        return sanitize_stem(from_description);
    }
    let name = file_name
        .strip_suffix(".mind.json")
        .or_else(|| file_name.strip_suffix(".mind.md"))
        .unwrap_or(file_name);
    sanitize_stem(name)
}

fn plain_text(value: &Value) -> String {
    let raw = value.as_str().unwrap_or("");
    let mut text = String::new();
    let mut in_tag = false;
    for ch in raw.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            if ch == '\n' || ch == '\r' {
                text.push(' ');
            } else {
                text.push(ch);
            }
        }
    }
    text.trim().to_string()
}

fn render_json(document: &Value) -> Result<String, String> {
    if !document.get("root").is_some() {
        return Err("导图数据无效".to_string());
    }
    let mut text = serde_json::to_string_pretty(document).map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

fn render_markdown(document: &Value) -> Result<String, String> {
    let root = document.get("root").ok_or("导图数据无效")?;
    let mut out = String::new();
    write_markdown(root, 1, &mut out)?;
    if out.trim().is_empty() {
        return Err("导图内容是空的".to_string());
    }
    Ok(out)
}

fn write_markdown(node: &Value, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > 64 {
        return Err("导图层级过深，无法导出为 Markdown".to_string());
    }
    let data = node.get("data").cloned().unwrap_or_else(|| json!({}));
    let text = plain_text(data.get("text").unwrap_or(&Value::Null));
    if depth <= 6 {
        for _ in 0..depth {
            out.push('#');
        }
        out.push(' ');
        out.push_str(&text);
        out.push_str("\n\n");
    } else {
        for _ in 0..(depth - 6) {
            out.push_str("   ");
        }
        out.push_str("* ");
        out.push_str(&text);
        out.push_str("\n\n");
    }
    if let Some(note) = data.get("note").and_then(|value| value.as_str()) {
        if !note.trim().is_empty() {
            for line in note.lines() {
                out.push_str("> ");
                out.push_str(line.trim_end_matches('\r'));
                out.push('\n');
            }
            out.push('\n');
        }
    }
    if let Some(children) = node.get("children").and_then(|value| value.as_array()) {
        for child in children {
            write_markdown(child, depth + 1, out)?;
        }
    }
    Ok(())
}

struct MdItem {
    depth: usize,
    text: String,
    note: String,
}

fn parse_markdown(markdown: &str) -> Result<Value, String> {
    let mut items: Vec<MdItem> = Vec::new();
    for line in markdown.lines() {
        if let Some(text) = quote_text(line) {
            if let Some(last) = items.last_mut() {
                if !last.note.is_empty() {
                    last.note.push('\n');
                }
                last.note.push_str(&text);
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some((depth, text)) = heading(line).or_else(|| list_item(line)) {
            items.push(MdItem {
                depth,
                text: text.to_string(),
                note: String::new(),
            });
            continue;
        }
        if let Some(last) = items.last_mut() {
            if !last.note.is_empty() {
                last.note.push('\n');
            }
            last.note.push_str(line.trim());
        }
    }
    if items.is_empty() {
        return Err("无法从 Gist 解析导图".to_string());
    }
    let mut index = 0;
    let mut roots = build_level(&items, &mut index, 0);
    if roots.is_empty() {
        return Err("无法从 Gist 解析导图".to_string());
    }
    let root = if roots.len() == 1 {
        roots.remove(0)
    } else {
        json!({
            "data": { "text": "Mind" },
            "children": roots
        })
    };
    Ok(wrap_document(root))
}

fn quote_text(line: &str) -> Option<String> {
    let trimmed = line.trim_end();
    if let Some(rest) = trimmed.strip_prefix("> ") {
        return Some(rest.to_string());
    }
    if trimmed == ">" {
        return Some(String::new());
    }
    None
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = line.get(hashes..)?;
    if !rest.starts_with(' ') {
        return None;
    }
    Some((hashes, rest.trim()))
}

fn list_item(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_end();
    let indent = trimmed.chars().take_while(|ch| *ch == ' ').count();
    let after = trimmed.get(indent..)?;
    let marker_len = ["* ", "- ", "+ "]
        .iter()
        .find(|marker| after.starts_with(*marker))
        .map(|marker| marker.len())?;
    let text = after[marker_len..].trim();
    Some((6 + indent / 3, text))
}

fn build_level(items: &[MdItem], index: &mut usize, parent_depth: usize) -> Vec<Value> {
    let mut children = Vec::new();
    while *index < items.len() && items[*index].depth > parent_depth {
        let depth = items[*index].depth;
        let mut data = json!({ "text": items[*index].text });
        if !items[*index].note.is_empty() {
            data["note"] = json!(items[*index].note);
        }
        *index += 1;
        let kids = build_level(items, index, depth);
        children.push(json!({ "data": data, "children": kids }));
    }
    children
}

fn wrap_document(root: Value) -> Value {
    json!({
        "layout": "logicalStructure",
        "root": root,
        "theme": { "template": "classic7", "config": {} },
        "view": null
    })
}

fn normalize_gist_document(value: Value) -> Result<Value, String> {
    if !value.is_object() {
        return Err("无法解析 Gist 里的导图".to_string());
    }
    if value.get("root").is_some() {
        return Ok(value);
    }
    if value.get("data").is_some() {
        return Ok(wrap_document(value));
    }
    Err("无法解析 Gist 里的导图".to_string())
}

fn parse_named_content(name: &str, content: &str) -> Result<Value, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Gist 里的导图是空的".to_string());
    }
    if name.ends_with(".mind.json") || trimmed.starts_with('{') || trimmed.starts_with('[') {
        let value: Value =
            serde_json::from_str(trimmed).map_err(|_| "无法解析 Gist 里的导图 JSON".to_string())?;
        return normalize_gist_document(value);
    }
    parse_markdown(content)
}

fn ensure_gist_size(content: &str) -> Result<(), String> {
    if content.len() > GIST_BYTE_LIMIT {
        Err(too_large_message(Some(content.len())))
    } else {
        Ok(())
    }
}

fn too_large_message(bytes: Option<usize>) -> String {
    match bytes {
        Some(bytes) => format!(
            "导图内容超过 Gist 约 1MB 的限制（当前 {}），请精简后再同步。",
            format_size(bytes)
        ),
        None => "导图内容超过 Gist 约 1MB 的限制，请精简后再同步。".to_string(),
    }
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1000 {
        format!("{} KB", bytes.div_ceil(1000))
    } else {
        format!("{bytes} B")
    }
}

fn render_document(document: &Value, format: GistFormat) -> Result<String, String> {
    let content = match format {
        GistFormat::Json => render_json(document)?,
        GistFormat::Markdown => render_markdown(document)?,
    };
    ensure_gist_size(&content)?;
    Ok(content)
}

fn create_body(description: &str, file_name: &str, content: &str) -> Value {
    json!({
        "description": description,
        "public": false,
        "files": {
            file_name: { "content": content }
        }
    })
}

fn update_body(description: &str, previous: &[String], file_name: &str, content: &str) -> Value {
    let mut files = Map::new();
    for name in previous {
        if name != file_name && (name.ends_with(".mind.json") || name.ends_with(".mind.md")) {
            files.insert(name.clone(), Value::Null);
        }
    }
    files.insert(file_name.to_string(), json!({ "content": content }));
    json!({
        "description": description,
        "public": false,
        "files": files
    })
}

fn format_from_name(name: &str) -> &'static str {
    if name.ends_with(".mind.md") {
        "markdown"
    } else {
        "json"
    }
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let time = secs % 86_400;
    let hour = time / 3600;
    let minute = (time % 3600) / 60;
    let second = time % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    (year as i32, month as u32, day as u32)
}

fn next_interval(current: Duration, hinted: Option<u64>) -> Duration {
    let bumped = current.saturating_add(Duration::from_secs(5));
    let hinted = hinted.map(Duration::from_secs).unwrap_or(Duration::ZERO);
    let next = if hinted > bumped { hinted } else { bumped };
    if next < Duration::from_secs(5) {
        Duration::from_secs(5)
    } else if next > Duration::from_secs(120) {
        Duration::from_secs(120)
    } else {
        next
    }
}

fn clip(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn clip_public(body: &str) -> String {
    if body.contains("access_token") || body.contains("gho_") || body.contains("github_pat_") {
        return "GitHub 返回了无法显示的内容".to_string();
    }
    clip(body, 180)
}

enum ApiError {
    NotFound,
    Unauthorized,
    TooLarge,
    Other(String),
}

impl ApiError {
    fn into_message(self) -> String {
        match self {
            Self::NotFound => "找不到这个 Gist，可能已被删除".to_string(),
            Self::Unauthorized => "GitHub 登录已失效，请重新登录".to_string(),
            Self::TooLarge => too_large_message(None),
            Self::Other(message) => message,
        }
    }
}

fn classify_status(status: u16, body: &str) -> Result<(), ApiError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    if status == 401 || (status == 403 && body.to_ascii_lowercase().contains("bad credentials")) {
        return Err(ApiError::Unauthorized);
    }
    if status == 404 {
        return Err(ApiError::NotFound);
    }
    if status == 413
        || ((status == 422 || status == 403)
            && (body.to_ascii_lowercase().contains("too large")
                || body.to_ascii_lowercase().contains("size limit")))
    {
        return Err(ApiError::TooLarge);
    }
    if (300..400).contains(&status) {
        return Err(ApiError::Other("GitHub 返回了意外的重定向".to_string()));
    }
    if status == 403 && body.to_ascii_lowercase().contains("rate limit") {
        return Err(ApiError::Other(
            "GitHub 请求过于频繁，请稍后再试".to_string(),
        ));
    }
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(|item| item.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| clip_public(body));
    Err(ApiError::Other(format!("GitHub 返回 {status}：{message}")))
}

async fn send(builder: reqwest::RequestBuilder) -> Result<(u16, String), ApiError> {
    let response = builder.send().await.map_err(|error| {
        ApiError::Other(format!(
            "无法连接 GitHub：{}",
            clip(&error.to_string(), 300)
        ))
    })?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(|error| {
        ApiError::Other(format!(
            "无法读取 GitHub 响应：{}",
            clip(&error.to_string(), 300)
        ))
    })?;
    Ok((status, body))
}

fn api_builder(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    token: &str,
) -> reqwest::RequestBuilder {
    client
        .request(method, url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(token)
}

fn join_api(root: &str, path: &str) -> String {
    format!("{}{}", root.trim_end_matches('/'), path)
}

fn verification_uri(value: &Value) -> String {
    let complete = value
        .get("verification_uri_complete")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let plain = value
        .get("verification_uri")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    if allowed_github_url(complete).is_ok() {
        return complete.to_string();
    }
    if allowed_github_url(plain).is_ok() {
        return plain.to_string();
    }
    DEFAULT_VERIFICATION.to_string()
}

fn seconds_until(deadline: SystemTime) -> u64 {
    deadline
        .duration_since(SystemTime::now())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn interval_secs(interval: Duration) -> u64 {
    interval.as_secs().max(1)
}

async fn request_device_code(
    client: &reqwest::Client,
    device_url: &str,
    client_id: &str,
) -> Result<Value, String> {
    let (status, body) = send(
        client
            .post(device_url)
            .header("Accept", "application/json")
            .json(&json!({
                "client_id": client_id,
                "scope": SCOPE
            })),
    )
    .await
    .map_err(ApiError::into_message)?;
    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    if let Some(error) = value.get("error").and_then(|item| item.as_str()) {
        let description = value
            .get("error_description")
            .and_then(|item| item.as_str())
            .unwrap_or(error);
        return Err(format!("GitHub 授权失败：{description}"));
    }
    if let Err(error) = classify_status(status, &body) {
        return Err(error.into_message());
    }
    if value
        .get("device_code")
        .and_then(|item| item.as_str())
        .is_none()
        || value
            .get("user_code")
            .and_then(|item| item.as_str())
            .is_none()
    {
        return Err("GitHub 没有返回验证码".to_string());
    }
    Ok(value)
}

enum TokenPoll {
    Pending,
    SlowDown { interval: Option<u64> },
    Approved { access_token: String, scope: String },
}

async fn request_token(
    client: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    device_code: &str,
) -> Result<TokenPoll, String> {
    let (status, body) = send(
        client
            .post(token_url)
            .header("Accept", "application/json")
            .json(&json!({
                "client_id": client_id,
                "device_code": device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            })),
    )
    .await
    .map_err(ApiError::into_message)?;
    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    if let Some(error) = value.get("error").and_then(|item| item.as_str()) {
        return match error {
            "authorization_pending" => Ok(TokenPoll::Pending),
            "slow_down" => Ok(TokenPoll::SlowDown {
                interval: value.get("interval").and_then(|item| item.as_u64()),
            }),
            "expired_token" => Err("验证码已过期，请重新登录".to_string()),
            "access_denied" => Err("已取消 GitHub 授权".to_string()),
            other => {
                let description = value
                    .get("error_description")
                    .and_then(|item| item.as_str())
                    .unwrap_or(other);
                Err(format!("GitHub 授权失败：{description}"))
            }
        };
    }
    if let Err(error) = classify_status(status, &body) {
        return Err(error.into_message());
    }
    let access_token = value
        .get("access_token")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    if access_token.is_empty() {
        return Err("GitHub 没有返回访问令牌".to_string());
    }
    Ok(TokenPoll::Approved {
        access_token,
        scope: value
            .get("scope")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

async fn fetch_login(
    client: &reqwest::Client,
    api_root: &str,
    token: &str,
) -> Result<String, ApiError> {
    let (status, body) = send(api_builder(
        client,
        reqwest::Method::GET,
        &join_api(api_root, "/user"),
        token,
    ))
    .await?;
    classify_status(status, &body)?;
    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    Ok(value
        .get("login")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string())
}

struct RemoteGist {
    id: String,
    html_url: String,
    description: String,
    updated_at: String,
    files: Map<String, Value>,
}

fn parse_remote_gist(value: &Value) -> Result<RemoteGist, ApiError> {
    let id = value
        .get("id")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    if !valid_gist_id(&id) {
        return Err(ApiError::Other("Gist 编号无效".to_string()));
    }
    let files = value
        .get("files")
        .and_then(|item| item.as_object())
        .cloned()
        .unwrap_or_default();
    Ok(RemoteGist {
        id,
        html_url: value
            .get("html_url")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
        description: value
            .get("description")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
        updated_at: value
            .get("updated_at")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
        files,
    })
}

fn is_mind_gist(description: &str, files: &Map<String, Value>) -> bool {
    files
        .keys()
        .any(|name| name.ends_with(".mind.json") || name.ends_with(".mind.md"))
        || description.trim().starts_with(GIST_PREFIX)
}

fn preferred_file_name(files: &Map<String, Value>) -> Option<String> {
    let mut names: Vec<&String> = files.keys().collect();
    names.sort();
    names
        .iter()
        .copied()
        .find(|name| name.ends_with(".mind.json"))
        .or_else(|| {
            names
                .iter()
                .copied()
                .find(|name| name.ends_with(".mind.md"))
        })
        .or_else(|| {
            names.iter().copied().find(|name| {
                files
                    .get(*name)
                    .and_then(|file| file.get("content"))
                    .and_then(|content| content.as_str())
                    .map(|content| content.trim_start().starts_with('{'))
                    .unwrap_or(false)
            })
        })
        .or_else(|| names.first().copied())
        .cloned()
}

async fn file_text(
    client: &reqwest::Client,
    token: &str,
    file: &Value,
) -> Result<String, ApiError> {
    if file.get("truncated").and_then(|value| value.as_bool()) == Some(true) {
        return Err(ApiError::TooLarge);
    }
    if let Some(content) = file.get("content").and_then(|value| value.as_str()) {
        ensure_gist_size(content).map_err(|_| ApiError::TooLarge)?;
        return Ok(content.to_string());
    }
    let Some(raw) = file.get("raw_url").and_then(|value| value.as_str()) else {
        return Err(ApiError::Other("Gist 文件没有文本内容".to_string()));
    };
    allowed_github_url(raw).map_err(ApiError::Other)?;
    let (status, body) = send(api_builder(client, reqwest::Method::GET, raw, token)).await?;
    classify_status(status, &body)?;
    ensure_gist_size(&body).map_err(|_| ApiError::TooLarge)?;
    Ok(body)
}

async fn get_gist(
    client: &reqwest::Client,
    api_root: &str,
    token: &str,
    gist_id: &str,
) -> Result<RemoteGist, ApiError> {
    if !valid_gist_id(gist_id) {
        return Err(ApiError::Other("Gist 编号无效".to_string()));
    }
    let (status, body) = send(api_builder(
        client,
        reqwest::Method::GET,
        &join_api(api_root, &format!("/gists/{gist_id}")),
        token,
    ))
    .await?;
    classify_status(status, &body)?;
    let value: Value =
        serde_json::from_str(&body).map_err(|_| ApiError::Other("无法解析 Gist".to_string()))?;
    parse_remote_gist(&value)
}

async fn create_gist(
    client: &reqwest::Client,
    api_root: &str,
    token: &str,
    description: &str,
    file_name: &str,
    content: &str,
) -> Result<RemoteGist, ApiError> {
    let (status, body) = send(
        api_builder(
            client,
            reqwest::Method::POST,
            &join_api(api_root, "/gists"),
            token,
        )
        .json(&create_body(description, file_name, content)),
    )
    .await?;
    classify_status(status, &body)?;
    let value: Value = serde_json::from_str(&body)
        .map_err(|_| ApiError::Other("无法解析新建的 Gist".to_string()))?;
    let gist = parse_remote_gist(&value)?;
    if gist.files.get(file_name).is_none() && gist.html_url.is_empty() && gist.id.is_empty() {
        return Err(ApiError::Other("GitHub 没有返回新建的 Gist".to_string()));
    }
    Ok(gist)
}

async fn patch_gist(
    client: &reqwest::Client,
    api_root: &str,
    token: &str,
    gist_id: &str,
    body: &Value,
) -> Result<RemoteGist, ApiError> {
    let (status, response) = send(
        api_builder(
            client,
            reqwest::Method::PATCH,
            &join_api(api_root, &format!("/gists/{gist_id}")),
            token,
        )
        .json(body),
    )
    .await?;
    classify_status(status, &response)?;
    let value: Value = serde_json::from_str(&response)
        .map_err(|_| ApiError::Other("无法解析更新后的 Gist".to_string()))?;
    parse_remote_gist(&value)
}

struct Upload {
    gist: RemoteGist,
    created: bool,
    recreated: bool,
}

async fn upload_gist(
    client: &reqwest::Client,
    api_root: &str,
    token: &str,
    existing_id: Option<&str>,
    description: &str,
    file_name: &str,
    content: &str,
) -> Result<Upload, ApiError> {
    if let Some(gist_id) = existing_id.filter(|id| valid_gist_id(id)) {
        match get_gist(client, api_root, token, gist_id).await {
            Ok(existing) => {
                let previous: Vec<String> = existing.files.keys().cloned().collect();
                let body = update_body(description, &previous, file_name, content);
                let gist = patch_gist(client, api_root, token, gist_id, &body).await?;
                return Ok(Upload {
                    gist,
                    created: false,
                    recreated: false,
                });
            }
            Err(ApiError::NotFound) => {
                let gist =
                    create_gist(client, api_root, token, description, file_name, content).await?;
                return Ok(Upload {
                    gist,
                    created: true,
                    recreated: true,
                });
            }
            Err(error) => return Err(error),
        }
    }
    let gist = create_gist(client, api_root, token, description, file_name, content).await?;
    Ok(Upload {
        gist,
        created: true,
        recreated: false,
    })
}

fn finish<T>(app_dir: &Path, result: Result<T, ApiError>) -> Result<T, String> {
    match result {
        Ok(value) => Ok(value),
        Err(ApiError::Unauthorized) => {
            // 只清掉已经失效的令牌。进行中的设备码登录是另一回事，不能一起取消。
            if let Ok(_gate) = lock_gate() {
                let _ = clear_token_unlocked(app_dir);
            }
            Err(ApiError::Unauthorized.into_message())
        }
        Err(error) => Err(error.into_message()),
    }
}

fn require_token(app_dir: &Path) -> Result<String, String> {
    let auth = read_auth(app_dir)?;
    if auth.access_token.trim().is_empty() {
        return Err("请先登录 GitHub".to_string());
    }
    Ok(auth.access_token)
}

pub(crate) fn status(app_dir: &Path, path: &str) -> Result<Status, String> {
    let auth = read_auth(app_dir)?;
    let client_id = configured_client_id(&auth.client_id);
    let sync = read_sync(app_dir).unwrap_or_default();
    let link = if path.trim().is_empty() {
        None
    } else {
        sync.documents.get(path)
    };
    Ok(Status {
        configured: !client_id.is_empty(),
        signed_in: !auth.access_token.trim().is_empty(),
        login: auth.login,
        linked: link.is_some(),
        gist_id: link.map(|item| item.gist_id.clone()).unwrap_or_default(),
        html_url: link.map(|item| item.html_url.clone()).unwrap_or_default(),
        format: link.map(|item| item.format.clone()).unwrap_or_default(),
        file_name: link.map(|item| item.file_name.clone()).unwrap_or_default(),
    })
}

pub(crate) async fn auth_start(app_dir: &Path) -> Result<AuthStart, String> {
    auth_start_with(app_dir, DEVICE_CODE_URL, TOKEN_URL, API_ROOT).await
}

async fn auth_start_with(
    app_dir: &Path,
    device_url: &str,
    token_url: &str,
    api_root: &str,
) -> Result<AuthStart, String> {
    let stored = read_auth(app_dir)?.client_id;
    let client_id = configured_client_id(&stored);
    if client_id.is_empty() {
        return Err(
            "还没有配置 GitHub OAuth 应用的 Client ID。请设置 MIND_GITHUB_CLIENT_ID，或在 github.json 里填写 clientId。不要填写 client secret。"
                .to_string(),
        );
    }
    let client = http_client()?;
    let payload = request_device_code(&client, device_url, &client_id).await?;
    let user_code = payload
        .get("user_code")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    let device_code = payload
        .get("device_code")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    let expires_in = payload
        .get("expires_in")
        .and_then(|item| item.as_u64())
        .unwrap_or(900)
        .max(1);
    let interval = Duration::from_secs(
        payload
            .get("interval")
            .and_then(|item| item.as_u64())
            .unwrap_or(5)
            .clamp(5, 120),
    );
    let verification_uri = verification_uri(&payload);
    let mut gate = lock_gate()?;
    gate.generation = gate.generation.saturating_add(1);
    let generation = gate.generation;
    gate.pending = Some(PendingAuth {
        generation,
        client_id,
        device_code,
        interval,
        expires_at: SystemTime::now() + Duration::from_secs(expires_in),
        last_poll: SystemTime::now(),
        token_url: token_url.to_string(),
        api_root: api_root.to_string(),
    });
    Ok(AuthStart {
        user_code,
        verification_uri,
        expires_in,
        interval_secs: interval_secs(interval),
    })
}

pub(crate) async fn auth_poll(app_dir: &Path) -> Result<AuthPoll, String> {
    let client = http_client()?;
    let snapshot = {
        let mut gate = lock_gate()?;
        let Some(pending) = gate.pending.as_mut() else {
            return Ok(cancelled_poll());
        };
        if SystemTime::now() >= pending.expires_at {
            gate.pending = None;
            return Err("验证码已过期，请重新登录".to_string());
        }
        let elapsed = SystemTime::now()
            .duration_since(pending.last_poll)
            .unwrap_or_default();
        if elapsed < pending.interval {
            return Ok(AuthPoll {
                status: "pending".to_string(),
                interval_secs: interval_secs(pending.interval - elapsed),
                login: String::new(),
                expires_in: seconds_until(pending.expires_at),
            });
        }
        pending.last_poll = SystemTime::now();
        (
            pending.generation,
            pending.client_id.clone(),
            pending.device_code.clone(),
            pending.interval,
            pending.expires_at,
            pending.token_url.clone(),
            pending.api_root.clone(),
        )
    };
    let (generation, client_id, device_code, interval, expires_at, token_url, api_root) = snapshot;
    let polled = request_token(&client, &token_url, &client_id, &device_code).await;
    match polled {
        Ok(TokenPoll::Pending) => Ok(AuthPoll {
            status: "pending".to_string(),
            interval_secs: interval_secs(interval),
            login: String::new(),
            expires_in: seconds_until(expires_at),
        }),
        Ok(TokenPoll::SlowDown { interval: hinted }) => {
            let mut gate = lock_gate()?;
            if gate.pending.as_ref().map(|item| item.generation) != Some(generation) {
                return Ok(cancelled_poll());
            }
            if let Some(pending) = gate.pending.as_mut() {
                pending.interval = next_interval(pending.interval, hinted);
                return Ok(AuthPoll {
                    status: "pending".to_string(),
                    interval_secs: interval_secs(pending.interval),
                    login: String::new(),
                    expires_in: seconds_until(pending.expires_at),
                });
            }
            Ok(cancelled_poll())
        }
        Ok(TokenPoll::Approved {
            access_token,
            scope,
        }) => {
            let login = match fetch_login(&client, &api_root, &access_token).await {
                Ok(login) => login,
                Err(ApiError::Unauthorized) => {
                    return finish(app_dir, Err(ApiError::Unauthorized));
                }
                Err(_) => String::new(),
            };
            let mut gate = lock_gate()?;
            if gate.pending.as_ref().map(|item| item.generation) != Some(generation) {
                return Ok(cancelled_poll());
            }
            gate.pending = None;
            let mut auth = read_auth_unlocked(app_dir)?;
            auth.access_token = access_token;
            auth.login = login.clone();
            auth.scope = scope;
            write_auth_unlocked(app_dir, &auth)?;
            Ok(AuthPoll {
                status: "success".to_string(),
                interval_secs: 0,
                login,
                expires_in: 0,
            })
        }
        Err(message) => {
            let mut gate = lock_gate()?;
            if gate.pending.as_ref().map(|item| item.generation) == Some(generation) {
                gate.pending = None;
            }
            Err(message)
        }
    }
}

fn cancelled_poll() -> AuthPoll {
    AuthPoll {
        status: "cancelled".to_string(),
        interval_secs: 0,
        login: String::new(),
        expires_in: 0,
    }
}

pub(crate) fn auth_cancel() {
    if let Ok(mut gate) = lock_gate() {
        gate.pending = None;
        gate.generation = gate.generation.saturating_add(1);
    }
}

pub(crate) fn sign_out(app_dir: &Path) -> Result<(), String> {
    let mut gate = lock_gate()?;
    gate.pending = None;
    gate.generation = gate.generation.saturating_add(1);
    clear_token_unlocked(app_dir)
}

pub(crate) async fn push(
    app_dir: &Path,
    path_key: &str,
    document: &Value,
    format: &str,
) -> Result<PushResult, String> {
    push_with(app_dir, path_key, document, format, API_ROOT).await
}

async fn push_with(
    app_dir: &Path,
    path_key: &str,
    document: &Value,
    format: &str,
    api_root: &str,
) -> Result<PushResult, String> {
    let format = parse_format(format)?;
    let content = render_document(document, format)?;
    let token = require_token(app_dir)?;
    let sync = read_sync(app_dir)?;
    let existing = sync.documents.get(path_key).cloned();
    let stem = stem_from_path(Path::new(path_key));
    let file_name = gist_file_name(&stem, format);
    let description = gist_description(&stem);
    let client = http_client()?;
    let uploaded = finish(
        app_dir,
        upload_gist(
            &client,
            api_root,
            &token,
            existing.as_ref().map(|link| link.gist_id.as_str()),
            &description,
            &file_name,
            &content,
        )
        .await,
    )?;
    let html_url = if uploaded.gist.html_url.is_empty() {
        format!("https://gist.github.com/{}", uploaded.gist.id)
    } else {
        uploaded.gist.html_url
    };
    remember_link(
        app_dir,
        path_key,
        DocLink {
            gist_id: uploaded.gist.id.clone(),
            html_url: html_url.clone(),
            format: format.as_str().to_string(),
            file_name: file_name.clone(),
            updated_at: now_iso(),
        },
    )?;
    Ok(PushResult {
        gist_id: uploaded.gist.id,
        html_url,
        file_name,
        created: uploaded.created,
        recreated: uploaded.recreated,
    })
}

fn pull_target(docs_dir: &Path, gist_id: &str, stem: &str, sync: &SyncFile) -> PathBuf {
    if let Some((path, _)) = sync
        .documents
        .iter()
        .find(|(_, link)| link.gist_id == gist_id)
    {
        let path = PathBuf::from(path);
        if path.parent().map(|parent| parent.is_dir()).unwrap_or(false) {
            return path;
        }
    }
    docs_dir.join(format!("{}.xmind", sanitize_stem(stem)))
}

pub(crate) async fn fetch_pull(
    app_dir: &Path,
    docs_dir: &Path,
    current_path: &str,
    gist_id: Option<String>,
) -> Result<FetchedPull, String> {
    fetch_pull_with(app_dir, docs_dir, current_path, gist_id, API_ROOT).await
}

async fn fetch_pull_with(
    app_dir: &Path,
    docs_dir: &Path,
    current_path: &str,
    gist_id: Option<String>,
    api_root: &str,
) -> Result<FetchedPull, String> {
    let token = require_token(app_dir)?;
    let sync = read_sync(app_dir)?;
    let gist_id = match gist_id {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => sync
            .documents
            .get(current_path)
            .map(|link| link.gist_id.clone())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "当前导图还没有关联的 Gist".to_string())?,
    };
    if !valid_gist_id(&gist_id) {
        return Err("Gist 编号无效".to_string());
    }
    let client = http_client()?;
    let gist = finish(app_dir, get_gist(&client, api_root, &token, &gist_id).await)?;
    let file_name = preferred_file_name(&gist.files).ok_or("无法从 Gist 解析导图")?;
    let format = format_from_name(&file_name).to_string();
    let file = gist
        .files
        .get(&file_name)
        .cloned()
        .ok_or("无法从 Gist 解析导图")?;
    let content = finish(app_dir, file_text(&client, &token, &file).await)?;
    let document = parse_named_content(&file_name, &content)?;
    let stem = stem_from_gist(&gist.description, &file_name);
    let target = pull_target(docs_dir, &gist.id, &stem, &sync);
    let target_exists = target.is_file();
    Ok(FetchedPull {
        gist_id: gist.id,
        html_url: gist.html_url,
        description: gist.description,
        file_name,
        format,
        document,
        target,
        target_exists,
    })
}

pub(crate) fn save_link(app_dir: &Path, fetched: &FetchedPull) -> Result<(), String> {
    remember_link(
        app_dir,
        &fetched.target.to_string_lossy(),
        DocLink {
            gist_id: fetched.gist_id.clone(),
            html_url: fetched.html_url.clone(),
            format: fetched.format.clone(),
            file_name: fetched.file_name.clone(),
            updated_at: now_iso(),
        },
    )
}

pub(crate) async fn list_gists(app_dir: &Path) -> Result<Vec<GistSummary>, String> {
    list_gists_with(app_dir, API_ROOT).await
}

async fn list_gists_with(app_dir: &Path, api_root: &str) -> Result<Vec<GistSummary>, String> {
    let token = require_token(app_dir)?;
    let client = http_client()?;
    let (status, body) = send(api_builder(
        &client,
        reqwest::Method::GET,
        &format!("{}?per_page=100", join_api(api_root, "/gists")),
        &token,
    ))
    .await
    .map_err(ApiError::into_message)?;
    if let Err(error) = classify_status(status, &body) {
        return fail(app_dir, error);
    }
    let value: Value = serde_json::from_str(&body).map_err(|_| "无法解析 Gist 列表".to_string())?;
    let items = value
        .as_array()
        .ok_or("无法解析 Gist 列表")?
        .iter()
        .filter_map(|item| parse_remote_gist(item).ok())
        .filter(|gist| is_mind_gist(&gist.description, &gist.files));
    let sync = read_sync(app_dir)?;
    let mut summaries = items
        .map(|gist| {
            let linked_path = sync
                .documents
                .iter()
                .find(|(_, link)| link.gist_id == gist.id)
                .map(|(path, _)| path.clone())
                .unwrap_or_default();
            GistSummary {
                file_name: preferred_file_name(&gist.files).unwrap_or_default(),
                id: gist.id,
                description: gist.description,
                html_url: gist.html_url,
                updated_at: gist.updated_at,
                linked_path,
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(summaries)
}

fn fail<T>(app_dir: &Path, error: ApiError) -> Result<T, String> {
    finish(app_dir, Err(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mind-github-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            Self(temp_dir())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_document() -> Value {
        json!({
            "layout": "logicalStructure",
            "root": {
                "data": { "text": "Root", "note": "line1\nline2" },
                "children": [{
                    "data": { "text": "<b>Child</b>" },
                    "children": [{
                        "data": { "text": "Grand" },
                        "children": []
                    }]
                }]
            },
            "theme": { "template": "classic7", "config": {} },
            "view": null
        })
    }

    #[test]
    fn client_id_prefers_env_then_file_then_compile_and_rejects_tokens() {
        assert_eq!(
            resolve_client_id("from-file", Some("Iv1.env"), Some("Iv1.compiled")),
            "Iv1.env"
        );
        assert_eq!(
            resolve_client_id("from-file", Some("  "), Some("Iv1.compiled")),
            "from-file"
        );
        assert_eq!(
            resolve_client_id("", None, Some("Iv1.compiled")),
            "Iv1.compiled"
        );
        assert_eq!(resolve_client_id("gho_secret", None, None), "");
        assert_eq!(resolve_client_id("github_pat_abc", None, None), "");
        assert_eq!(resolve_client_id("has space", None, None), "");
    }

    #[test]
    fn json_roundtrip_keeps_theme_and_markdown_roundtrip_keeps_text() {
        let document = sample_document();
        let json_text = render_json(&document).unwrap();
        let parsed = parse_named_content("Map.mind.json", &json_text).unwrap();
        assert_eq!(parsed["theme"]["template"], "classic7");
        assert_eq!(parsed["root"]["data"]["text"], "Root");

        let markdown = render_markdown(&document).unwrap();
        assert!(markdown.contains("> line1"));
        let parsed = parse_markdown(&markdown).unwrap();
        assert_eq!(parsed["root"]["data"]["text"], "Root");
        assert_eq!(parsed["root"]["data"]["note"], "line1\nline2");
        assert_eq!(parsed["root"]["children"][0]["data"]["text"], "Child");
        assert_eq!(
            parsed["root"]["children"][0]["children"][0]["data"]["text"],
            "Grand"
        );
    }

    #[test]
    fn markdown_exports_seventh_level_as_indented_list() {
        let mut node = json!({"data": {"text": "L7"}, "children": []});
        for level in (1..7).rev() {
            node = json!({
                "data": { "text": format!("L{level}") },
                "children": [node]
            });
        }
        let document = json!({"root": node});
        let markdown = render_markdown(&document).unwrap();
        assert!(markdown.contains("\n   * L7"));
        let parsed = parse_markdown(&markdown).unwrap();
        let mut cursor = parsed["root"].clone();
        for expected in ["L1", "L2", "L3", "L4", "L5", "L6", "L7"] {
            assert_eq!(cursor["data"]["text"], expected);
            if expected != "L7" {
                cursor = cursor["children"][0].clone();
            }
        }
    }

    #[test]
    fn gist_size_limit_is_one_megabyte() {
        assert!(ensure_gist_size(&"a".repeat(GIST_BYTE_LIMIT)).is_ok());
        let error = ensure_gist_size(&"a".repeat(GIST_BYTE_LIMIT + 1)).unwrap_err();
        assert!(error.contains("超过 Gist"));
        assert!(error.contains("1.0 MB"));
    }

    #[test]
    fn create_and_update_bodies_are_secret() {
        let created = create_body("Mind: Map", "Map.mind.json", "{\"root\":{}}");
        assert_eq!(created["public"], false);
        assert!(created.to_string().contains("Map.mind.json"));
        assert!(!created.to_string().contains("client_secret"));

        let updated = update_body(
            "Mind: Map",
            &["Map.mind.md".to_string(), "notes.txt".to_string()],
            "Map.mind.json",
            "{}",
        );
        assert_eq!(updated["public"], false);
        assert!(updated["files"]["Map.mind.md"].is_null());
        assert!(updated["files"].get("notes.txt").is_none());
        assert_eq!(updated["files"]["Map.mind.json"]["content"], "{}");
    }

    #[test]
    fn url_allowlist_accepts_device_and_gist_pages_only() {
        assert!(allowed_github_url("https://github.com/login/device").is_ok());
        assert!(allowed_github_url("https://github.com/login/device?user_code=WDJB-MJHT").is_ok());
        assert!(allowed_github_url("https://gist.github.com/octocat/abc").is_ok());
        assert!(allowed_github_url("https://gist.githubusercontent.com/octocat/abc/raw/1").is_ok());
        assert!(allowed_github_url("http://github.com/login/device").is_err());
        assert!(allowed_github_url("https://github.com/phaethix/mind-map").is_err());
        assert!(allowed_github_url("https://evil.com/login/device").is_err());
        assert!(allowed_github_url("https://gist.github.com.evil.com/abc").is_err());
        assert!(allowed_github_url("https://user:pass@github.com/login/device").is_err());
    }

    #[test]
    fn interval_grows_on_slow_down_and_stays_bounded() {
        assert_eq!(
            next_interval(Duration::from_secs(5), None),
            Duration::from_secs(10)
        );
        assert_eq!(
            next_interval(Duration::from_secs(5), Some(50)),
            Duration::from_secs(50)
        );
        assert_eq!(
            next_interval(Duration::from_secs(120), None),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn epoch_day_is_first_of_january_1970() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(1), (1970, 1, 2));
    }

    #[test]
    fn links_follow_renames_and_forget_deleted_files() {
        let dir = TempDir::new();
        remember_link(
            &dir.0,
            "/docs/a.xmind",
            DocLink {
                gist_id: "abc".to_string(),
                html_url: "https://gist.github.com/octocat/abc".to_string(),
                format: "json".to_string(),
                file_name: "a.mind.json".to_string(),
                updated_at: "2026-10-30T00:00:00Z".to_string(),
            },
        )
        .unwrap();
        remember_link(
            &dir.0,
            "/docs/folder/c.xmind",
            DocLink {
                gist_id: "def".to_string(),
                html_url: String::new(),
                format: "json".to_string(),
                file_name: "c.mind.json".to_string(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        remap_links(
            &dir.0,
            Path::new("/docs/a.xmind"),
            Path::new("/docs/b.xmind"),
        );
        remap_links(
            &dir.0,
            Path::new("/docs/folder"),
            Path::new("/docs/renamed"),
        );
        let sync = read_sync(&dir.0).unwrap();
        assert!(sync.documents.contains_key("/docs/b.xmind"));
        assert!(!sync.documents.contains_key("/docs/a.xmind"));
        assert!(sync.documents.contains_key("/docs/renamed/c.xmind"));
        assert!(!sync.documents.contains_key("/docs/folder/c.xmind"));
        forget_links(&dir.0, Path::new("/docs/b.xmind"));
        let left = read_sync(&dir.0).unwrap();
        assert!(!left.documents.contains_key("/docs/b.xmind"));
        assert!(left.documents.contains_key("/docs/renamed/c.xmind"));
        forget_links(&dir.0, Path::new("/docs/renamed"));
        assert!(read_sync(&dir.0).unwrap().documents.is_empty());
    }

    #[test]
    fn pull_target_reuses_linked_path_and_otherwise_creates_under_docs() {
        let dir = TempDir::new();
        let linked = dir.0.join("linked.xmind");
        fs::write(&linked, b"x").unwrap();
        let mut sync = SyncFile::default();
        sync.documents.insert(
            linked.to_string_lossy().into_owned(),
            DocLink {
                gist_id: "abc".to_string(),
                html_url: String::new(),
                format: "json".to_string(),
                file_name: "Map.mind.json".to_string(),
                updated_at: String::new(),
            },
        );
        assert_eq!(pull_target(&dir.0, "abc", "Other", &sync), linked);
        assert_eq!(
            pull_target(&dir.0, "new", "Other", &sync),
            dir.0.join("Other.xmind")
        );
    }

    #[cfg(unix)]
    #[test]
    fn token_file_is_private_and_sign_out_clears_it() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        write_auth_unlocked(
            &dir.0,
            &AuthFile {
                client_id: "Iv1.example".to_string(),
                access_token: "gho_local_token".to_string(),
                login: "octocat".to_string(),
                scope: SCOPE.to_string(),
            },
        )
        .unwrap();
        let mode = fs::metadata(auth_path(&dir.0))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        sign_out(&dir.0).unwrap();
        let auth = read_auth(&dir.0).unwrap();
        assert!(auth.access_token.is_empty());
        assert!(auth.login.is_empty());
        assert_eq!(auth.client_id, "Iv1.example");
        let raw = fs::read_to_string(auth_path(&dir.0)).unwrap();
        assert!(!raw.contains("gho_local_token"));
    }

    struct Mock {
        base: String,
        shutdown: Arc<AtomicBool>,
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl Mock {
        fn start() -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let shutdown = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let flag = shutdown.clone();
            let recorded = requests.clone();
            std::thread::spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let _ = stream.set_nonblocking(false);
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                            let request = read_http(&mut stream);
                            let (status, body) = route(&request);
                            recorded.lock().unwrap().push(request);
                            let header = format!(
                                "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                reason(status),
                                body.len()
                            );
                            let _ = stream.write_all(header.as_bytes());
                            let _ = stream.write_all(body.as_bytes());
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                base: format!("http://127.0.0.1:{port}"),
                shutdown,
                requests,
            }
        }
    }

    impl Drop for Mock {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::SeqCst);
        }
    }

    fn reason(status: u16) -> &'static str {
        match status {
            200 => "OK",
            201 => "Created",
            401 => "Unauthorized",
            404 => "Not Found",
            _ => "Error",
        }
    }

    fn route(request: &str) -> (u16, String) {
        let first = request.lines().next().unwrap_or("");
        if first.starts_with("POST /login/device/code") {
            return (
                200,
                r#"{"device_code":"device-123","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","verification_uri_complete":"https://github.com/login/device?user_code=WDJB-MJHT","expires_in":900,"interval":5}"#.to_string(),
            );
        }
        if first.starts_with("POST /login/oauth/access_token") {
            return (
                200,
                r#"{"access_token":"gho_test_token","token_type":"bearer","scope":"gist,read:user"}"#.to_string(),
            );
        }
        if first.starts_with("GET /user") {
            return (200, r#"{"login":"octocat"}"#.to_string());
        }
        if first.starts_with("GET /gists/missing") {
            return (404, r#"{"message":"Not Found"}"#.to_string());
        }
        if first.starts_with("GET /gists/abc") {
            return (
                200,
                r#"{"id":"abc","html_url":"https://gist.github.com/octocat/abc","description":"Mind: Map","public":false,"files":{"Map.mind.md":{"filename":"Map.mind.md","content":"Old"}}}"#.to_string(),
            );
        }
        if first.starts_with("PATCH /gists/abc") {
            return (
                200,
                r#"{"id":"abc","html_url":"https://gist.github.com/octocat/abc","description":"Mind: Map","public":false,"files":{"Map.mind.json":{"filename":"Map.mind.json"}}}"#.to_string(),
            );
        }
        if first.starts_with("POST /gists") {
            if request.contains("\"public\":true") {
                return (422, r#"{"message":"refused public"}"#.to_string());
            }
            return (
                201,
                r#"{"id":"created1","html_url":"https://gist.github.com/octocat/created1","description":"Mind: Map","public":false,"files":{"Map.mind.json":{"filename":"Map.mind.json"}}}"#.to_string(),
            );
        }
        if first.starts_with("GET /gists?") || first.starts_with("GET /gists ") {
            return (
                200,
                r#"[{"id":"abc","description":"Mind: Map","html_url":"https://gist.github.com/octocat/abc","updated_at":"2026-10-30T00:00:00Z","files":{"Map.mind.json":{"filename":"Map.mind.json"}}},{"id":"other","description":"notes","updated_at":"2026-10-29T00:00:00Z","files":{"notes.txt":{"filename":"notes.txt"}}}]"#.to_string(),
            );
        }
        if first.starts_with("GET /gists/created1") {
            return (
                200,
                r#"{"id":"created1","html_url":"https://gist.github.com/octocat/created1","description":"Mind: Map","files":{"Map.mind.json":{"filename":"Map.mind.json","content":"{\"layout\":\"logicalStructure\",\"root\":{\"data\":{\"text\":\"Root\"},\"children\":[]},\"theme\":{\"template\":\"classic7\",\"config\":{}},\"view\":null}\n"}}}"#.to_string(),
            );
        }
        (404, r#"{"message":"no"}"#.to_string())
    }

    fn read_http(stream: &mut std::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0_u8; 4096];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if http_complete(&buf) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn http_complete(buf: &[u8]) -> bool {
        let Some(start) = buf.windows(4).position(|item| item == b"\r\n\r\n") else {
            return false;
        };
        let header_end = start + 4;
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        for line in headers.lines() {
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                if let Ok(length) = value.trim().parse::<usize>() {
                    return buf.len() - header_end >= length;
                }
            }
        }
        true
    }

    fn block<T>(future: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(future)
    }

    static AUTH_TEST: Mutex<()> = Mutex::new(());

    #[test]
    fn device_flow_stores_token_without_returning_it() {
        let _guard = AUTH_TEST.lock().unwrap();
        let dir = TempDir::new();
        write_auth_unlocked(
            &dir.0,
            &AuthFile {
                client_id: "Iv1.example".to_string(),
                ..AuthFile::default()
            },
        )
        .unwrap();
        let mock = Mock::start();
        let started = block(auth_start_with(
            &dir.0,
            &format!("{}/login/device/code", mock.base),
            &format!("{}/login/oauth/access_token", mock.base),
            &mock.base,
        ));
        let started = started.expect("device code");
        assert_eq!(started.user_code, "WDJB-MJHT");
        assert!(started.verification_uri.contains("github.com/login/device"));
        {
            let mut gate = lock_gate().unwrap();
            if let Some(pending) = gate.pending.as_mut() {
                pending.last_poll = UNIX_EPOCH;
            }
        }
        let polled = block(auth_poll(&dir.0)).expect("poll");
        assert_eq!(polled.status, "success");
        assert_eq!(polled.login, "octocat");
        let rendered = serde_json::to_string(&polled).unwrap();
        assert!(!rendered.contains("gho_test_token"));
        assert!(!rendered.contains("device-123"));
        let raw = fs::read_to_string(auth_path(&dir.0)).unwrap();
        assert!(raw.contains("gho_test_token"));
        assert!(raw.contains("octocat"));
        let device_request = mock
            .requests
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.contains("POST /login/device/code"))
            .cloned()
            .unwrap();
        assert!(device_request.contains("gist read:user"));
        assert!(!device_request.contains("client_secret"));
        sign_out(&dir.0).unwrap();
        let after = block(auth_poll(&dir.0)).unwrap();
        assert_eq!(after.status, "cancelled");
    }

    #[test]
    fn push_creates_secret_gist_and_pull_parses_it() {
        let dir = TempDir::new();
        let docs = TempDir::new();
        write_auth_unlocked(
            &dir.0,
            &AuthFile {
                client_id: String::new(),
                access_token: "gho_test_token".to_string(),
                login: "octocat".to_string(),
                scope: SCOPE.to_string(),
            },
        )
        .unwrap();
        let mock = Mock::start();
        let document = sample_document();
        let path = docs.0.join("Map.xmind");
        fs::write(&path, b"local").unwrap();
        let path_key = path.to_string_lossy().into_owned();
        let pushed = block(push_with(&dir.0, &path_key, &document, "json", &mock.base)).unwrap();
        assert!(pushed.created);
        assert!(!pushed.recreated);
        assert_eq!(pushed.gist_id, "created1");
        let create_request = mock
            .requests
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.contains("POST /gists"))
            .cloned()
            .unwrap();
        assert!(create_request.contains("\"public\":false"));
        assert!(
            create_request.contains("Authorization: Bearer gho_test_token")
                || create_request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer gho_test_token")
        );
        assert!(!create_request
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or("")
            .contains("gho_test_token"));
        assert!(!create_request.contains("client_secret"));

        let listed = block(list_gists_with(&dir.0, &mock.base)).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "abc");

        let fetched = block(fetch_pull_with(
            &dir.0,
            &docs.0,
            "",
            Some("created1".to_string()),
            &mock.base,
        ))
        .unwrap();
        assert_eq!(fetched.document["root"]["data"]["text"], "Root");
        assert_eq!(fetched.target, path);
        assert!(fetched.target_exists);

        fs::remove_file(&path).unwrap();
        forget_links(&dir.0, path.as_path());
        let fresh = block(fetch_pull_with(
            &dir.0,
            &docs.0,
            "",
            Some("created1".to_string()),
            &mock.base,
        ))
        .unwrap();
        assert_eq!(fresh.target, docs.0.join("Map.xmind"));
        assert!(!fresh.target_exists);

        fs::write(&fresh.target, b"old").unwrap();
        let again = block(fetch_pull_with(
            &dir.0,
            &docs.0,
            "",
            Some("created1".to_string()),
            &mock.base,
        ))
        .unwrap();
        assert!(again.target_exists);
    }

    #[test]
    fn push_updates_existing_gist_and_recreates_when_missing() {
        let dir = TempDir::new();
        write_auth_unlocked(
            &dir.0,
            &AuthFile {
                access_token: "gho_test_token".to_string(),
                ..AuthFile::default()
            },
        )
        .unwrap();
        remember_link(
            &dir.0,
            "/docs/Map.xmind",
            DocLink {
                gist_id: "abc".to_string(),
                html_url: String::new(),
                format: "markdown".to_string(),
                file_name: "Map.mind.md".to_string(),
                updated_at: String::new(),
            },
        )
        .unwrap();
        let mock = Mock::start();
        let pushed = block(push_with(
            &dir.0,
            "/docs/Map.xmind",
            &sample_document(),
            "json",
            &mock.base,
        ))
        .unwrap();
        assert!(!pushed.created);
        assert_eq!(pushed.gist_id, "abc");
        let patch = mock
            .requests
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.contains("PATCH /gists/abc"))
            .cloned()
            .unwrap();
        assert!(patch.contains("\"public\":false"));
        assert!(patch.contains("\"Map.mind.md\":null"));

        let mut sync = read_sync(&dir.0).unwrap();
        sync.documents.get_mut("/docs/Map.xmind").unwrap().gist_id = "missing".to_string();
        write_sync(&dir.0, &sync).unwrap();
        let recreated = block(push_with(
            &dir.0,
            "/docs/Map.xmind",
            &sample_document(),
            "json",
            &mock.base,
        ))
        .unwrap();
        assert!(recreated.recreated);
        assert_eq!(recreated.gist_id, "created1");
    }

    #[test]
    fn unauthorized_push_clears_the_saved_token() {
        let dir = TempDir::new();
        write_auth_unlocked(
            &dir.0,
            &AuthFile {
                access_token: "gho_test_token".to_string(),
                login: "octocat".to_string(),
                ..AuthFile::default()
            },
        )
        .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = read_http(&mut stream);
                let body = r#"{"message":"Bad credentials"}"#;
                let header = format!(
                    "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
            }
        });
        let error = block(push_with(
            &dir.0,
            "/docs/Map.xmind",
            &sample_document(),
            "json",
            &format!("http://127.0.0.1:{port}"),
        ))
        .unwrap_err();
        assert_eq!(error, "GitHub 登录已失效，请重新登录");
        assert!(!error.contains("gho_test_token"));
        assert!(read_auth(&dir.0).unwrap().access_token.is_empty());
    }
}
