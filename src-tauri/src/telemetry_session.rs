use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static GENERATION: Mutex<u64> = Mutex::new(0);
const FAIDI_ORIGIN: &str = "https://faidi.yadea.com:1367";

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySession {
    token: String,
    user_id: String,
    expires_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    collector: Option<String>,
}

fn session_path() -> Option<PathBuf> {
    let file_name = if uses_faidi() { "telemetry-session-faidi.json" } else { "telemetry-session.json" };
    Some(dirs::data_local_dir()?.join("AIDI Desktop").join(file_name))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

pub fn invalidate() -> Option<u64> {
    let mut generation = GENERATION.lock().ok()?;
    *generation = generation.wrapping_add(1);
    if let Some(path) = session_path() {
        if let Err(error) = std::fs::remove_file(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return None;
            }
        }
    }
    Some(*generation)
}

fn backend_url() -> Option<reqwest::Url> {
    resolve_backend_url(option_env!("AIDI_TELEMETRY_URL")?)
}

fn resolve_backend_url(value: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(value).ok()?;
    let host = url.host_str()?;
    let allowed = match (url.scheme(), host) {
        ("https", "faidi.yadea.com") => url.port() == Some(1367),
        ("https", "aidi.yadea.com.cn" | "microsapptest.yadea.com.cn") => url.port().is_none(),
        ("http", "localhost" | "127.0.0.1") => url.port() == Some(1367),
        _ => false,
    };
    if !allowed || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    url.set_path("/api/auth/telemetry");
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

fn uses_faidi() -> bool {
    backend_url().is_some_and(|url| url.origin().ascii_serialization() == FAIDI_ORIGIN)
}

fn save(session: &TelemetrySession, expected_generation: u64) -> Option<()> {
    use std::io::Write;

    let generation = GENERATION.lock().ok()?;
    if *generation != expected_generation {
        return None;
    }
    let path = session_path()?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    let pending = path.with_extension(format!("{}.tmp", now_ms()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&pending).ok()?;
    file.write_all(&serde_json::to_vec(session).ok()?).ok()?;
    drop(file);
    std::fs::rename(pending, path).ok()
}

pub async fn establish(feishu_access_token: String, employee_no: Option<&str>) {
    let Some(generation) = invalidate() else { return };
    let Some(employee_no) = employee_no.map(str::trim).filter(|id| !id.is_empty()) else { return };
    let Some(url) = backend_url() else { return };

    let Ok(client) = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(7))
        .build() else { return };
    let collector = url.origin().ascii_serialization();
    let Ok(response) = client.post(url).bearer_auth(feishu_access_token).send().await else { return };
    let Ok(response) = response.error_for_status() else { return };
    if let Ok(mut session) = response.json::<TelemetrySession>().await {
        if session.user_id == employee_no && session.expires_at > now_ms() {
            session.collector = Some(collector);
            save(&session, generation);
        }
    }
}

#[tauri::command]
pub fn get_telemetry_session(window: tauri::WebviewWindow, collector: Option<String>) -> Option<TelemetrySession> {
    let url = window.url().ok()?;
    let host = url.host_str()?;
    let trusted = match (url.scheme(), host) {
        ("https", "aidi.yadea.com.cn" | "microsapptest.yadea.com.cn") => url.port().is_none(),
        ("http", "localhost" | "127.0.0.1") => {
            url.port() == Some(5173) || (cfg!(debug_assertions) && url.port() == Some(1420))
        }
        ("tauri", "localhost") | ("http" | "https", "tauri.localhost") => true,
        _ => false,
    };
    if !trusted || !matches!(window.label(), "main" | "menu" | "submenu" | "optimizer" | "panel" | "chat") {
        return None;
    }
    let _generation = GENERATION.lock().ok()?;
    let path = session_path()?;
    let data = std::fs::read(&path).ok()?;
    let session: TelemetrySession = serde_json::from_slice(&data).ok()?;
    // Older online Web builds omit the collector and must not receive the Faidi token.
    if uses_faidi() && (collector.as_deref() != Some(FAIDI_ORIGIN) || session.collector.as_deref() != Some(FAIDI_ORIGIN)) {
        return None;
    }
    let auth: serde_json::Value = serde_json::from_slice(&std::fs::read(path.with_file_name("auth.json")).ok()?).ok()?;
    let user = match auth.get("user")? {
        serde_json::Value::String(value) => serde_json::from_str(value).ok()?,
        value => value.clone(),
    };
    let employee_no = user.get("fsUserId")?.as_str()?.trim();
    (session.expires_at > now_ms() && employee_no == session.user_id).then_some(session)
}
