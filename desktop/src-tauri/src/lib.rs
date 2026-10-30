use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::UpdaterExt;

mod github;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionFile {
    current_path: String,
    lang: String,
    mind_map_config: Value,
    local_config: Value,
    #[serde(default)]
    recent: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileCard {
    path: String,
    name: String,
    modified_ms: u64,
    thumbnail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSession {
    path: String,
    file_name: String,
    mind_map_data: Value,
    mind_map_config: Value,
    lang: String,
    local_config: Value,
}

struct AppState {
    docs_dir: PathBuf,
    session_path: PathBuf,
    session: Mutex<SessionFile>,
    pending_open: Mutex<Option<PathBuf>>,
}

fn empty_document() -> Value {
    json!({
        "layout": "logicalStructure",
        "root": {
            "data": { "text": "根节点" },
            "children": []
        },
        "theme": {
            "template": "classic7",
            "config": {}
        },
        "view": null
    })
}

fn normalize_document(value: Value) -> Value {
    if value.get("root").is_some() {
        return value;
    }
    if value.get("data").is_some() {
        let mut document = empty_document();
        document["root"] = value;
        return document;
    }
    value
}

const FRONTEND_PORT: u16 = 17321;

fn start_frontend_server(app: AppHandle) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", FRONTEND_PORT)).map_err(|error| {
        format!("本机端口 {FRONTEND_PORT} 已被占用，请先关掉另一个 Mind 窗口：{error}")
    })?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let app = app.clone();
            std::thread::spawn(move || {
                let _ = serve_frontend(&app, &mut stream);
            });
        }
    });
    Ok(())
}

fn serve_frontend(app: &AppHandle, stream: &mut TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let request_path = request
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("/");
    let request_path = request_path.split('?').next().unwrap_or("/");
    let asset_path = if request_path == "/" {
        "index.html".to_string()
    } else {
        request_path.trim_start_matches('/').to_string()
    };
    if let Some(asset) = app.asset_resolver().get(asset_path) {
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            asset.mime_type,
            asset.bytes.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(&asset.bytes)?;
    } else {
        let body = b"not found";
        let header = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(body)?;
    }
    Ok(())
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "未命名.xmind".to_string())
}

fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp_name = format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
    );
    let tmp_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(tmp_name);
    fs::write(&tmp_path, text).map_err(|error| error.to_string())?;
    fs::rename(&tmp_path, path).map_err(|error| error.to_string())?;
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    write_text_atomic(path, &text)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| format!("无法解析 {}: {error}", path.display()))
}

fn ensure_xmind(path: PathBuf) -> PathBuf {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("xmind") | Some("smm") | Some("json") => path,
        _ => path.with_extension("xmind"),
    }
}

fn next_untitled(dir: &Path) -> PathBuf {
    let first = dir.join("未命名.xmind");
    if !first.exists() {
        return first;
    }
    for index in 2..1000 {
        let candidate = dir.join(format!("未命名 {index}.xmind"));
        if !candidate.exists() {
            return candidate;
        }
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    dir.join(format!("未命名 {stamp}.xmind"))
}

fn set_window_title(app: &AppHandle, file_name: &str) {
    if let Some(window) = app.get_webview_window("main") {
        // macOS 的菜单栏已经写着应用名，窗口标题只留文件名
        #[cfg(target_os = "macos")]
        let title = file_name.to_string();
        #[cfg(not(target_os = "macos"))]
        let title = format!("{file_name} — Mind");
        let _ = window.set_title(&title);
    }
}

fn persist_session(state: &AppState, session: &SessionFile) -> Result<(), String> {
    let value = serde_json::to_value(session).map_err(|error| error.to_string())?;
    write_json(&state.session_path, &value)
}

fn load_session_file(path: &Path) -> Option<SessionFile> {
    let value = read_json(path).ok()?;
    serde_json::from_value(value).ok()
}

fn create_document(path: &Path) -> Result<Value, String> {
    let document = empty_document();
    write_document(path, &document)?;
    Ok(document)
}

fn read_document(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let value = if bytes.len() >= 4 && &bytes[..4] == b"PK\x03\x04" {
        read_xmind_bytes(&bytes)?
    } else {
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?
    };
    Ok(normalize_document(value))
}

fn write_document(path: &Path, data: &Value) -> Result<(), String> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("xmind") {
        write_xmind(path, data)
    } else {
        write_json(path, data)
    }
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
            text.push(ch);
        }
    }
    text
}

fn topic_from_node(node: &Value, relationships: &mut Vec<Value>) -> Value {
    let data = node.get("data").cloned().unwrap_or_else(|| json!({}));
    let uid = data
        .get("uid")
        .and_then(|value| value.as_str())
        .unwrap_or("node")
        .to_string();
    if let Some(targets) = data
        .get("associativeLineTargets")
        .and_then(|value| value.as_array())
    {
        for target in targets {
            let Some(target_id) = target.as_str() else {
                continue;
            };
            let mut relationship = json!({
                "id": format!("rel-{uid}-{target_id}"),
                "end1Id": uid,
                "end2Id": target_id
            });
            if let Some(title) = data
                .get("associativeLineText")
                .and_then(|value| value.get(target_id))
                .and_then(|value| value.as_str())
            {
                relationship["title"] = json!(title);
            }
            relationships.push(relationship);
        }
    }
    let mut topic = json!({
        "id": uid,
        "class": "topic",
        "title": plain_text(data.get("text").unwrap_or(&Value::Null)),
        "children": { "attached": [] }
    });
    if let Some(children) = node.get("children").and_then(|value| value.as_array()) {
        for child in children {
            topic["children"]["attached"]
                .as_array_mut()
                .expect("attached children")
                .push(topic_from_node(child, relationships));
        }
    }
    topic
}

fn xmind_content(data: &Value) -> Value {
    let root = data.get("root").unwrap_or(data);
    let mut relationships = Vec::new();
    let root_topic = topic_from_node(root, &mut relationships);
    let sheet_id = format!(
        "sheet-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    );
    json!([{
        "id": sheet_id,
        "class": "sheet",
        "title": "Mind",
        "rootTopic": root_topic,
        "relationships": relationships,
        "topicPositioning": "fixed",
        "topicOverlapping": "overlap",
        "coreVersion": "2.100.0"
    }])
}

fn write_xmind(path: &Path, data: &Value) -> Result<(), String> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let files = [
        (
            "mind.json",
            serde_json::to_vec_pretty(data).map_err(|error| error.to_string())?,
        ),
        (
            "content.json",
            serde_json::to_vec(&xmind_content(data)).map_err(|error| error.to_string())?,
        ),
        (
            "metadata.json",
            br#"{"dataStructureVersion":"2","creator":{"name":"Mind"},"layoutEngineVersion":"3"}"#
                .to_vec(),
        ),
        (
            "manifest.json",
            br#"{"file-entries":{"content.json":{},"metadata.json":{},"mind.json":{}}}"#.to_vec(),
        ),
    ];
    for (name, bytes) in files {
        zip.start_file(name, options)
            .map_err(|error| error.to_string())?;
        zip.write_all(&bytes).map_err(|error| error.to_string())?;
    }
    zip.finish().map_err(|error| error.to_string())?;
    let bytes = cursor.into_inner();
    let tmp = path.with_extension("xmind.tmp");
    fs::write(&tmp, &bytes).map_err(|error| error.to_string())?;
    fs::rename(&tmp, path).map_err(|error| error.to_string())
}

fn read_zip_file(
    archive: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    name: &str,
) -> Option<Vec<u8>> {
    let mut file = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn topic_to_node(topic: &Value) -> Value {
    let mut node = json!({
        "data": {
            "text": topic.get("title").and_then(|value| value.as_str()).unwrap_or(""),
            "uid": topic.get("id").and_then(|value| value.as_str()).unwrap_or("")
        },
        "children": []
    });
    if let Some(attached) = topic
        .get("children")
        .and_then(|value| value.get("attached"))
        .and_then(|value| value.as_array())
    {
        for child in attached {
            node["children"]
                .as_array_mut()
                .expect("children")
                .push(topic_to_node(child));
        }
    }
    node
}

fn add_associative_line(node: &mut Value, from: &str, to: &str, title: Option<&str>) -> bool {
    let uid = node
        .get("data")
        .and_then(|data| data.get("uid"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    if uid == from {
        let targets = node["data"]
            .as_object_mut()
            .expect("node data")
            .entry("associativeLineTargets")
            .or_insert_with(|| json!([]));
        if let Some(list) = targets.as_array_mut() {
            list.push(json!(to));
        }
        if let Some(title) = title {
            node["data"]
                .as_object_mut()
                .expect("node data")
                .entry("associativeLineText")
                .or_insert_with(|| json!({}))[to] = json!(title);
        }
        return true;
    }
    if let Some(children) = node
        .get_mut("children")
        .and_then(|value| value.as_array_mut())
    {
        for child in children {
            if add_associative_line(child, from, to, title) {
                return true;
            }
        }
    }
    false
}

fn xmind_content_to_document(content: &Value) -> Value {
    let sheet = content
        .as_array()
        .and_then(|sheets| sheets.first())
        .unwrap_or(content);
    let Some(root_topic) = sheet.get("rootTopic") else {
        return empty_document();
    };
    let mut root = topic_to_node(root_topic);
    if let Some(relationships) = sheet
        .get("relationships")
        .and_then(|value| value.as_array())
    {
        for relationship in relationships {
            let Some(from) = relationship.get("end1Id").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(to) = relationship.get("end2Id").and_then(|value| value.as_str()) else {
                continue;
            };
            let title = relationship.get("title").and_then(|value| value.as_str());
            add_associative_line(&mut root, from, to, title);
        }
    }
    json!({
        "layout": "logicalStructure",
        "root": root,
        "theme": { "template": "classic7", "config": {} },
        "view": null
    })
}

fn read_xmind_bytes(bytes: &[u8]) -> Result<Value, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))
        .map_err(|error| error.to_string())?;
    if let Some(mind) = read_zip_file(&mut archive, "mind.json") {
        return serde_json::from_slice(&mind).map_err(|error| error.to_string());
    }
    let content = read_zip_file(&mut archive, "content.json")
        .ok_or_else(|| "xmind 文件里没有内容".to_string())?;
    let content: Value = serde_json::from_slice(&content).map_err(|error| error.to_string())?;
    Ok(xmind_content_to_document(&content))
}

fn session_view(session: &SessionFile, document: Value) -> DesktopSession {
    let path = PathBuf::from(&session.current_path);
    DesktopSession {
        file_name: file_name_of(&path),
        path: session.current_path.clone(),
        mind_map_data: document,
        mind_map_config: session.mind_map_config.clone(),
        lang: session.lang.clone(),
        local_config: session.local_config.clone(),
    }
}

fn switch_to(state: &AppState, app: &AppHandle, path: PathBuf) -> Result<DesktopSession, String> {
    let document = if path.exists() {
        read_document(&path)?
    } else {
        create_document(&path)?
    };
    let mut session = state.session.lock().map_err(|error| error.to_string())?;
    let current = path.to_string_lossy().into_owned();
    session.current_path = current.clone();
    remember_recent(&mut session, &current);
    persist_session(state, &session)?;
    let view = session_view(&session, document);
    set_window_title(app, &view.file_name);
    Ok(view)
}

fn remember_recent(session: &mut SessionFile, path: &str) {
    session.recent.retain(|item| item != path);
    session.recent.insert(0, path.to_string());
    session.recent.truncate(50);
}

fn display_name(path: &Path) -> String {
    let name = file_name_of(path);
    name.strip_suffix(".xmind").unwrap_or(&name).to_string()
}

fn modified_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn thumbnails_dir(state: &AppState) -> PathBuf {
    state
        .session_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("thumbnails")
}

fn thumbnail_file(state: &AppState, path: &str) -> PathBuf {
    let name: String = path.bytes().map(|byte| format!("{byte:02x}")).collect();
    thumbnails_dir(state).join(format!("{name}.jpg"))
}

fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= data.len() {
        let n =
            ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8) | data[index + 2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        index += 3;
    }
    if index < data.len() {
        let mut n = (data[index] as u32) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        if index + 1 < data.len() {
            n |= (data[index + 1] as u32) << 8;
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        }
    }
    out
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    fn value(byte: u8) -> Result<u8, String> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("缩略图无法解析".to_string()),
        }
    }
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if bytes.len() % 4 != 0 {
        return Err("缩略图无法解析".to_string());
    }
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().filter(|byte| **byte == b'=').count();
        let a = value(chunk[0])?;
        let b = value(chunk[1])?;
        let c = if chunk[2] == b'=' {
            0
        } else {
            value(chunk[2])?
        };
        let d = if chunk[3] == b'=' {
            0
        } else {
            value(chunk[3])?
        };
        let n = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | d as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

fn thumbnail_data_url(state: &AppState, path: &str) -> String {
    let file = thumbnail_file(state, path);
    let Ok(bytes) = fs::read(&file) else {
        return String::new();
    };
    format!("data:image/jpeg;base64,{}", encode_base64(&bytes))
}

fn file_card(state: &AppState, path: &Path) -> FileCard {
    let text = path.to_string_lossy().into_owned();
    FileCard {
        name: display_name(path),
        modified_ms: modified_ms(path),
        thumbnail: thumbnail_data_url(state, &text),
        path: text,
    }
}

fn is_xmind(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("xmind")
}

// —— 平台相关的系统调用 ——
// 这几个动作在三个系统上没有共同的实现方式，各写一份比引第三方依赖更可控，
// 也能保证任意平台上都不会因为缺某个命令而编不过。

// 用系统默认程序打开文件或用文件管理器打开目录
#[cfg(target_os = "macos")]
fn open_external(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .status()
        .map(|_| ())
        .map_err(|error| format!("无法打开：{error}"))
}

#[cfg(target_os = "windows")]
fn open_external(path: &Path) -> Result<(), String> {
    // start 走 ShellExecute：目录交给资源管理器，文件按关联程序打开
    let mut command = Command::new("cmd");
    command.args(["/C", "start", ""]).arg(path);
    no_console_window(&mut command);
    command
        .status()
        .map(|_| ())
        .map_err(|error| format!("无法打开：{error}"))
}

// 桌面应用里调用 shell 命令不能弹控制台窗口，否则每次都会闪一下黑框
#[cfg(target_os = "windows")]
fn no_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_external(path: &Path) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(path)
        .status()
        .map(|_| ())
        .map_err(|error| format!("无法打开：{error}"))
}

#[cfg(target_os = "macos")]
fn move_to_trash(path: &Path) -> Result<(), String> {
    let text = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let status = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "tell application \"Finder\" to delete POSIX file \"{text}\""
        ))
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("无法移到废纸篓".to_string())
    }
}

#[cfg(target_os = "windows")]
fn move_to_trash(path: &Path) -> Result<(), String> {
    // 回收站没有命令行原语，借 Microsoft.VisualBasic 的 FileSystem 走 Shell 的删除
    // 目录要用 DeleteDirectory，DeleteFile 对文件夹会直接失败
    let method = if path.is_dir() {
        "DeleteDirectory"
    } else {
        "DeleteFile"
    };
    let mut command = Command::new("powershell");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Add-Type -AssemblyName Microsoft.VisualBasic; \
                 [Microsoft.VisualBasic.FileIO.FileSystem]::{method}($args[0], 'OnlyErrorDialogs', 'SendToRecycleBin')"
            ),
        ])
        .arg(path);
    no_console_window(&mut command);
    let status = command
        .status()
        .map_err(|error| format!("无法移到回收站：{error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("无法移到回收站".to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn move_to_trash(path: &Path) -> Result<(), String> {
    // gio 随 glib 一起安装，而 glib 是 GTK/WebKitGTK 的硬依赖，桌面环境下必然存在
    let status = Command::new("gio")
        .arg("trash")
        .arg(path)
        .status()
        .map_err(|error| format!("无法移到回收站（缺少 gio/glib2）：{error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("无法移到回收站".to_string())
    }
}

fn pick_file(app: &AppHandle, directory: PathBuf, save: bool) -> Result<Option<PathBuf>, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let mut dialog = rfd::FileDialog::new()
            .set_directory(&directory)
            .add_filter("Mind", &["xmind", "smm", "json"]);
        let picked = if save {
            dialog = dialog.set_file_name("未命名.xmind");
            dialog.save_file().map(ensure_xmind)
        } else {
            dialog.pick_file()
        };
        let _ = sender.send(picked);
    })
    .map_err(|error| error.to_string())?;
    receiver.recv().map_err(|error| error.to_string())
}

fn initial_state(app: &AppHandle) -> Result<AppState, String> {
    let documents = app
        .path()
        .document_dir()
        .map_err(|error| error.to_string())?;
    let docs_dir = documents.join("Mind");
    let legacy_dir = documents.join("思维导图");
    if !docs_dir.exists() && legacy_dir.exists() {
        fs::rename(&legacy_dir, &docs_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&docs_dir).map_err(|error| error.to_string())?;
    // 统一成规范路径再存进 state：Windows 上 canonicalize 会加 \\?\ 前缀，
    // 如果不统一，后面 docs_root() 和 docs_dir 比较会全部失配
    let docs_dir = fs::canonicalize(&docs_dir).unwrap_or(docs_dir);
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&app_dir).map_err(|error| error.to_string())?;
    let session_path = app_dir.join("session.json");
    let mut session = load_session_file(&session_path).unwrap_or_else(|| SessionFile {
        current_path: docs_dir.join("未命名.xmind").to_string_lossy().into_owned(),
        lang: "zh".to_string(),
        mind_map_config: json!({}),
        local_config: Value::Null,
        recent: Vec::new(),
    });
    if session.current_path.contains("/思维导图/") {
        let migrated = session.current_path.replace("/思维导图/", "/Mind/");
        if Path::new(&migrated).exists() {
            session.current_path = migrated;
        }
    }
    if session.current_path.ends_with(".smm") {
        let old_path = PathBuf::from(&session.current_path);
        let new_path = old_path.with_extension("xmind");
        if old_path.exists() && !new_path.exists() {
            if let Ok(data) = read_document(&old_path) {
                if write_document(&new_path, &data).is_ok() {
                    session.current_path = new_path.to_string_lossy().into_owned();
                }
            }
        } else if new_path.exists() {
            session.current_path = new_path.to_string_lossy().into_owned();
        }
        let value = serde_json::to_value(&session).map_err(|error| error.to_string())?;
        write_json(&session_path, &value)?;
    }
    Ok(AppState {
        docs_dir,
        session_path,
        session: Mutex::new(session),
        pending_open: Mutex::new(None),
    })
}

#[tauri::command]
fn bootstrap(state: State<AppState>) -> Result<DesktopSession, String> {
    let session = state.session.lock().map_err(|error| error.to_string())?;
    Ok(DesktopSession {
        path: String::new(),
        file_name: String::new(),
        mind_map_data: Value::Null,
        mind_map_config: session.mind_map_config.clone(),
        lang: session.lang.clone(),
        local_config: session.local_config.clone(),
    })
}

#[tauri::command]
fn save_mind_map(state: State<AppState>, data: Value) -> Result<(), String> {
    let session = state.session.lock().map_err(|error| error.to_string())?;
    write_document(Path::new(&session.current_path), &data)
}

#[tauri::command]
fn save_config(state: State<AppState>, config: Value) -> Result<(), String> {
    let mut session = state.session.lock().map_err(|error| error.to_string())?;
    session.mind_map_config = config;
    persist_session(&state, &session)
}

#[tauri::command]
fn save_lang(state: State<AppState>, lang: String) -> Result<(), String> {
    let mut session = state.session.lock().map_err(|error| error.to_string())?;
    session.lang = lang;
    persist_session(&state, &session)
}

#[tauri::command]
fn save_local_config(state: State<AppState>, config: Value) -> Result<(), String> {
    let mut session = state.session.lock().map_err(|error| error.to_string())?;
    session.local_config = config;
    persist_session(&state, &session)
}

#[tauri::command]
fn new_file(app: AppHandle, state: State<AppState>) -> Result<DesktopSession, String> {
    let path = next_untitled(&state.docs_dir);
    switch_to(&state, &app, path)
}

#[tauri::command]
fn open_file(app: AppHandle, state: State<AppState>) -> Result<Option<DesktopSession>, String> {
    let Some(path) = pick_file(&app, state.docs_dir.clone(), false)? else {
        return Ok(None);
    };
    switch_to(&state, &app, path).map(Some)
}

#[tauri::command]
fn save_file_as(
    app: AppHandle,
    state: State<AppState>,
    data: Value,
) -> Result<Option<DesktopSession>, String> {
    let Some(path) = pick_file(&app, state.docs_dir.clone(), true)? else {
        return Ok(None);
    };
    write_document(&path, &data)?;
    let mut session = state.session.lock().map_err(|error| error.to_string())?;
    let current = path.to_string_lossy().into_owned();
    session.current_path = current.clone();
    remember_recent(&mut session, &current);
    persist_session(&state, &session)?;
    let view = session_view(&session, data);
    set_window_title(&app, &view.file_name);
    Ok(Some(view))
}

#[tauri::command]
fn list_files(state: State<AppState>) -> Result<Vec<FileCard>, String> {
    let mut seen = HashSet::new();
    let mut cards = Vec::new();
    let mut push = |path: PathBuf| {
        if !is_xmind(&path) || !path.is_file() {
            return;
        }
        let text = path.to_string_lossy().into_owned();
        if !seen.insert(text) {
            return;
        }
        cards.push(file_card(&state, &path));
    };
    if let Ok(entries) = fs::read_dir(&state.docs_dir) {
        for entry in entries.flatten() {
            push(entry.path());
        }
    }
    let recent = state
        .session
        .lock()
        .map_err(|error| error.to_string())?
        .recent
        .clone();
    for path in recent {
        push(PathBuf::from(path));
    }
    cards.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    Ok(cards)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FolderCard {
    path: String,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Crumb {
    // 根目录留空，由前端显示成「全部文件」
    name: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryListing {
    path: String,
    breadcrumb: Vec<Crumb>,
    folders: Vec<FolderCard>,
    files: Vec<FileCard>,
}

// 前端传来的路径一律限制在文档目录内，防止误操作到别处
fn docs_root(state: &AppState) -> Result<PathBuf, String> {
    fs::canonicalize(&state.docs_dir).map_err(|error| error.to_string())
}

fn canonical_inside(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path).map_err(|_| "路径不存在".to_string())?;
    if canonical.starts_with(root) {
        Ok(canonical)
    } else {
        Err("路径不在文档目录内".to_string())
    }
}

fn resolve_dir(state: &AppState, raw: Option<String>) -> Result<PathBuf, String> {
    let root = docs_root(state)?;
    let raw = raw.unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(root);
    }
    let candidate = PathBuf::from(raw);
    let target = if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    };
    let dir = canonical_inside(&root, &target)?;
    if !dir.is_dir() {
        return Err("目录不存在".to_string());
    }
    Ok(dir)
}

fn valid_folder_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|value| matches!(value, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
}

fn folder_card(path: &Path) -> FolderCard {
    FolderCard {
        name: file_name_of(path),
        path: path.to_string_lossy().into_owned(),
    }
}

fn breadcrumbs(root: &Path, dir: &Path) -> Vec<Crumb> {
    let mut chain = Vec::new();
    let mut current = dir;
    while current != root {
        chain.push(current.to_path_buf());
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    chain.push(root.to_path_buf());
    chain.reverse();
    chain
        .into_iter()
        .map(|path| Crumb {
            name: if path == root {
                String::new()
            } else {
                file_name_of(&path)
            },
            path: path.to_string_lossy().into_owned(),
        })
        .collect()
}

#[tauri::command]
fn library_list(state: State<AppState>, dir: Option<String>) -> Result<LibraryListing, String> {
    let root = docs_root(&state)?;
    let current = resolve_dir(&state, dir)?;
    let mut folders = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&current) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                folders.push(folder_card(&path));
            } else if file_type.is_file() && is_xmind(&path) {
                files.push(file_card(&state, &path));
            }
        }
    }
    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    Ok(LibraryListing {
        path: current.to_string_lossy().into_owned(),
        breadcrumb: breadcrumbs(&root, &current),
        folders,
        files,
    })
}

#[tauri::command]
fn library_create_folder(
    state: State<AppState>,
    dir: Option<String>,
    name: String,
) -> Result<FolderCard, String> {
    let parent = resolve_dir(&state, dir)?;
    let name = name.trim();
    if !valid_folder_name(name) {
        return Err("文件夹名称无效".to_string());
    }
    let target = parent.join(name);
    if target.exists() {
        return Err("已存在同名文件夹".to_string());
    }
    fs::create_dir(&target).map_err(|error| error.to_string())?;
    Ok(folder_card(&target))
}

#[tauri::command]
fn library_create_file(
    app: AppHandle,
    state: State<AppState>,
    dir: Option<String>,
) -> Result<DesktopSession, String> {
    let target = resolve_dir(&state, dir)?;
    let path = next_untitled(&target);
    switch_to(&state, &app, path)
}

#[tauri::command]
fn library_move(
    state: State<AppState>,
    path: String,
    dir: Option<String>,
) -> Result<FileCard, String> {
    let root = docs_root(&state)?;
    let source = canonical_inside(&root, &PathBuf::from(&path))?;
    if !source.is_file() || !is_xmind(&source) {
        return Err("只能移动 .xmind 文件".to_string());
    }
    let target_dir = resolve_dir(&state, dir)?;
    let dest = target_dir.join(file_name_of(&source));
    if dest == source {
        return Ok(file_card(&state, &source));
    }
    if dest.exists() {
        return Err("目标位置已有同名文件".to_string());
    }
    fs::rename(&source, &dest).map_err(|error| error.to_string())?;
    // 移动后同步缩略图与会话路径，否则「打开的文件」会指向不存在的位置
    remap_thumbnails(&state, &source, &dest);
    remap_session_paths(&state, &source, &dest);
    Ok(file_card(&state, &dest))
}

#[tauri::command]
fn library_recent(state: State<AppState>) -> Result<Vec<FileCard>, String> {
    let recent = state
        .session
        .lock()
        .map_err(|error| error.to_string())?
        .recent
        .clone();
    let mut seen = HashSet::new();
    let mut cards = Vec::new();
    for item in recent {
        if !seen.insert(item.clone()) {
            continue;
        }
        let path = PathBuf::from(&item);
        if path.is_file() && is_xmind(&path) {
            cards.push(file_card(&state, &path));
        }
        if cards.len() >= 12 {
            break;
        }
    }
    Ok(cards)
}

fn collect_matches(dir: &Path, query: &str, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 || out.len() >= 200 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            if file_name_of(&path).starts_with('.') {
                continue;
            }
            collect_matches(&path, query, out, depth + 1);
        } else if file_type.is_file()
            && is_xmind(&path)
            && file_name_of(&path).to_lowercase().contains(query)
        {
            out.push(path);
        }
    }
}

#[tauri::command]
fn library_search(state: State<AppState>, query: String) -> Result<Vec<FileCard>, String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let root = docs_root(&state)?;
    let mut matches = Vec::new();
    collect_matches(&root, &query, &mut matches, 0);
    matches.sort_by(|a, b| modified_ms(b).cmp(&modified_ms(a)));
    matches.truncate(100);
    Ok(matches
        .into_iter()
        .map(|path| file_card(&state, &path))
        .collect())
}

// 缩略图按文件全路径的十六进制命名，文件夹改名或删除时要同步处理
fn thumbnail_path_from_name(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".jpg")?;
    if stem.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(stem.len() / 2);
    let mut index = 0;
    while index < stem.len() {
        bytes.push(u8::from_str_radix(&stem[index..index + 2], 16).ok()?);
        index += 2;
    }
    String::from_utf8(bytes).ok()
}

fn remap_thumbnails(state: &AppState, from: &Path, to: &Path) {
    let Ok(entries) = fs::read_dir(thumbnails_dir(state)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(source) = thumbnail_path_from_name(name) else {
            continue;
        };
        let Ok(rest) = Path::new(&source).strip_prefix(from) else {
            continue;
        };
        let target = to.join(rest).to_string_lossy().into_owned();
        let _ = fs::rename(&path, thumbnail_file(state, &target));
    }
}

fn forget_thumbnails(state: &AppState, dir: &Path) {
    let Ok(entries) = fs::read_dir(thumbnails_dir(state)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(source) = thumbnail_path_from_name(name) else {
            continue;
        };
        if Path::new(&source).starts_with(dir) {
            let _ = fs::remove_file(&path);
        }
    }
}

fn remap_session_paths(state: &AppState, from: &Path, to: &Path) {
    if let Ok(mut session) = state.session.lock() {
        let remap = |value: &mut String| {
            if let Ok(rest) = PathBuf::from(&*value).strip_prefix(from) {
                *value = to.join(rest).to_string_lossy().into_owned();
            }
        };
        remap(&mut session.current_path);
        for item in session.recent.iter_mut() {
            remap(item);
        }
        let _ = persist_session(state, &session);
    }
    if let Some(dir) = github_store_dir(state) {
        github::remap_links(&dir, from, to);
    }
}

fn forget_session_paths(state: &AppState, dir: &Path) {
    if let Ok(mut session) = state.session.lock() {
        if PathBuf::from(&session.current_path).starts_with(dir) {
            session.current_path.clear();
        }
        session
            .recent
            .retain(|item| !PathBuf::from(item).starts_with(dir));
        let _ = persist_session(state, &session);
    }
    if let Some(app_dir) = github_store_dir(state) {
        github::forget_links(&app_dir, dir);
    }
}

#[tauri::command]
fn library_rename_folder(
    state: State<AppState>,
    path: String,
    name: String,
) -> Result<FolderCard, String> {
    let root = docs_root(&state)?;
    let source = canonical_inside(&root, &PathBuf::from(&path))?;
    if !source.is_dir() || source == root {
        return Err("只能重命名文档目录下的文件夹".to_string());
    }
    let name = name.trim();
    if !valid_folder_name(name) {
        return Err("文件夹名称无效".to_string());
    }
    let parent = source.parent().ok_or_else(|| "路径无效".to_string())?;
    let dest = parent.join(name);
    if dest == source {
        return Ok(folder_card(&source));
    }
    if dest.exists() {
        return Err("已存在同名文件夹".to_string());
    }
    fs::rename(&source, &dest).map_err(|error| error.to_string())?;
    remap_thumbnails(&state, &source, &dest);
    remap_session_paths(&state, &source, &dest);
    Ok(folder_card(&dest))
}

#[tauri::command]
fn library_delete_folder(state: State<AppState>, path: String) -> Result<(), String> {
    let root = docs_root(&state)?;
    let target = canonical_inside(&root, &PathBuf::from(&path))?;
    if !target.is_dir() || target == root {
        return Err("只能删除文档目录下的文件夹".to_string());
    }
    move_to_trash(&target)?;
    forget_thumbnails(&state, &target);
    forget_session_paths(&state, &target);
    Ok(())
}

#[tauri::command]
fn open_path(
    app: AppHandle,
    state: State<AppState>,
    path: String,
) -> Result<DesktopSession, String> {
    switch_to(&state, &app, PathBuf::from(path))
}

#[tauri::command]
fn rename_file(
    app: AppHandle,
    state: State<AppState>,
    path: String,
    name: String,
) -> Result<FileCard, String> {
    let source = PathBuf::from(&path);
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\\') {
        return Err("名称无效".to_string());
    }
    let file_name = if trimmed.ends_with(".xmind") {
        trimmed.to_string()
    } else {
        format!("{trimmed}.xmind")
    };
    let parent = source.parent().ok_or_else(|| "名称无效".to_string())?;
    let dest = parent.join(&file_name);
    if dest != source && dest.exists() {
        return Err("已有同名文件".to_string());
    }
    if dest != source {
        fs::rename(&source, &dest).map_err(|error| error.to_string())?;
        let old_thumb = thumbnail_file(&state, &path);
        if old_thumb.exists() {
            let _ = fs::rename(old_thumb, thumbnail_file(&state, &dest.to_string_lossy()));
        }
    }
    let dest_text = dest.to_string_lossy().into_owned();
    let mut session = state.session.lock().map_err(|error| error.to_string())?;
    if session.current_path == path {
        session.current_path = dest_text.clone();
    }
    for item in session.recent.iter_mut() {
        if item == &path {
            *item = dest_text.clone();
        }
    }
    persist_session(&state, &session)?;
    if path != dest_text {
        if let Some(dir) = github_store_dir(&state) {
            github::remap_links(&dir, Path::new(&path), Path::new(&dest_text));
        }
    }
    set_window_title(&app, &file_name);
    Ok(file_card(&state, &dest))
}

#[tauri::command]
fn delete_file(state: State<AppState>, path: String) -> Result<(), String> {
    let source = PathBuf::from(&path);
    if source.exists() {
        move_to_trash(&source)?;
    }
    let thumb = thumbnail_file(&state, &path);
    if thumb.exists() {
        let _ = fs::remove_file(thumb);
    }
    let mut session = state.session.lock().map_err(|error| error.to_string())?;
    session.recent.retain(|item| item != &path);
    if session.current_path == path {
        session.current_path.clear();
    }
    persist_session(&state, &session)?;
    if let Some(dir) = github_store_dir(&state) {
        github::forget_links(&dir, Path::new(&path));
    }
    Ok(())
}

#[tauri::command]
fn save_thumbnail(state: State<AppState>, path: String, image: String) -> Result<(), String> {
    let payload = image
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(image.as_str());
    let bytes = decode_base64(payload)?;
    let dest = thumbnail_file(&state, &path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(dest, bytes).map_err(|error| error.to_string())
}

#[tauri::command]
fn boot_log(_message: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn reveal_documents(state: State<AppState>) -> Result<(), String> {
    open_external(&state.docs_dir)
}

#[tauri::command]
fn take_pending_open(
    app: AppHandle,
    state: State<AppState>,
) -> Result<Option<DesktopSession>, String> {
    let pending = state
        .pending_open
        .lock()
        .map_err(|error| error.to_string())?
        .take();
    let Some(path) = pending else {
        return Ok(None);
    };
    switch_to(&state, &app, path).map(Some)
}

// 同一个动作在三个系统里叫法不同，菜单文案跟着平台走
fn reveal_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Finder"
    } else {
        "Show in Folder"
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let app_menu = Submenu::with_items(
        app,
        "Mind",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Mind"), None)?,
            // macOS 的惯例：About 下面是检查更新
            &MenuItem::with_id(
                app,
                "check-update",
                "Check for Updates…",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("Hide Mind"))?,
            &PredefinedMenuItem::hide_others(app, Some("Hide Others"))?,
            &PredefinedMenuItem::show_all(app, Some("Show All"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some("Quit Mind"))?,
        ],
    )?;
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &MenuItem::with_id(app, "new", "New", true, Some("CmdOrCtrl+N"))?,
            &MenuItem::with_id(app, "open", "Open…", true, Some("CmdOrCtrl+O"))?,
            &MenuItem::with_id(app, "save", "Save", true, Some("CmdOrCtrl+S"))?,
            &MenuItem::with_id(app, "save-as", "Save As…", true, Some("CmdOrCtrl+Shift+S"))?,
            &MenuItem::with_id(app, "rename", "Rename…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "reveal", reveal_label(), true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "github-sign-in",
                "Sign in to GitHub…",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app,
                "github-push",
                "Push to Secret Gist",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(app, "github-pull", "Pull from Gist…", true, None::<&str>)?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::cut(app, Some("Cut"))?,
            &PredefinedMenuItem::copy(app, Some("Copy"))?,
            &PredefinedMenuItem::paste(app, Some("Paste"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::select_all(app, Some("Select All"))?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu])
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

fn ai_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join("ai.json"))
}

fn load_ai_value(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn ai_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

fn write_ai_template(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let text = serde_json::to_string_pretty(&json!({
        "baseUrl": "https://api.openai.com/v1",
        "model": "gpt-4o-mini",
        "apiKey": ""
    }))
    .map_err(|error| error.to_string())?;
    fs::write(path, text + "\n").map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiConfigStatus {
    configured: bool,
    base_url: String,
    model: String,
}

#[tauri::command]
fn ai_config_status(app: AppHandle) -> Result<AiConfigStatus, String> {
    let path = ai_config_path(&app)?;
    let value = load_ai_value(&path)?;
    let base_url = ai_text(&value, "baseUrl");
    let model = ai_text(&value, "model");
    let api_key = ai_text(&value, "apiKey");
    Ok(AiConfigStatus {
        configured: !base_url.is_empty() && !model.is_empty() && !api_key.is_empty(),
        base_url,
        model,
    })
}

#[tauri::command]
fn reveal_ai_config(app: AppHandle) -> Result<(), String> {
    let path = ai_config_path(&app)?;
    write_ai_template(&path)?;
    open_external(&path)
}

#[tauri::command]
async fn ai_complete(app: AppHandle, messages: Vec<ChatMessage>) -> Result<String, String> {
    if messages.is_empty() || messages.len() > 8 {
        return Err("请求内容无效".to_string());
    }
    for message in &messages {
        if message.role != "system" && message.role != "user" && message.role != "assistant" {
            return Err("请求内容无效".to_string());
        }
        if message.content.chars().count() > 20000 {
            return Err("请求内容过长".to_string());
        }
    }
    let path = ai_config_path(&app)?;
    let value = load_ai_value(&path)?;
    let base_url = ai_text(&value, "baseUrl");
    let model = ai_text(&value, "model");
    let api_key = ai_text(&value, "apiKey");
    if base_url.is_empty() || model.is_empty() || api_key.is_empty() {
        return Err("还没有配置 API Key".to_string());
    }
    let url = chat_completions_url(&base_url);
    let parsed = reqwest::Url::parse(&url).map_err(|error| error.to_string())?;
    let host = parsed.host_str().unwrap_or("");
    let allowed = match parsed.scheme() {
        "https" => true,
        "http" => host == "localhost" || host == "127.0.0.1" || host == "::1",
        _ => false,
    };
    if !allowed {
        return Err("只接受 https 接口，或本机的 http 接口".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(parsed)
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "messages": messages
                .iter()
                .map(|message| json!({
                    "role": message.role,
                    "content": message.content
                }))
                .collect::<Vec<_>>()
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        let clipped: String = body.chars().take(500).collect();
        return Err(format!("模型接口返回 {status}: {clipped}"));
    }
    let payload: Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    let content = payload["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if content.trim().is_empty() {
        return Err("模型没有返回内容".to_string());
    }
    Ok(content)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    version: String,
    notes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStatus {
    current_version: String,
    update: Option<UpdateInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    // 服务器没给 Content-Length 时为 None，前端据此显示不确定进度
    total: Option<u64>,
}

fn empty_progress() -> UpdateProgress {
    UpdateProgress {
        downloaded: 0,
        total: None,
    }
}

// 下载进度放在静态变量里：前端在安装过程中轮询，避免依赖事件通道
static UPDATE_PROGRESS: Mutex<UpdateProgress> = Mutex::new(UpdateProgress {
    downloaded: 0,
    total: None,
});

fn update_error(error: tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error;
    match error {
        // 还没发过带 latest.json 的版本时会出现这个错误，别把英文原文抛给用户
        Error::ReleaseNotFound => "当前发布渠道还没有提供更新包".to_string(),
        Error::TargetNotFound(target) => format!("当前平台没有可用的更新包（{target}）"),
        Error::TargetsNotFound(targets) => {
            format!("当前平台没有可用的更新包（{}）", targets.join(" / "))
        }
        other => format!("更新失败：{other}"),
    }
}

#[tauri::command]
fn desktop_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

async fn pending_update(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    match updater.check().await {
        Ok(update) => Ok(update),
        // 最新 Release 还没带 latest.json 时，当成“没有更新”，别拿报错打扰用户
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => Ok(None),
        Err(error) => Err(update_error(error)),
    }
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<UpdateStatus, String> {
    // deb 安装的客户端拿不到 AppImage 更新包，提前给出明确提示
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_none() {
        return Err("当前安装方式（deb）不支持自动更新，请到 Releases 页面下载新版本".to_string());
    }
    let current_version = app.package_info().version.to_string();
    let update = pending_update(&app).await?;
    Ok(UpdateStatus {
        current_version,
        update: update.map(|update| UpdateInfo {
            version: update.version.clone(),
            notes: update.body.clone().unwrap_or_default(),
        }),
    })
}

#[tauri::command]
async fn install_update(app: AppHandle, version: String) -> Result<(), String> {
    let Some(update) = pending_update(&app).await? else {
        return Err("没有可用的更新".to_string());
    };
    // 弹窗展示的版本如果已经不是最新的，先让用户重新确认，别装成另一个版本
    if update.version != version {
        return Err("版本已变化，请重新检查更新".to_string());
    }
    if let Ok(mut progress) = UPDATE_PROGRESS.lock() {
        *progress = empty_progress();
    }
    update
        .download_and_install(
            |chunk_length, content_length| {
                if let Ok(mut progress) = UPDATE_PROGRESS.lock() {
                    progress.downloaded += chunk_length as u64;
                    if content_length.is_some() {
                        progress.total = content_length;
                    }
                }
            },
            || {},
        )
        .await
        .map_err(update_error)?;
    app.restart();
}

#[tauri::command]
fn update_progress() -> UpdateProgress {
    UPDATE_PROGRESS
        .lock()
        .map(|progress| progress.clone())
        .unwrap_or_else(|_| empty_progress())
}

fn github_app_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

fn github_store_dir(state: &AppState) -> Option<PathBuf> {
    state.session_path.parent().map(Path::to_path_buf)
}

fn require_sync_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("请先打开一份导图再同步到 GitHub".to_string());
    }
    let file = Path::new(trimmed);
    if !file.is_file() {
        return Err("当前导图文件不存在".to_string());
    }
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("xmind") | Some("smm") | Some("json") => Ok(trimmed.to_string()),
        _ => Err("只能同步 .xmind 导图".to_string()),
    }
}

fn document_for_push(path: &str, data: Value) -> Result<Value, String> {
    if data.is_null() {
        return read_document(Path::new(path));
    }
    if !data.is_object() {
        return Err("导图数据无效".to_string());
    }
    let document = normalize_document(data);
    if document.get("root").is_none() {
        return Err("导图数据无效".to_string());
    }
    write_document(Path::new(path), &document)?;
    Ok(document)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GithubPullResult {
    needs_confirm: bool,
    gist_id: String,
    description: String,
    html_url: String,
    path: String,
    file_name: String,
    session: Option<DesktopSession>,
}

#[tauri::command]
fn github_status(app: AppHandle, path: String) -> Result<github::Status, String> {
    github::status(&github_app_dir(&app)?, path.trim())
}

#[tauri::command]
async fn github_auth_start(app: AppHandle) -> Result<github::AuthStart, String> {
    github::auth_start(&github_app_dir(&app)?).await
}

#[tauri::command]
async fn github_auth_poll(app: AppHandle) -> Result<github::AuthPoll, String> {
    github::auth_poll(&github_app_dir(&app)?).await
}

#[tauri::command]
fn github_auth_cancel() -> Result<(), String> {
    github::auth_cancel();
    Ok(())
}

#[tauri::command]
fn github_sign_out(app: AppHandle) -> Result<(), String> {
    github::sign_out(&github_app_dir(&app)?)
}

#[tauri::command]
fn github_open_url(url: String) -> Result<(), String> {
    github::allowed_github_url(&url)?;
    open_external(Path::new(url.trim()))
}

#[tauri::command]
async fn github_push(
    app: AppHandle,
    path: String,
    data: Value,
    format: String,
) -> Result<github::PushResult, String> {
    let path = require_sync_path(&path)?;
    let document = document_for_push(&path, data)?;
    github::push(&github_app_dir(&app)?, &path, &document, &format).await
}

#[tauri::command]
async fn github_pull(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    gist_id: Option<String>,
    overwrite: bool,
) -> Result<GithubPullResult, String> {
    let app_dir = github_app_dir(&app)?;
    let docs_dir = state.docs_dir.clone();
    let fetched = github::fetch_pull(&app_dir, &docs_dir, path.trim(), gist_id).await?;
    if fetched.target_exists && !overwrite {
        return Ok(GithubPullResult {
            needs_confirm: true,
            gist_id: fetched.gist_id,
            description: fetched.description,
            html_url: fetched.html_url,
            path: fetched.target.to_string_lossy().into_owned(),
            file_name: display_name(&fetched.target),
            session: None,
        });
    }
    write_document(&fetched.target, &fetched.document)?;
    github::save_link(&app_dir, &fetched)?;
    let session = switch_to(&state, &app, fetched.target.clone())?;
    Ok(GithubPullResult {
        needs_confirm: false,
        gist_id: fetched.gist_id,
        description: fetched.description,
        html_url: fetched.html_url,
        path: fetched.target.to_string_lossy().into_owned(),
        file_name: display_name(&fetched.target),
        session: Some(session),
    })
}

#[tauri::command]
async fn github_list_gists(app: AppHandle) -> Result<Vec<github::GistSummary>, String> {
    github::list_gists(&github_app_dir(&app)?).await
}

fn dispatch_menu(app: &AppHandle, action: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let script = format!("window.__desktopOnMenu && window.__desktopOnMenu({action:?})");
        let _ = window.eval(&script);
    }
}

// “用本应用打开文件”只有 apple 平台会派发 RunEvent::Opened，
// 该变体本身也是条件编译的，所以这里必须按平台分叉，否则 Windows/Linux 编译不过
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
fn handle_run_event(app: &AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::Opened { urls } = event {
        for url in urls {
            let Ok(path) = url.to_file_path() else {
                continue;
            };
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut pending) = state.pending_open.lock() {
                    *pending = Some(path);
                }
            }
            dispatch_menu(app, "open-pending");
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
fn handle_run_event(_app: &AppHandle, _event: tauri::RunEvent) {
    // Windows/Linux 没有这个事件：文件关联要靠启动参数 + 单实例插件，暂未接入
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .menu(|app| build_menu(app))
        .on_menu_event(|app, event| {
            dispatch_menu(app, event.id().as_ref());
        })
        .setup(|app| {
            let state = initial_state(app.handle()).map_err(std::io::Error::other)?;
            app.manage(state);
            // 首页没有文档时也不显示应用名：菜单栏里已经有了
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("");
            }
            // macOS 的 tauri:// 自定义协议能跑脚本，但网页视图经常是白的。
            // 开发时用的 localhost 是正常的，所以正式包也改由本机 HTTP 提供页面。
            #[cfg(not(debug_assertions))]
            {
                start_frontend_server(app.handle().clone()).map_err(std::io::Error::other)?;
                if let Some(window) = app.get_webview_window("main") {
                    let url = format!("http://127.0.0.1:{FRONTEND_PORT}/")
                        .parse()
                        .map_err(std::io::Error::other)?;
                    window.navigate(url).map_err(std::io::Error::other)?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            save_mind_map,
            save_config,
            save_lang,
            save_local_config,
            new_file,
            open_file,
            save_file_as,
            reveal_documents,
            take_pending_open,
            boot_log,
            list_files,
            library_list,
            library_create_folder,
            library_create_file,
            library_move,
            library_recent,
            library_search,
            library_rename_folder,
            library_delete_folder,
            open_path,
            rename_file,
            delete_file,
            save_thumbnail,
            ai_config_status,
            reveal_ai_config,
            ai_complete,
            github_status,
            github_auth_start,
            github_auth_poll,
            github_auth_cancel,
            github_sign_out,
            github_open_url,
            github_push,
            github_pull,
            github_list_gists,
            desktop_version,
            check_update,
            install_update,
            update_progress
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Mind")
        .run(handle_run_event);
}
