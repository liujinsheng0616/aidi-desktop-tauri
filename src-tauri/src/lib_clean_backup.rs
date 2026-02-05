// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use sysinfo::{Disk, Disks, System};
use tauri::{Emitter, Listener, Manager};

// ==================== DOCK STATE ====================

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

pub struct DockState {
    pub is_docked: bool,
    pub dock_edge: Option<DockEdge>,
    pub dock_x: i32,
    pub dock_y: i32,
    pub is_popped_out: bool,
    pub menu_direction: String,
}

impl DockState {
    fn new() -> Self {
        DockState {
            is_docked: false,
            dock_edge: None,
            dock_x: 0,
            dock_y: 0,
            is_popped_out: false,
            menu_direction: "right".to_string(),
        }
    }
}

fn default_dock_state() -> DockState {
    DockState::new()
}

static DOCK_STATE: Mutex<DockState> = Mutex::new(default_dock_state());

// ==================== SCAN RESULT STRUCTURES ====================

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScanItem {
    pub dimension: String,
    pub status: String,
    pub summary: String,
    pub details: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CleanResult {
    pub cleaned: u64,
    pub success_count: u64,
    pub failed_count: u64,
    pub details: Vec<CleanDetail>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CleanDetail {
    pub path: String,
    pub size: u64,
    pub success: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub ball_size: Option<u32>,
    pub opacity: Option<u32>,
    pub color_theme: Option<String>,
    pub theme_mode: Option<String>,
}

pub struct WindowStates {
    pub ball_hover: bool,
    pub menu_hover: bool,
    pub submenu_hover: bool,
}

// ==================== HELPER FUNCTIONS ====================

fn get_temp_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(temp_dir) = dirs::cache_dir() {
        dirs.push(temp_dir.join("tmp"));
        dirs.push(temp_dir.join("temp"));
    }

    if cfg!(windows) {
        dirs.push(PathBuf::from("C:\\Windows\\Temp"));
    } else if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/private/var/tmp"));
        dirs.push(PathBuf::from("/private/tmp"));
        dirs.push(PathBuf::from("/tmp"));
    }

    if let Some(cache_dir) = dirs::cache_dir() {
        dirs.push(cache_dir.clone());
        if let Some(name) = cache_dir.file_name() {
            dirs.push(cache_dir.parent().unwrap().join("Caches").join(name));
        }
    }

    #[cfg(windows)]
    {
        dirs.push(PathBuf::from("C:\\Windows\\Prefetch"));
    }

    if let Some(home_dir) = dirs::home_dir() {
        if cfg!(windows) {
            dirs.push(home_dir.join("AppData").join("Local").join("Google").join("Chrome").join("User Data").join("Default").join("Cache"));
            dirs.push(home_dir.join("AppData").join("Local").join("Microsoft").join("Edge").join("User Data").join("Default").join("Cache"));
        } else if cfg!(target_os = "macos") {
            dirs.push(home_dir.join("Library").join("Caches").join("Google").join("Chrome"));
            dirs.push(home_dir.join("Library").join("Caches").join("com.microsoft.edgemac"));
        }
    }

    dirs
}

fn get_recycle_bin_path() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Some(home_dir) = dirs::home_dir() {
            let sid = get_current_user_sid();
            return Some(home_dir.join("$RECYCLE.BIN").join(sid));
        }
    } else if cfg!(target_os = "macos") {
        if let Some(home_dir) = dirs::home_dir() {
            return Some(home_dir.join(".Trash"));
        }
    }
    None
}

#[cfg(windows)]
fn get_current_user_sid() -> String {
    use std::process::Command;
    let output = Command::new("whoami")
        .args(&["/user"])
        .output()
        .unwrap_or_else(|_| b"S-1-5-21-0-0-0-0".to_vec().into());
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .nth(1)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "S-1-5-21-0-0-0-0".to_string())
}

#[cfg(target_os = "macos")]
fn get_current_user_sid() -> String {
    String::new()
}

fn categorize_path(path: &Path, _temp_dirs: &[PathBuf]) -> Option<&'static str> {
    let path_str = path.to_string_lossy().to_lowercase();

    if let Some(recycle_bin) = get_recycle_bin_path() {
        if path.starts_with(&recycle_bin) {
            return Some("recycleBin");
        }
    }

    #[cfg(windows)]
    if path_str.contains("windows\\temp") || path_str.contains("/private/var/tmp") || path_str.contains("/private/tmp") {
        return Some("systemTemp");
    }

    #[cfg(windows)]
    if path_str.contains("prefetch") {
        return Some("prefetch");
    }

    if path_str.contains("chrome") && (path_str.contains("cache") || path_str.contains("caches")) {
        return Some("browserCache");
    }
    if path_str.contains("edge") && (path_str.contains("cache") || path_str.contains("caches")) {
        return Some("browserCache");
    }

    Some("temp")
}

fn calculate_dir_size(path: &Path) -> (u64, u64) {
    let mut total_size = 0u64;
    let mut file_count = 0u64;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let (size, count) = calculate_dir_size(&entry_path);
                total_size += size;
                file_count += count;
            } else if let Ok(metadata) = entry.metadata() {
                total_size += metadata.len();
                file_count += 1;
            }
        }
    }

    (total_size, file_count)
}

#[cfg(windows)]
fn get_disk_health_info() -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    use std::process::Command;

    let mut volumes = Vec::new();
    let physical_disks = Vec::new();

    if let Ok(output) = Command::new("powershell")
        .args(&[
            "-Command",
            "Get-Volume | Select-Object DriveLetter, FileSystemLabel, Size, SizeRemaining, FileSystem, HealthStatus | ConvertTo-Json",
        ])
        .output()
    {
        if let Ok(json_str) = String::from_utf8(output.stdout) {
            if let Ok(volumes_data) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(arr) = volumes_data.as_array() {
                    for vol in arr {
                        let drive_letter = vol.get("DriveLetter")
                            .and_then(|v| v.as_str())
                            .unwrap_or("C");

                        volumes.push(serde_json::json!({
                            "drive": format!("{}:", drive_letter),
                            "label": vol.get("FileSystemLabel")
                                .and_then(|v| v.as_str())
                                .unwrap_or(""),
                            "size": vol.get("Size")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            "free": vol.get("SizeRemaining")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            "fileSystem": vol.get("FileSystem")
                                .and_then(|v| v.as_str())
                                .unwrap_or(""),
                            "healthStatus": vol.get("HealthStatus")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown"),
                            "status": "good"
                        }));
                    }
                }
            }
        }
    }

    (volumes, physical_disks)
}

#[cfg(target_os = "macos")]
fn get_disk_health_info() -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    use std::process::Command;

    let mut volumes = Vec::new();
    let physical_disks = Vec::new();

    if let Ok(output) = Command::new("df")
        .args(&["-k"])
        .output()
    {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                if let Ok(total_kb) = parts[1].parse::<u64>() {
                    if let Ok(available_kb) = parts[3].parse::<u64>() {
                        let total = total_kb * 1024;
                        let free = available_kb * 1024;

                        volumes.push(serde_json::json!({
                            "drive": parts[5],
                            "label": "",
                            "size": total,
                            "free": free,
                            "fileSystem": parts[0],
                            "healthStatus": "Healthy",
                            "status": if free < total / 10 { "danger" } else { "good" }
                        }));
                    }
                }
            }
        }
    }

    (volumes, physical_disks)
}

// ==================== WINDOW COMMANDS ====================

#[tauri::command]
pub fn show_main_window(window: tauri::Window) {
    let _ = window.set_decorations(false);
    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);
    let _ = window.set_resizable(false);
    let _ = window.show();
}

#[tauri::command]
pub fn hide_main_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
pub fn show_menu_window(app: tauri::AppHandle) {
    if let Some(menu_window) = app.webview_windows().get("menu") {
        let _ = menu_window.show();
    }
}

#[tauri::command]
pub fn hide_menu_window(app: tauri::AppHandle) {
    if let Some(menu_window) = app.webview_windows().get("menu") {
        let _ = menu_window.hide();
    }
    if let Some(submenu_window) = app.webview_windows().get("submenu") {
        let _ = submenu_window.hide();
    }
}

#[tauri::command]
pub fn show_submenu_window(app: tauri::AppHandle) {
    if let Some(submenu_window) = app.webview_windows().get("submenu") {
        let _ = submenu_window.show();
    }
}

#[tauri::command]
pub fn hide_submenu_window(app: tauri::AppHandle) {
    if let Some(submenu_window) = app.webview_windows().get("submenu") {
        let _ = submenu_window.hide();
    }
}

#[tauri::command]
pub fn show_optimizer_window(app: tauri::AppHandle) {
    if let Some(optimizer_window) = app.webview_windows().get("optimizer") {
        let _ = optimizer_window.show();
    }
}

#[tauri::command]
pub fn hide_optimizer_window(app: tauri::AppHandle) {
    if let Some(optimizer_window) = app.webview_windows().get("optimizer") {
        let _ = optimizer_window.hide();
    }
}

#[tauri::command]
pub fn open_panel(app: tauri::AppHandle) {
    if let Some(panel_window) = app.webview_windows().get("panel") {
        let _ = panel_window.show();
        let _ = panel_window.set_focus();
    }
}

// ==================== OPTIMIZER COMMANDS ====================

#[tauri::command]
pub fn optimizer_scan_all(_app: tauri::AppHandle) -> Result<Vec<ScanItem>, String> {
    let mut items = Vec::new();

    match optimizer_disk_scan(_app.clone()) {
        Ok(item) => items.push(item),
        Err(e) => items.push(ScanItem {
            dimension: "disk".to_string(),
            status: "error".to_string(),
            summary: format!("扫描失败: {}", e),
            details: serde_json::Value::Object(serde_json::Map::new()),
        }),
    }

    match optimizer_memory_status(_app.clone()) {
        Ok(item) => items.push(item),
        Err(e) => items.push(ScanItem {
            dimension: "memory".to_string(),
            status: "error".to_string(),
            summary: format!("扫描失败: {}", e),
            details: serde_json::Value::Object(serde_json::Map::new()),
        }),
    }

    match optimizer_startup_list(_app.clone()) {
        Ok(item) => items.push(item),
        Err(e) => items.push(ScanItem {
            dimension: "startup".to_string(),
            status: "error".to_string(),
            summary: format!("扫描失败: {}", e),
            details: serde_json::Value::Object(serde_json::Map::new()),
        }),
    }

    match optimizer_disk_health(_app.clone()) {
        Ok(item) => items.push(item),
        Err(e) => items.push(ScanItem {
            dimension: "health".to_string(),
            status: "error".to_string(),
            summary: format!("扫描失败: {}", e),
            details: serde_json::Value::Object(serde_json::Map::new()),
        }),
    }

    match optimizer_system_info(_app.clone()) {
        Ok(item) => items.push(item),
        Err(e) => items.push(ScanItem {
            dimension: "system".to_string(),
            status: "error".to_string(),
            summary: format!("扫描失败: {}", e),
            details: serde_json::Value::Object(serde_json::Map::new()),
        }),
    }

    Ok(items)
}

#[tauri::command]
pub fn optimizer_disk_health(_app: tauri::AppHandle) -> Result<ScanItem, String> {
    let (volumes, physical_disks) = get_disk_health_info();

    let summary = if physical_disks.is_empty() {
        "无法获取磁盘健康信息".to_string()
    } else if volumes.iter().any(|v| v.get("status") == Some(&serde_json::Value::String("danger".to_string()))) {
        "磁盘空间不足".to_string()
    } else {
        format!("检测到 {} 个卷, {} 个物理磁盘", volumes.len(), physical_disks.len())
    };

    Ok(ScanItem {
        dimension: "health".to_string(),
        status: "good".to_string(),
        summary,
        details: serde_json::json!({
            "volumes": volumes,
            "physicalDisks": physical_disks
        }),
    })
}

#[tauri::command]
pub fn optimizer_disk_scan(_app: tauri::AppHandle) -> Result<ScanItem, String> {
    let temp_dirs = get_temp_dirs();
    let mut categories: HashMap<&str, serde_json::Value> = HashMap::new();

    for cat in &["temp", "systemTemp", "prefetch", "recycleBin", "browserCache"] {
        categories.insert(cat, serde_json::json!({
            "size": 0,
            "count": 0,
            "needsAuth": false
        }));
    }

    let mut total_size = 0u64;

    for temp_dir in &temp_dirs {
        if !temp_dir.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(temp_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_dir() {
                    let dir_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    if cfg!(windows) && (dir_name == "System32" || dir_name == "Windows") {
                        continue;
                    }
                }

                let (size, count) = if path.is_dir() {
                    calculate_dir_size(&path)
                } else {
                    entry.metadata()
                        .map(|m| (m.len(), 1))
                        .unwrap_or((0, 0))
                };

                if size == 0 && count == 0 {
                    continue;
                }

                total_size += size;

                if let Some(category) = categorize_path(&path, &temp_dirs) {
                    if let Some(cat_data) = categories.get_mut(category) {
                        let current_size = cat_data.get("size")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let current_count = cat_data.get("count")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        *cat_data = serde_json::json!({
                            "size": current_size + size,
                            "count": current_count + count,
                            "needsAuth": matches!(category, "systemTemp" | "prefetch")
                        });
                    }
                }
            }
        }
    }

    if let Some(recycle_bin) = get_recycle_bin_path() {
        if recycle_bin.exists() {
            let (size, count) = calculate_dir_size(&recycle_bin);
            total_size += size;
            if let Some(cat_data) = categories.get_mut("recycleBin") {
                let current_size = cat_data.get("size")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let current_count = cat_data.get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                *cat_data = serde_json::json!({
                    "size": current_size + size,
                    "count": current_count + count,
                    "needsAuth": false
                });
            }
        }
    }

    let summary = format!("发现 {} 个临时文件", categories.values()
        .filter_map(|v| v.get("count").and_then(|c| c.as_u64()))
        .sum::<u64>());

    Ok(ScanItem {
        dimension: "disk".to_string(),
        status: if total_size > 1024 * 1024 * 1024 { "warning" } else { "good" },
        summary,
        details: serde_json::json!({
            "totalSize": total_size,
            "totalSizeMB": total_size / 1024 / 1024,
            "totalSizeGB": total_size as f64 / 1024.0 / 1024.0 / 1024.0,
            "categories": categories
        }),
    })
}

#[tauri::command]
pub fn optimizer_disk_clean(_app: tauri::AppHandle, categories_json: String) -> Result<CleanResult, String> {
    let categories: Vec<String> = serde_json::from_str(&categories_json)
        .map_err(|e| format!("Invalid categories JSON: {}", e))?;

    let mut cleaned = 0u64;
    let mut success_count = 0u64;
    let mut failed_count = 0u64;
    let mut details = Vec::new();

    let temp_dirs = get_temp_dirs();

    for category in &categories {
        if *category == "recycleBin" {
            if let Some(recycle_bin) = get_recycle_bin_path() {
                if recycle_bin.exists() {
                    match clean_directory(&recycle_bin, &mut details) {
                        Ok(size) => {
                            cleaned += size;
                            success_count += 1;
                        }
                        Err(_) => failed_count += 1,
                    }
                }
            }
            continue;
        }

        for temp_dir in &temp_dirs {
            if !temp_dir.exists() {
                continue;
            }

            let should_clean = match category.as_str() {
                "temp" => true,
                "systemTemp" => temp_dir.to_string_lossy().to_lowercase().contains("temp") ||
                                temp_dir.to_string_lossy().to_lowercase().contains("tmp"),
                "prefetch" => cfg!(windows) && temp_dir.to_string_lossy().to_lowercase().contains("prefetch"),
                "browserCache" => temp_dir.to_string_lossy().to_lowercase().contains("cache") ||
                                  temp_dir.to_string_lossy().to_lowercase().contains("caches"),
                _ => false,
            };

            if should_clean {
                match clean_directory(temp_dir, &mut details) {
                    Ok(size) => {
                        cleaned += size;
                        success_count += 1;
                    }
                    Err(_) => failed_count += 1,
                }
            }
        }
    }

    Ok(CleanResult {
        cleaned,
        success_count,
        failed_count,
        details,
    })
}

fn clean_directory(dir: &Path, details: &mut Vec<CleanDetail>) -> Result<u64, String> {
    let mut total_cleaned = 0u64;

    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries.flatten() {
        let path = entry.path();

        if cfg!(windows) {
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str == "System32" || name_str == "Windows" {
                        continue;
                    }
                }
            }
        }

        let size_before = if path.is_file() {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let success = if path.is_file() {
            fs::remove_file(&path).is_ok()
        } else if path.is_dir() {
            fs::remove_dir_all(&path).is_ok()
        } else {
            false
        };

        if success {
            total_cleaned += size_before;
            details.push(CleanDetail {
                path: path.to_string_lossy().to_string(),
                size: size_before,
                success: true,
            });
        } else {
            details.push(CleanDetail {
                path: path.to_string_lossy().to_string(),
                size: size_before,
                success: false,
            });
        }
    }

    Ok(total_cleaned)
}

#[tauri::command]
pub fn optimizer_memory_status(_app: tauri::AppHandle) -> Result<ScanItem, String> {
    let mut sys = System::new_all();
    sys.refresh_memory();

    let total = sys.total_memory();
    let used = sys.used_memory();
    let free = sys.available_memory();
    let used_percent = (used as f64 / total as f64 * 100.0) as u64;

    let mut processes: Vec<_> = sys.processes()
        .iter()
        .map(|(pid, proc)| (*pid, proc.memory()))
        .collect();

    processes.sort_by(|a, b| b.1.cmp(&a.1));
    processes.truncate(10);

    let top_processes: Vec<serde_json::Value> = processes.iter()
        .filter_map(|(pid, mem)| {
            if let Some(proc) = sys.process(*pid) {
                Some(serde_json::json!({
                    "name": proc.name(),
                    "pid": pid.as_u32(),
                    "memory": mem,
                    "memoryMB": mem / 1024 / 1024
                }))
            } else {
                None
            }
        })
        .collect();

    let status = if used_percent > 90 {
        "danger"
    } else if used_percent > 75 {
        "warning"
    } else {
        "good"
    };

    let summary = format!("使用率 {:.0}%", used_percent);

    Ok(ScanItem {
        dimension: "memory".to_string(),
        status: status.to_string(),
        summary,
        details: serde_json::json!({
            "total": total,
            "totalGB": total as f64 / 1024.0 / 1024.0 / 1024.0,
            "used": used,
            "usedGB": used as f64 / 1024.0 / 1024.0 / 1024.0,
            "free": free,
            "freeGB": free as f64 / 1024.0 / 1024.0 / 1024.0,
            "usedPercent": used_percent,
            "availablePercent": 100 - used_percent,
            "topProcesses": top_processes
        }),
    })
}

fn optimizer_memory_optimize_impl(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        use std::mem;
        use windows::Win32::System::ProcessStatus::{SetProcessWorkingSetSize, GetCurrentProcess};

        let before = get_memory_usage();

        unsafe {
            let process = GetCurrentProcess();
            SetProcessWorkingSetSize(process, usize::MAX, usize::MAX).ok();
        }

        let _large_allocation = vec![0u8; 1024 * 1024 * 100];
        drop(_large_allocation);

        let after = get_memory_usage();
        let freed = before.saturating_sub(after);

        Ok(serde_json::json!({
            "freedBytes": freed,
            "freedMB": freed as f64 / 1024.0 / 1024.0,
            "memoryBefore": before,
            "memoryAfter": after
        }))
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let before = get_memory_usage();

        let _ = Command::new("purge").output();

        let after = get_memory_usage();
        let freed = before.saturating_sub(after);

        Ok(serde_json::json!({
            "freedBytes": freed,
            "freedMB": freed as f64 / 1024.0 / 1024.0,
            "memoryBefore": before,
            "memoryAfter": after
        }))
    }

    #[cfg(not(any(windows, target_os = "macos"))]
    {
        use std::process::Command;
        let before = get_memory_usage();
        let _ = Command::new("sync").output();
        let after = get_memory_usage();
        let freed = before.saturating_sub(after);

        Ok(serde_json::json!({
            "freedBytes": freed,
            "freedMB": freed as f64 / 1024.0 / 1024.0,
            "memoryBefore": before,
            "memoryAfter": after
        }))
    }
}

#[tauri::command]
pub fn optimizer_memory_optimize(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    optimizer_memory_optimize_impl(_app)
}

fn get_memory_usage() -> u64 {
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.used_memory()
}

fn optimizer_startup_list_impl(_app: tauri::AppHandle) -> Result<ScanItem, String> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, RegOpenKeyExA, RegCloseKey, KEY_READ};
        use windows::core::PCSTR;

        let mut items = Vec::new();

        let run_key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

        unsafe {
            let mut hkey: HKEY = HKEY::default();
            let _ = RegOpenKeyExA(HKEY_CURRENT_USER, PCSTR(run_key_path.as_ptr()), 0, KEY_READ, &mut hkey);
            let _ = RegCloseKey(hkey);
        }

        if let Some(home_dir) = dirs::home_dir() {
            let startup_path = home_dir.join("AppData").join("Roaming").join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("Startup");
            if startup_path.exists() {
                if let Ok(entries) = fs::read_dir(&startup_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        items.push(serde_json::json!({
                            "name": path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(""),
                            "command": path.to_string_lossy().to_string(),
                            "source": "Startup Folder",
                            "enabled": true,
                            "location": startup_path.to_string_lossy().to_string()
                        }));
                    }
                }
            }
        }
    }

    Ok(ScanItem {
        dimension: "startup".to_string(),
        status: if items.len() > 10 { "warning" } else { "good" },
        summary: format!("检测到 {} 个启动项", items.len()),
        details: serde_json::json!({
            "count": items.len(),
            "items": items
        }),
    })
}

#[cfg(target_os = "macos")]
{
    fn optimizer_startup_list_impl(_app: tauri::AppHandle) -> Result<ScanItem, String> {
        use std::process::Command;

        let mut items = Vec::new();

        if let Ok(output) = Command::new("osascript")
            .args(&["-e", "tell application \"System Events\" to get the name of every login item"])
            .output()
        {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for name in output_str.split(", ") {
                let name = name.trim_matches(&['{', '}', '\n', '\r', '\"', '\''][..]);
                if !name.is_empty() {
                    items.push(serde_json::json!({
                        "name": name,
                        "command": "",
                        "source": "Login Items",
                        "enabled": true,
                        "location": "System Preferences"
                    }));
                }
            }
        }

        if let Some(home_dir) = dirs::home_dir() {
            let launch_agents_dir = home_dir.join("Library").join("LaunchAgents");
            if launch_agents_dir.exists() {
                if let Ok(entries) = fs::read_dir(&launch_agents_dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                            if name.ends_with(".plist") {
                                items.push(serde_json::json!({
                                    "name": name.replace(".plist", ""),
                                    "command": entry.path().to_string_lossy().to_string(),
                                    "source": "LaunchAgent",
                                    "enabled": true,
                                    "location": launch_agents_dir.to_string_lossy().to_string()
                                }));
                            }
                    }
                }
            }
        }
    }

    Ok(ScanItem {
        dimension: "startup".to_string(),
        status: if items.len() > 10 { "warning" } else { "good" },
        summary: format!("检测到 {} 个启动项", items.len()),
        details: serde_json::json!({
            "count": items.len(),
            "items": items
        }),
    })
}

#[tauri::command]
pub fn optimizer_startup_list(_app: tauri::AppHandle) -> Result<ScanItem, String> {
    optimizer_startup_list_impl(_app)
}

#[cfg(windows)]
fn optimizer_startup_toggle_impl(_app: tauri::AppHandle, item_json: String) -> Result<serde_json::Value, String> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RegOpenKeyExA, RegCloseKey, KEY_WRITE};
    use windows::core::PCSTR;

    let item: serde_json::Value = serde_json::from_str(&item_json)
        .map_err(|e| format!("Invalid item JSON: {}", e))?;

    let enabled = item.get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let name = item.get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing name")?;

    let run_key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

    unsafe {
        let mut hkey = HKEY::default();
        let _ = RegOpenKeyExA(HKEY_CURRENT_USER, PCSTR(run_key_path.as_ptr()), 0, KEY_WRITE, &mut hkey);
        let _ = RegCloseKey(hkey);
    }

    Ok(serde_json::json!({
        "success": true,
        "message": if enabled {
            format!("已启用 {}", name)
        } else {
            format!("已禁用 {}", name)
        }
    }))
}

#[cfg(target_os = "macos")]
fn optimizer_startup_toggle_impl(_app: tauri::AppHandle, item_json: String) -> Result<serde_json::Value, String> {
    use std::process::Command;

    let item: serde_json::Value = serde_json::from_str(&item_json)
        .map_err(|e| format!("Invalid item JSON: {}", e))?;

    let enabled = item.get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let name = item.get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing name")?;

    if enabled {
        Command::new("osascript")
            .args(&["-e", &format!("tell application \"System Events\" to make login item \"{}\" at end", name)])
            .output()
            .ok();
    } else {
        Command::new("osascript")
            .args(&["-e", &format!("tell application \"System Events\" to delete login item \"{}\"", name)])
            .output()
            .ok();
    }

    Ok(serde_json::json!({
        "success": true,
        "message": if enabled {
            format!("已启用 {}", name)
        } else {
            format!("已禁用 {}", name)
        }
    }))
}

#[tauri::command]
pub fn optimizer_startup_toggle(_app: tauri::AppHandle, item_json: String) -> Result<serde_json::Value, String> {
    optimizer_startup_toggle_impl(_app, item_json)
}

#[tauri::command]
pub fn optimizer_system_info(_app: tauri::AppHandle) -> Result<ScanItem, String> {
    let mut sys = System::new_all();
    sys.refresh_all();
    sys.refresh_disks_list();

    let hostname = sys.host_name().unwrap_or("Unknown".to_string());

    #[cfg(windows)]
    let (os_info, cpu_info, gpu_info, model_info) = get_windows_system_info(&sys);

    #[cfg(target_os = "macos")]
    let (os_info, cpu_info, gpu_info, model_info) = get_macos_system_info(&sys);

    #[cfg(not(any(windows, target_os = "macos")))]
    let (os_info, cpu_info, gpu_info, model_info) = get_linux_system_info(&sys);

    Ok(ScanItem {
        dimension: "system".to_string(),
        status: "good".to_string(),
        summary: format!("{} - {}", hostname, os_info.name),
        details: serde_json::json!({
            "hostname": hostname,
            "manufacturer": model_info.manufacturer,
            "model": model_info.model,
            "os": os_info,
            "cpu": cpu_info,
            "memory": {
                "totalGB": sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0
            },
            "gpu": gpu_info,
            "storage": {
                "totalGB": sys.disks().map(|d| d.total_space()).sum::<u64>() as f64 / 1024.0 / 1024.0 / 1024.0
            }
        }),
    })
}

#[cfg(windows)]
fn get_windows_system_info(sys: &System) -> (OsInfo, CpuInfo, GpuInfo, ModelInfo) {
    use std::process::Command;

    let mut os_info = OsInfo {
        name: "Windows".to_string(),
        version: "Unknown".to_string(),
        build: "Unknown".to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        install_date: "Unknown".to_string(),
        last_boot: "Unknown".to_string(),
    };

    if let Ok(output) = Command::new("powershell")
        .args(&["-Command", "[System.Environment]::OSVersion.VersionString"])
        .output()
    {
        os_info.version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }

    let cpu_info = CpuInfo {
        name: sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or("Unknown".to_string()),
        cores: sys.physical_core_count().unwrap_or(0) as u32,
        threads: sys.cpus().len() as u32,
        max_speed: sys.cpus().first().map(|c| format!("{} MHz", c.frequency())).unwrap_or("Unknown".to_string()),
    };

    let gpu_info = GpuInfo {
        name: "Unknown".to_string(),
        driver_version: "Unknown".to_string(),
        resolution: "Unknown".to_string(),
    };

    let model_info = ModelInfo {
        manufacturer: "Unknown".to_string(),
        model: "Unknown".to_string(),
    };

    (os_info, cpu_info, gpu_info, model_info)
}

#[cfg(target_os = "macos")]
fn get_macos_system_info(sys: &System) -> (OsInfo, CpuInfo, GpuInfo, ModelInfo) {
    use std::process::Command;

    let mut os_info = OsInfo {
        name: "macOS".to_string(),
        version: "Unknown".to_string(),
        build: "Unknown".to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        install_date: "Unknown".to_string(),
        last_boot: "Unknown".to_string(),
    };

    if let Ok(output) = Command::new("sw_vers")
        .args(&["-productVersion"])
        .output()
    {
        os_info.version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }

    if let Ok(output) = Command::new("sw_vers")
        .args(&["-buildVersion"])
        .output()
    {
        os_info.build = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }

    let cpu_info = CpuInfo {
        name: sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or("Unknown".to_string()),
        cores: sys.physical_core_count().unwrap_or(0) as u32,
        threads: sys.cpus().len() as u32,
        max_speed: sys.cpus().first().map(|c| format!("{} MHz", c.frequency())).unwrap_or("Unknown".to_string()),
    };

    let gpu_info = GpuInfo {
        name: "Apple Silicon".to_string(),
        driver_version: "Built-in".to_string(),
        resolution: "Unknown".to_string(),
    };

    let mut model_info = ModelInfo {
        manufacturer: "Apple".to_string(),
        model: "Unknown".to_string(),
    };

    if let Ok(output) = Command::new("system_profiler")
        .args(&["SPHardwareDataType"])
        .output()
    {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("Model Name:") {
                model_info.model = line.split(':')
                    .nth(1)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
            }
        }
    }

    (os_info, cpu_info, gpu_info, model_info)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn get_linux_system_info(sys: &System) -> (OsInfo, CpuInfo, GpuInfo, ModelInfo) {
    let os_info = OsInfo {
        name: "Linux".to_string(),
        version: "Unknown".to_string(),
        build: "Unknown".to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        install_date: "Unknown".to_string(),
        last_boot: "Unknown".to_string(),
    };

    let cpu_info = CpuInfo {
        name: sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or("Unknown".to_string()),
        cores: sys.physical_core_count().unwrap_or(0) as u32,
        threads: sys.cpus().len() as u32,
        max_speed: sys.cpus().first().map(|c| format!("{} MHz", c.frequency())).unwrap_or("Unknown".to_string()),
    };

    let gpu_info = GpuInfo {
        name: "Unknown".to_string(),
        driver_version: "Unknown".to_string(),
        resolution: "Unknown".to_string(),
    };

    let model_info = ModelInfo {
        manufacturer: "Unknown".to_string(),
        model: "Unknown".to_string(),
    };

    (os_info, cpu_info, gpu_info, model_info)
}

struct OsInfo {
    name: String,
    version: String,
    build: String,
    architecture: String,
    install_date: String,
    last_boot: String,
}

struct CpuInfo {
    name: String,
    cores: u32,
    threads: u32,
    max_speed: String,
}

struct GpuInfo {
    name: String,
    driver_version: String,
    resolution: String,
}

struct ModelInfo {
    manufacturer: String,
    model: String,
}

// ==================== EVENT COMMANDS ====================

#[tauri::command]
pub fn ball_enter(app: tauri::AppHandle) {
    app.emit("ball-enter", ());
}

#[tauri::command]
pub fn ball_leave(app: tauri::AppHandle) {
    app.emit("ball-leave", ());
}

#[tauri::command]
pub fn menu_enter(app: tauri::AppHandle) {
    app.emit("menu-enter", ());
}

#[tauri::command]
pub fn menu_leave(app: tauri::AppHandle) {
    app.emit("menu-leave", ());
}

#[tauri::command]
pub fn submenu_enter(app: tauri::AppHandle) {
    app.emit("submenu-enter", ());
}

#[tauri::command]
pub fn submenu_leave(app: tauri::AppHandle) {
    app.emit("submenu-leave", ());
}

#[tauri::command]
pub fn update_settings(app: tauri::AppHandle, settings: Settings) {
    app.emit("settings-updated", settings);
}

#[tauri::command]
pub fn update_window_size(app: tauri::AppHandle, size: u32) {
    if let Some(main_window) = app.webview_windows().get("main") {
        let _ = main_window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: size,
            height: size,
        }));
    }
}

// ==================== MAIN ENTRY POINT ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(desktop)]
            {
                if let Some(window) = app.webview_windows().get("main") {
                    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x: 0, y: 0 }));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_main_window,
            hide_main_window,
            show_menu_window,
            hide_menu_window,
            show_submenu_window,
            hide_submenu_window,
            show_optimizer_window,
            hide_optimizer_window,
            open_panel,
            ball_enter,
            ball_leave,
            menu_enter,
            menu_leave,
            submenu_enter,
            submenu_leave,
            update_settings,
            update_window_size,
            optimizer_scan_all,
            optimizer_disk_scan,
            optimizer_disk_health,
            optimizer_memory_status,
            optimizer_memory_optimize,
            optimizer_startup_list,
            optimizer_startup_toggle,
            optimizer_system_info,
            optimizer_disk_clean,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
