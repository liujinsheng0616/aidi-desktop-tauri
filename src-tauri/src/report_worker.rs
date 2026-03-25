//! 设备信息上报守护线程模块
//!
//! 提供后台静默上报功能：
//! - 定期检查是否需要上报（每 24 小时）
//! - 自动采集系统信息并上报到飞书多维表格
//! - 从远程配置获取上报间隔

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::feishu::bitable::report_device;
use crate::feishu::types::DeviceReportRequest;

/// 检查间隔（秒）：每 24 小时检查一次
const CHECK_INTERVAL_SECS: u64 = 86400;

/// 默认上报间隔天数（30天）
const DEFAULT_INTERVAL_DAYS: u32 = 30;

/// 远程配置地址
const REMOTE_CONFIG_URL: &str = "https://oss.yadea.com.cn/aigc/aidi-report.json";

/// 本地配置文件名
const CONFIG_FILE_NAME: &str = "report_config.json";

/// 用户信息（从登录获取）
static USER_INFO: Mutex<Option<(String, String)>> = Mutex::new(None); // (userCode, userName)

/// 远程配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteConfig {
    /// 是否启用上报
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 上报间隔天数
    #[serde(default = "default_interval_days")]
    pub interval_days: u32,
}

fn default_enabled() -> bool { true }
fn default_interval_days() -> u32 { 30 }

/// 本地上报配置
#[derive(Serialize, Deserialize, Clone)]
pub struct ReportConfig {
    /// 上次上报时间（ISO 8601）
    pub last_report_time: Option<String>,
    /// 上报间隔天数
    pub report_interval_days: u32,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            last_report_time: None,
            report_interval_days: DEFAULT_INTERVAL_DAYS,
        }
    }
}

/// 安全地获取锁
fn safe_lock<T>(mutex: &Mutex<T>) -> Option<std::sync::MutexGuard<'_, T>> {
    mutex.lock().ok()
}

/// 设置用户信息（由前端调用）
pub fn set_user_info(user_code: String, user_name: String) {
    if let Some(mut user_info) = safe_lock(&USER_INFO) {
        *user_info = Some((user_code, user_name));
    }
}

/// 获取用户信息
fn get_user_info() -> Option<(String, String)> {
    safe_lock(&USER_INFO).and_then(|user_info| user_info.clone())
}

/// 获取远程配置
async fn fetch_remote_config() -> RemoteConfig {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok();

    if let Some(client) = client {
        match client.get(REMOTE_CONFIG_URL).send().await {
            Ok(resp) => {
                if let Ok(config) = resp.json::<RemoteConfig>().await {
                    println!("[ReportWorker] 远程配置: enabled={}, interval_days={}", config.enabled, config.interval_days);
                    return config;
                }
            }
            Err(e) => {
                println!("[ReportWorker] 获取远程配置失败: {}", e);
            }
        }
    }

    RemoteConfig::default()
}

/// 获取配置文件路径
fn get_config_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let config_dir = app.path().app_data_dir().ok()?;
    Some(config_dir.join(CONFIG_FILE_NAME))
}

/// 读取本地配置
fn read_local_config(app: &tauri::AppHandle) -> ReportConfig {
    let path = match get_config_path(app) {
        Some(p) => p,
        None => return ReportConfig::default(),
    };

    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
    }
    ReportConfig::default()
}

/// 保存本地配置
fn save_local_config(app: &tauri::AppHandle, config: &ReportConfig) {
    let path = match get_config_path(app) {
        Some(p) => p,
        None => return,
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(&path, content);
    }
}

/// 执行系统信息采集脚本
#[cfg(not(target_os = "windows"))]
fn run_system_info_script() -> Result<serde_json::Value, String> {
    use std::process::Command;

    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let mut path = exe_path.clone();
    path.pop();
    let script_path = path.join("scripts").join("system-info.sh");

    let output = Command::new("/bin/bash")
        .arg(&script_path)
        .output()
        .map_err(|e| format!("执行系统信息脚本失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("系统信息脚本执行失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).map_err(|e| format!("解析系统信息输出失败: {}", e))
}

/// 执行系统信息采集脚本 (Windows)
#[cfg(target_os = "windows")]
fn run_system_info_script() -> Result<serde_json::Value, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let mut path = exe_path.clone();
    path.pop();
    let script_path = path.join("scripts").join("system-info.ps1");
    let script_path_str = script_path.to_string_lossy().to_string();

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &script_path_str])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行系统信息脚本失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("系统信息脚本执行失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).map_err(|e| format!("解析系统信息输出失败: {}", e))
}

/// 从系统信息 JSON 中提取上报数据
fn extract_report_data(system_info: &serde_json::Value, user_code: &str, user_name: &str) -> DeviceReportRequest {
    let details = system_info.get("details").cloned().unwrap_or(serde_json::json!({}));
    let os = details.get("os").cloned().unwrap_or(serde_json::json!({}));
    let cpu = details.get("cpu").cloned().unwrap_or(serde_json::json!({}));
    let memory = details.get("memory").cloned().unwrap_or(serde_json::json!({}));
    let gpu = details.get("gpu").cloned().unwrap_or(serde_json::json!({}));
    let storage = details.get("storage").cloned().unwrap_or(serde_json::json!({}));

    DeviceReportRequest {
        user_code: user_code.to_string(),
        user_name: user_name.to_string(),
        hostname: details.get("hostname").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        ip: details.get("ip").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        manufacturer: details.get("manufacturer").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        model: details.get("model").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        serial_number: details.get("serialNumber").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        manufacture_date: details.get("manufactureDate").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        os_name: os.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        os_version: os.get("version").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        os_arch: os.get("architecture").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        os_install_date: os.get("installDate").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        os_last_boot: os.get("lastBoot").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        cpu_name: cpu.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        cpu_cores: cpu.get("cores").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        memory_gb: memory.get("totalGB").and_then(|v| v.as_f64()).unwrap_or(0.0),
        storage_gb: storage.get("totalGB").and_then(|v| v.as_f64()).unwrap_or(0.0),
        gpu_name: gpu.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// 上报设备信息到飞书多维表格
async fn report_device_info(
    system_info: &serde_json::Value,
    user_code: &str,
    user_name: &str,
) -> Result<(), String> {
    let report_data = extract_report_data(system_info, user_code, user_name);
    report_device(&report_data).await?;
    Ok(())
}

/// 执行检查和上报
async fn check_and_report(app: &tauri::AppHandle) -> Result<(), String> {
    // 1. 检查是否有用户信息
    let user_info = get_user_info();
    if user_info.is_none() {
        return Ok(());
    }
    let (user_code, user_name) = user_info.unwrap();

    // 2. 获取远程配置
    let remote_config = fetch_remote_config().await;
    if !remote_config.enabled {
        println!("[ReportWorker] 远程配置已禁用上报");
        return Ok(());
    }

    // 3. 读取本地配置
    let local_config = read_local_config(app);

    // 4. 检查时间间隔（使用远程配置的间隔）
    if let Some(ref last) = local_config.last_report_time {
        if let Ok(last_time) = DateTime::parse_from_rfc3339(last) {
            let days = (Utc::now() - last_time.with_timezone(&Utc)).num_days();
            if days < remote_config.interval_days as i64 {
                println!("[ReportWorker] 未到上报时间: 距上次 {} 天，要求 {} 天", days, remote_config.interval_days);
                return Ok(());
            }
        }
    }

    // 5. 采集系统信息
    let system_info = run_system_info_script()?;

    // 6. 上报数据到飞书多维表格
    report_device_info(&system_info, &user_code, &user_name).await?;
    println!("[ReportWorker] ✅ 上报成功: {} - {}", user_name, user_code);

    // 7. 更新本地配置
    let updated = ReportConfig {
        last_report_time: Some(Utc::now().to_rfc3339()),
        report_interval_days: remote_config.interval_days,
    };
    save_local_config(app, &updated);

    Ok(())
}

/// 启动后台上报守护线程
pub fn start_report_worker(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        rt.block_on(async {
            // 启动时等待 30 秒，让前端完成初始化
            tokio::time::sleep(Duration::from_secs(30)).await;

            loop {
                if let Err(e) = check_and_report(&app).await {
                    println!("[ReportWorker] ❌ 上报失败: {}", e);
                }
                tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
            }
        });
    });
}

/// 立即触发一次上报（供前端手动调用）
pub async fn trigger_report_now(app: &tauri::AppHandle) -> Result<(), String> {
    check_and_report(app).await
}
