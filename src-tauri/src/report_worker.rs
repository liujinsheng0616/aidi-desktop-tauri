//! 设备信息上报守护线程模块
//!
//! 提供后台静默上报功能：
//! - 定期检查是否需要上报（每天一次）
//! - 判断条件：超过间隔天数 或 远程配置的强制上报标志
//! - 自动采集系统信息并上报到后端

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// 检查间隔（秒）：每 24 小时检查一次
/// 测试时改为 60 秒快速验证
const CHECK_INTERVAL_SECS: u64 = 60;

/// 默认上报间隔天数
const DEFAULT_INTERVAL_DAYS: u32 = 30;

/// 本地配置文件名
const CONFIG_FILE_NAME: &str = "report_config.json";

/// 认证 Token（由前端设置）
static AUTH_TOKEN: Mutex<Option<String>> = Mutex::new(None);

/// 用户信息（从本地存储获取）
static USER_INFO: Mutex<Option<(String, String)>> = Mutex::new(None); // (userCode, userName)

/// 本地上报配置
#[derive(Serialize, Deserialize, Clone)]
pub struct ReportConfig {
    /// 上次上报时间（ISO 8601）
    pub last_report_time: Option<String>,
    /// 上报间隔天数
    pub report_interval_days: u32,
    /// 远程配置版本号
    pub config_version: u32,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            last_report_time: None,
            report_interval_days: DEFAULT_INTERVAL_DAYS,
            config_version: 0,
        }
    }
}

/// 远程配置响应
#[derive(Deserialize)]
struct RemoteConfig {
    #[serde(rename = "forceReport")]
    force_report: bool,
    #[serde(rename = "configVersion")]
    config_version: i32,
    #[serde(rename = "defaultIntervalDays")]
    default_interval_days: i32,
}

/// 设备信息上报请求体
#[derive(Serialize)]
struct DeviceReportRequest {
    #[serde(rename = "userCode")]
    user_code: String,
    #[serde(rename = "userName")]
    user_name: String,
    hostname: String,
    ip: String,
    manufacturer: String,
    model: String,
    #[serde(rename = "serialNumber")]
    serial_number: String,
    #[serde(rename = "manufactureDate")]
    manufacture_date: String,
    #[serde(rename = "osName")]
    os_name: String,
    #[serde(rename = "osVersion")]
    os_version: String,
    #[serde(rename = "osArch")]
    os_arch: String,
    #[serde(rename = "osInstallDate")]
    os_install_date: String,
    #[serde(rename = "osLastBoot")]
    os_last_boot: String,
    #[serde(rename = "cpuName")]
    cpu_name: String,
    #[serde(rename = "cpuCores")]
    cpu_cores: i32,
    #[serde(rename = "memoryGb")]
    memory_gb: f64,
    #[serde(rename = "storageGb")]
    storage_gb: f64,
    #[serde(rename = "gpuName")]
    gpu_name: String,
}

/// 安全地获取锁（处理 poisoned mutex）
fn safe_lock<T>(mutex: &Mutex<T>) -> Option<std::sync::MutexGuard<'_, T>> {
    mutex.lock().ok()
}

/// 设置认证 Token（由前端调用）
pub fn set_auth_token(token: String) {
    if let Some(mut auth_token) = safe_lock(&AUTH_TOKEN) {
        *auth_token = Some(token);
    }
}

/// 获取认证 Token
fn get_auth_token() -> Option<String> {
    safe_lock(&AUTH_TOKEN).and_then(|auth_token| auth_token.clone())
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

/// 获取 API 基础 URL
fn get_api_base_url() -> String {
    // 优先读取环境变量
    if let Ok(url) = std::env::var("AIDI_API_URL") {
        return url;
    }

    // 通过环境变量 AIDI_ENV 决定使用哪个环境
    let env = std::env::var("AIDI_ENV").unwrap_or_else(|_| "dev".to_string());

    match env.as_str() {
        "test" => "https://microsapitest.yadea.com.cn".to_string(),
        "prod" => "https://aidi.yadea.com.cn".to_string(),
        _ => "http://127.0.0.1:9900".to_string(), // 开发环境默认后端地址
    }
}

/// 获取远程配置
async fn fetch_remote_config() -> Result<RemoteConfig, String> {
    let token = get_auth_token().ok_or("未登录，无法获取远程配置")?;

    let base_url = get_api_base_url();
    let url = format!("{}/api-aigc/device/info/config", base_url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("请求远程配置失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("远程配置接口返回错误: {}", response.status()));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析远程配置响应失败: {}", e))?;

    // 解析响应结构：{ "resp_code": 0, "data": { ... } }
    let data = json
        .get("data")
        .ok_or("远程配置响应缺少 data 字段")?;

    let config: RemoteConfig = serde_json::from_value(data.clone())
        .map_err(|e| format!("解析远程配置失败: {}", e))?;

    Ok(config)
}

/// 判断是否需要上报
fn should_report(local: &ReportConfig, remote: &RemoteConfig) -> bool {
    println!("[ReportWorker] 本地配置: last_time={:?}, interval={}, version={}",
             local.last_report_time, local.report_interval_days, local.config_version);
    println!("[ReportWorker] 远程配置: force={}, interval={}, version={}",
             remote.force_report, remote.default_interval_days, remote.config_version);

    // 条件1：远程配置的强制版本 > 本地版本（立即上报）
    if remote.force_report && remote.config_version > local.config_version as i32 {
        println!("[ReportWorker] 触发上报：远程强制上报标志（版本 {} > {}）",
                 remote.config_version, local.config_version);
        return true;
    }

    // 条件2：从未上报过
    if local.last_report_time.is_none() {
        println!("[ReportWorker] 触发上报：从未上报过");
        return true;
    }

    // 条件3：距离上次上报 >= 间隔天数
    // 远程配置优先，如果远程配置为0则表示每次都上报
    let interval_days = remote.default_interval_days.max(0) as u32;

    if let Some(ref last) = local.last_report_time {
        if let Ok(last_time) = DateTime::parse_from_rfc3339(last) {
            let last_utc: DateTime<Utc> = last_time.with_timezone(&Utc);
            let now = Utc::now();
            let days = (now - last_utc).num_days();
            println!("[ReportWorker] 距离上次上报 {} 天，间隔 {} 天", days, interval_days);
            if days >= interval_days as i64 {
                println!("[ReportWorker] 触发上报：超过间隔天数（{} >= {}）",
                         days, interval_days);
                return true;
            }
        }
    }

    false
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
    use std::process::Command;

    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let mut path = exe_path.clone();
    path.pop();
    let script_path = path.join("scripts").join("system-info.ps1");
    let script_path_str = script_path.to_string_lossy().to_string();

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &script_path_str])
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
    }
}

/// 上报设备信息
async fn report_device_info(
    system_info: &serde_json::Value,
    user_code: &str,
    user_name: &str,
) -> Result<(), String> {
    let token = get_auth_token().ok_or("未登录，无法上报设备信息")?;

    let base_url = get_api_base_url();
    let url = format!("{}/api-aigc/device/info/report", base_url);

    let report_data = extract_report_data(system_info, user_code, user_name);

    println!("[ReportWorker] 上报数据: {}", serde_json::to_string_pretty(&report_data).unwrap_or_default());

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&report_data)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("上报设备信息失败: {}", e))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    println!("[ReportWorker] 上报响应: {} - {}", status, body);

    if !status.is_success() {
        return Err(format!("上报设备信息失败: {} - {}", status, body));
    }

    println!("[ReportWorker] 设备信息上报成功");
    Ok(())
}

/// 执行检查和上报
async fn check_and_report(app: &tauri::AppHandle) -> Result<(), String> {
    println!("[ReportWorker] 开始检查是否需要上报...");

    // 1. 检查是否已登录
    if get_auth_token().is_none() {
        println!("[ReportWorker] 未登录，跳过上报检查");
        return Ok(());
    }

    let user_info = get_user_info();
    if user_info.is_none() {
        println!("[ReportWorker] 无用户信息，跳过上报检查");
        return Ok(());
    }
    let (user_code, user_name) = user_info.unwrap();

    // 2. 读取本地配置
    let local_config = read_local_config(app);

    // 3. 获取远程配置
    let remote_config = match fetch_remote_config().await {
        Ok(config) => config,
        Err(e) => {
            println!("[ReportWorker] 获取远程配置失败: {}，使用本地配置", e);
            // 如果无法获取远程配置，仅检查时间间隔
            if let Some(ref last) = local_config.last_report_time {
                if let Ok(last_time) = DateTime::parse_from_rfc3339(last) {
                    let days = (Utc::now() - last_time.with_timezone(&Utc)).num_days();
                    if days < local_config.report_interval_days as i64 {
                        println!("[ReportWorker] 距离上次上报 {} 天，未到间隔，跳过", days);
                        return Ok(());
                    }
                }
            }
            // 创建一个默认的远程配置
            RemoteConfig {
                force_report: false,
                config_version: local_config.config_version as i32,
                default_interval_days: local_config.report_interval_days as i32,
            }
        }
    };

    // 4. 判断是否需要上报
    if !should_report(&local_config, &remote_config) {
        println!("[ReportWorker] 不满足上报条件，跳过");
        return Ok(());
    }

    // 5. 采集系统信息
    println!("[ReportWorker] 开始采集系统信息...");
    let system_info = run_system_info_script()?;

    // 6. 上报数据
    println!("[ReportWorker] 开始上报设备信息...");
    report_device_info(&system_info, &user_code, &user_name).await?;

    // 7. 更新本地配置
    let updated = ReportConfig {
        last_report_time: Some(Utc::now().to_rfc3339()),
        report_interval_days: remote_config.default_interval_days as u32,
        config_version: remote_config.config_version as u32,
    };
    save_local_config(app, &updated);

    println!("[ReportWorker] 上报流程完成");
    Ok(())
}

/// 启动后台上报守护线程
pub fn start_report_worker(app: tauri::AppHandle) {
    println!("[ReportWorker] 启动后台上报守护线程");

    // 使用 std::thread 创建独立的后台线程
    std::thread::spawn(move || {
        // 在线程内创建 Tokio runtime
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        rt.block_on(async {
            // 启动时等待一段时间，让前端完成初始化
            // 测试时改为 5 秒快速验证
            tokio::time::sleep(Duration::from_secs(5)).await;

            loop {
                // 执行检查和上报
                if let Err(e) = check_and_report(&app).await {
                    eprintln!("[ReportWorker] 检查/上报出错: {}", e);
                }

                // 休眠等待下次检查
                tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
            }
        });
    });
}

/// 立即触发一次上报（供前端手动调用）
pub async fn trigger_report_now(app: &tauri::AppHandle) -> Result<(), String> {
    check_and_report(app).await
}
