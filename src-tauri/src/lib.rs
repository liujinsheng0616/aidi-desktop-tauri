// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(unexpected_cfgs)]
#![allow(deprecated)]

mod report_worker;

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Position, Size};
use tauri::tray::TrayIconBuilder;
use tauri::menu::{Menu, MenuItem};

// ==================== LOGGING ====================

use std::fs::{OpenOptions, File};
use std::io::Write;
use std::sync::OnceLock;

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

/// 初始化日志文件（尝试多个位置以确保日志能够创建）
fn init_log_file() {
    // 先输出诊断信息到控制台（无论如何都会显示）
    #[cfg(target_os = "windows")]
    {
        eprintln!("=== AIDI 日志诊断 ===");
        eprintln!("桌面目录: {:?}", dirs::desktop_dir());
        eprintln!("本地数据目录: {:?}", dirs::data_local_dir());
        eprintln!("可执行文件路径: {:?}", std::env::current_exe());
        eprintln!("当前用户: {}", whoami::username());
    }

    // 尝试多个日志位置，按优先级排序
    // 优先级：本地数据目录 > 桌面 > 可执行文件同级目录 > 临时目录
    let log_locations: Vec<Option<std::path::PathBuf>> = vec![
        // 优先：本地应用数据目录（比桌面更可靠，Windows 11 + OneDrive 可能导致桌面路径问题）
        dirs::data_local_dir().map(|p| {
            let dir = p.join("AIDI Desktop");
            match std::fs::create_dir_all(&dir) {
                Ok(_) => eprintln!("目录创建成功: {:?}", dir),
                Err(e) => eprintln!("目录创建失败: {:?} - {}", dir, e),
            }
            dir.join("debug.log")
        }),
        // 备选：桌面（可能因 OneDrive 或权限问题失败）
        dirs::desktop_dir().map(|p| p.join("aidi-debug.log")),
        // 备选：可执行文件同级目录
        std::env::current_exe().ok().and_then(|exe| {
            exe.parent().map(|p| p.join("aidi-debug.log"))
        }),
        // 最后备选：临时目录（最可靠的备选位置）
        Some(std::env::temp_dir().join("aidi-debug.log")),
    ];

    for location in log_locations.into_iter().flatten() {
        eprintln!("尝试创建日志文件: {:?}", location);
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&location)
        {
            Ok(file) => {
                eprintln!("日志文件创建成功: {:?}", location);
                let _ = LOG_FILE.set(Mutex::new(file));
                // 写入启动日志
                log_msg(&format!("=== AIDI 启动 {} ===", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
                log_msg(&format!("日志文件位置: {:?}", location));
                // Windows 特定诊断信息
                #[cfg(target_os = "windows")]
                {
                    log_msg(&format!("Windows 桌面目录: {:?}", dirs::desktop_dir()));
                    log_msg(&format!("Windows 本地数据目录: {:?}", dirs::data_local_dir()));
                    log_msg(&format!("可执行文件路径: {:?}", std::env::current_exe()));
                }
                return;
            }
            Err(e) => {
                eprintln!("无法创建日志文件 {:?}: {}", location, e);
            }
        }
    }
    eprintln!("警告: 所有日志位置都失败，日志将仅输出到控制台");
}

/// 写入日志消息
fn log_msg(msg: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_line = format!("[{}] {}\n", timestamp, msg);
    println!("{}", log_line.trim());
    if let Some(log_file) = LOG_FILE.get() {
        if let Ok(mut file) = log_file.lock() {
            let _ = file.write_all(log_line.as_bytes());
        }
    }
}

// ==================== GLOBAL LOGIN STATUS ====================

/// 全局登录状态（用于动态切换托盘菜单）
static IS_LOGGED_IN: AtomicBool = AtomicBool::new(false);

// ==================== DATA STRUCTURES ====================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub ball_size: u32,
    pub opacity: u32,
    pub color_theme: String,
    pub theme_mode: String,
}

// ==================== EXTERNAL URL CONFIGURATION ====================

/// 获取外部项目的基础 URL
/// 优先级：AIDI_EXTERNAL_URL > VITE_APP_DOMAIN > AIDI_ENV > 默认 test 环境
fn get_external_url_base(_app: &AppHandle) -> String {
    // 优先读取环境变量 AIDI_EXTERNAL_URL
    if let Ok(url) = std::env::var("AIDI_EXTERNAL_URL") {
        return url;
    }

    // 尝试读取 VITE_APP_DOMAIN（从 .env 文件或环境变量）
    if let Ok(domain) = std::env::var("VITE_APP_DOMAIN") {
        return format!("{}/aidi-desktop", domain);
    }

    // 通过环境变量 AIDI_ENV 决定使用哪个环境
    let env = std::env::var("AIDI_ENV").unwrap_or_else(|_| "test".to_string());

    match env.as_str() {
        "test" => "https://microsapptest.yadea.com.cn/aidi-desktop",
        "prod" => "https://aidi.yadea.com.cn/aidi-desktop",
        _ => "http://127.0.0.1:5173",
    }.to_string()
}

/// 构建菜单页面的完整 URL
/// 注意：Vue Router 使用 Hash 模式，所以路径需要加 # 前缀
fn build_menu_url(app: &AppHandle, direction: &str) -> String {
    let base_url = get_external_url_base(app);
    format!("{}/#/menu?direction={}", base_url, direction)
}

/// 构建登录页面的完整 URL
fn build_login_url(app: &AppHandle) -> String {
    let base_url = get_external_url_base(app);
    format!("{}/#/login", base_url)
}

// ==================== INTERACTION STATE MACHINE ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Dragging variant reserved for future use
enum InteractionState {
    Idle,           // 空闲
    Hovering,       // 悬浮球 hover
    MenuShowing,    // 菜单显示中
    HideDelaying,   // 等待隐藏
    Dragging,       // 拖拽中
    Animating,      // 动画中
}

impl Default for InteractionState {
    fn default() -> Self {
        InteractionState::Idle
    }
}



// ==================== DOCK STATE ====================

#[derive(Debug, Clone, Default)]
struct DockState {
    is_docked: bool,
    dock_edge: Option<String>, // "left", "right", "top", "bottom"
    is_popped_out: bool,
    hidden_x: i32,
    hidden_y: i32,
    pop_out_x: i32,
    pop_out_y: i32,
    window_width: u32,
    window_height: u32,
    // Interaction state machine
    interaction_state: InteractionState,
    // 弹出保护状态
    is_in_pop_protection: bool,
    // hover 状态
    ball_hover: bool,
    menu_hover: bool,
    // 菜单窗口位置
    menu_window_x: i32,       // 菜单窗口初始 x（逻辑像素），menu_expand 需要用
    menu_window_y: i32,       // 菜单窗口初始 y
    submenu_opens_left: bool, // true = 子菜单向左展开（球在右侧）
}

// Global state version counter for canceling stale operations
static STATE_VERSION: AtomicU64 = AtomicU64::new(0);

fn next_state_version() -> u64 {
    STATE_VERSION.fetch_add(1, Ordering::SeqCst) + 1
}

fn current_state_version() -> u64 {
    STATE_VERSION.load(Ordering::SeqCst)
}

static DOCK_STATE: Mutex<DockState> = Mutex::new(DockState {
    is_docked: false,
    dock_edge: None,
    is_popped_out: false,
    hidden_x: 0,
    hidden_y: 0,
    pop_out_x: 0,
    pop_out_y: 0,
    window_width: 0,
    window_height: 0,
    interaction_state: InteractionState::Idle,
    is_in_pop_protection: false,
    ball_hover: false,
    menu_hover: false,
    menu_window_x: 0,
    menu_window_y: 0,
    submenu_opens_left: false,
});

// 定时器句柄（使用 Arc<Mutex<Option<...>>> 存储跨线程可访问的句柄）
static HIDE_DOCK_TIMER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
static POP_PROTECTION_TIMER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

static BALL_SIZE: Mutex<u32> = Mutex::new(60);
const BALL_PADDING: u32 = 12; // 外环需要 ballSize + 24，窗口尺寸 = ballSize + BALL_PADDING * 2，防止光晕被截断
const EDGE_THRESHOLD: i32 = 15; // Edge detection threshold (reduced for better UX)
const DOCK_VISIBLE_AMOUNT: i32 = 35; // Fixed visible amount when docked (pixels)

// Animation constants
const ANIMATION_FRAMES: u32 = 12;
#[cfg(target_os = "windows")]
const ANIMATION_FRAME_MS: u64 = 33; // ~30fps on Windows
#[cfg(not(target_os = "windows"))]
const ANIMATION_FRAME_MS: u64 = 16; // ~60fps on other platforms

// Platform-specific delays
#[cfg(target_os = "windows")]
const HIDE_DELAY_MS: u64 = 400;
#[cfg(not(target_os = "windows"))]
const HIDE_DELAY_MS: u64 = 300;

#[cfg(target_os = "windows")]
const MENU_HIDE_DELAY_MS: u64 = 100;
#[cfg(not(target_os = "windows"))]
const MENU_HIDE_DELAY_MS: u64 = 80;

// macOS menu bar height
#[cfg(target_os = "macos")]
const MENUBAR_HEIGHT: i32 = 25;
#[cfg(not(target_os = "macos"))]
const MENUBAR_HEIGHT: i32 = 0;

// Ease-out cubic function: 1 - (1-t)^3
fn ease_out_cubic(t: f32) -> f32 {
    let t_inv = 1.0 - t;
    1.0 - t_inv * t_inv * t_inv
}

/// 设置窗口为圆形
fn apply_circular_window_mask(window: &tauri::WebviewWindow, size: u32) {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::{id, nil, YES};
        use objc::{msg_send, sel, sel_impl};

        if let Ok(ns_window) = window.ns_window() {
            let ns_window = ns_window as id;
            unsafe {
                let content_view: id = msg_send![ns_window, contentView];
                let _: () = msg_send![content_view, setWantsLayer: YES];
                let layer: id = msg_send![content_view, layer];
                if layer != nil {
                    let _: () = msg_send![layer, setCornerRadius: (size / 2) as f64];
                    let _: () = msg_send![layer, setMasksToBounds: YES];
                }
            }
        }
    }

    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Gdi::{CreateEllipticRgn, SetWindowRgn};
        use windows::Win32::Foundation::HWND;

        if let Ok(hwnd) = window.hwnd() {
            let hwnd = HWND(hwnd.0);
            unsafe {
                let hrgn = CreateEllipticRgn(0, 0, size as i32, size as i32);
                SetWindowRgn(hwnd, Some(hrgn), true);
            }
        }
    }
}

// Animate window to target position with easing
fn animate_to_position(
    window: &tauri::WebviewWindow,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    expected_version: u64,
) {
    let dx = end_x - start_x;
    let dy = end_y - start_y;

    for frame in 1..=ANIMATION_FRAMES {
        // Check if state version changed (operation cancelled)
        if current_state_version() != expected_version {
            return;
        }

        let t = frame as f32 / ANIMATION_FRAMES as f32;
        let eased = ease_out_cubic(t);

        let x = start_x + (dx as f32 * eased) as i32;
        let y = start_y + (dy as f32 * eased) as i32;

        let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
        std::thread::sleep(Duration::from_millis(ANIMATION_FRAME_MS));
    }

    // Ensure final position is exact
    if current_state_version() == expected_version {
        let _ = window.set_position(Position::Physical(PhysicalPosition { x: end_x, y: end_y }));
    }
}

// ==================== POSITION DETECTION FUNCTIONS ====================

// ==================== WINDOW MANAGEMENT ====================

#[tauri::command]
fn show_main_window(app: tauri::AppHandle, window: tauri::Window) {
    let _ = window.show();
    sync_toggle_menu_item(&app, true);
}

#[tauri::command]
fn hide_main_window(app: tauri::AppHandle, window: tauri::Window) {
    let _ = window.hide();
    sync_toggle_menu_item(&app, false);
}

/// 根据登录状态重建托盘菜单
/// - 未登录：只显示"登录"选项
/// - 已登录：显示"打开AIDI"、"显示/隐藏浮动球"、"退出"
fn rebuild_tray_menu(app: &tauri::AppHandle, is_logged_in: bool, ball_visible: bool) {
    log_msg(&format!("rebuild_tray_menu: is_logged_in={}, ball_visible={}", is_logged_in, ball_visible));
    if let Some(tray) = app.tray_by_id("main-tray") {
        log_msg("rebuild_tray_menu: 找到托盘图标");
        if is_logged_in {
            // 已登录菜单：打开AIDI、显示/隐藏浮动球、退出
            let toggle_label = if ball_visible { "隐藏浮动球" } else { "显示浮动球" };
            if let (Ok(toggle_item), Ok(aigc_item), Ok(quit_item)) = (
                MenuItem::with_id(app, "toggle", toggle_label, true, None::<&str>),
                MenuItem::with_id(app, "aigc", "打开AIDI", true, None::<&str>),
                MenuItem::with_id(app, "quit", "退出", true, None::<&str>),
            ) {
                if let Ok(menu) = Menu::with_items(app, &[&aigc_item, &toggle_item, &quit_item]) {
                    let _ = tray.set_menu(Some(menu));
                    log_msg("rebuild_tray_menu: 已登录菜单设置成功");
                } else {
                    log_msg("rebuild_tray_menu: 已登录菜单创建失败");
                }
            }
        } else {
            // 未登录菜单：登录、退出
            if let (Ok(login_item), Ok(quit_item)) = (
                MenuItem::with_id(app, "login", "登录", true, None::<&str>),
                MenuItem::with_id(app, "quit", "退出", true, None::<&str>),
            ) {
                if let Ok(menu) = Menu::with_items(app, &[&login_item, &quit_item]) {
                    let _ = tray.set_menu(Some(menu));
                    log_msg("rebuild_tray_menu: 未登录菜单设置成功");
                } else {
                    log_msg("rebuild_tray_menu: 未登录菜单创建失败");
                }
            }
        }
    } else {
        log_msg("rebuild_tray_menu: 找不到托盘图标 main-tray");
    }
}

/// 同步 Tray 菜单"显示/隐藏浮动球"文字（重建菜单并 set_menu）
/// 兼容旧调用，内部调用 rebuild_tray_menu
fn sync_toggle_menu_item(app: &tauri::AppHandle, visible: bool) {
    let is_logged_in = IS_LOGGED_IN.load(Ordering::SeqCst);
    rebuild_tray_menu(app, is_logged_in, visible);
}

#[tauri::command]
fn show_menu_window(app: tauri::AppHandle) {
    if let Some(menu_window) = app.webview_windows().get("menu") {
        let _ = menu_window.show();
    }
}

#[tauri::command]
fn hide_menu_window(app: tauri::AppHandle) {
    if let Some(menu_window) = app.webview_windows().get("menu") {
        let _ = menu_window.hide();
    }
}

#[tauri::command]
fn show_optimizer_window(app: tauri::AppHandle) {
    // 先隐藏菜单（与 Electron 版本一致）
    let windows = app.webview_windows();
    if let Some(menu_window) = windows.get("menu") {
        let _ = menu_window.emit("menu-hidden", ());
        let _ = menu_window.hide();
    }

    // 重置 hover 状态
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.menu_hover = false;
        state.ball_hover = false;
        state.interaction_state = InteractionState::Idle;
    }

    // 显示 optimizer 窗口
    if let Some(optimizer_window) = windows.get("optimizer") {
        let _ = optimizer_window.show();
        let _ = optimizer_window.set_focus();
        let _ = optimizer_window.emit("optimizer-shown", ());
    }
}

#[tauri::command]
fn hide_optimizer_window(app: tauri::AppHandle) {
    if let Some(optimizer_window) = app.webview_windows().get("optimizer") {
        let _ = optimizer_window.hide();
    }
}

#[tauri::command]
fn open_panel(app: tauri::AppHandle) {
    if let Some(panel_window) = app.webview_windows().get("panel") {
        let _ = panel_window.show();
    }
}

// ==================== BALL INTERACTION ====================

/// 拖拽开始前准备：只更新状态，不移动窗口
/// 使用自定义拖拽逻辑时，不需要移动窗口到 pop_out 位置
#[tauri::command]
fn prepare_drag(_app: tauri::AppHandle) {
    // 取消所有定时器
    next_state_version();
    {
        let mut timer = HIDE_DOCK_TIMER.lock().unwrap();
        let _ = timer.take();
    }
    {
        let mut timer = POP_PROTECTION_TIMER.lock().unwrap();
        let _ = timer.take();
    }
    {
        let mut timer = MENU_HIDE_TIMER.lock().unwrap();
        let _ = timer.take();
    }

    // 更新状态，但不重置 is_docked（让 drag_end 处理）
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.ball_hover = true;
        state.interaction_state = InteractionState::Dragging;
        // 不要设置 is_docked = false，让 drag_end 来决定
        // 只重置 popped_out 状态，因为拖拽开始时球已经弹出了
        state.is_popped_out = false;
    }
}

#[tauri::command]
fn ball_enter(app: tauri::AppHandle) {
    let _ = app.emit("ball-enter", ());

    // Cancel any pending dock hide and update state
    let version = next_state_version();
    {
        let mut timer = HIDE_DOCK_TIMER.lock().unwrap();
        let _ = timer.take();
    }

    // Update ball hover state and check if we need to pop out
    let (should_pop, hidden_x, hidden_y, pop_out_x, pop_out_y) = {
        let mut state = DOCK_STATE.lock().unwrap();
        state.ball_hover = true;
        state.interaction_state = InteractionState::Hovering;

        if state.is_docked && !state.is_popped_out {
            state.is_popped_out = true;
            state.is_in_pop_protection = true;
            (
                true,
                state.hidden_x,
                state.hidden_y,
                state.pop_out_x,
                state.pop_out_y,
            )
        } else {
            (false, 0, 0, 0, 0)
        }
    };

    if should_pop {
        // Animate pop out
        let app_handle = app.clone();
        std::thread::spawn(move || {
            if let Some(main_window) = app_handle.webview_windows().get("main") {
                animate_to_position(
                    &main_window,
                    hidden_x,
                    hidden_y,
                    pop_out_x,
                    pop_out_y,
                    version,
                );
            }
        });

        // Start pop protection period: 600ms after popping out, don't respond to hide requests
        let app_handle = app.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));

            // End protection period and check if we should hide
            let (should_hide, hidden_x, hidden_y, pop_x, pop_y) = {
                let mut state = DOCK_STATE.lock().unwrap();
                state.is_in_pop_protection = false;

                if state.is_docked && !state.ball_hover && !state.menu_hover
                {
                    (
                        true,
                        state.hidden_x,
                        state.hidden_y,
                        state.pop_out_x,
                        state.pop_out_y,
                    )
                } else {
                    (false, 0, 0, 0, 0)
                }
            };

            if should_hide {
                let hide_version = next_state_version();
                if let Some(main_window) = app_handle.webview_windows().get("main") {
                    animate_to_position(
                        &main_window,
                        pop_x,
                        pop_y,
                        hidden_x,
                        hidden_y,
                        hide_version,
                    );
                }

                let mut state = DOCK_STATE.lock().unwrap();
                if state.is_docked {
                    state.is_popped_out = false;
                    state.interaction_state = InteractionState::Idle;
                }
            }
        });

        let mut timer = POP_PROTECTION_TIMER.lock().unwrap();
        *timer = Some(handle);
    }
}

#[tauri::command]
fn ball_leave(app: tauri::AppHandle) {
    let _ = app.emit("ball-leave", ());

    // Update ball hover state and get dock info
    let (is_docked, is_popped_out) = {
        let mut state = DOCK_STATE.lock().unwrap();
        state.ball_hover = false;
        if state.interaction_state == InteractionState::Hovering {
            state.interaction_state = InteractionState::HideDelaying;
        }
        (state.is_docked, state.is_popped_out)
    };

    // 启动菜单隐藏检查
    let app_handle = app.clone();
    let menu_hide_handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(MENU_HIDE_DELAY_MS));

        let should_hide_menu = {
            let state = DOCK_STATE.lock().unwrap();
            // 如果鼠标不在球或菜单上，则隐藏菜单
            !state.ball_hover && !state.menu_hover
        };

        if should_hide_menu {
            // 隐藏菜单窗口
            let windows = app_handle.webview_windows();
            if let Some(menu_window) = windows.get("menu") {
                let _ = menu_window.emit("menu-hidden", ());
                let _ = menu_window.hide();
            }
        }
    });

    // 将菜单隐藏任务也加入到定时器管理中
    {
        let mut timer = MENU_HIDE_TIMER.lock().unwrap();
        *timer = Some(menu_hide_handle);
    }

    // If docked and popped out, delay hiding
    if is_docked && is_popped_out {
        // Cancel any existing timer
        {
            let mut timer = HIDE_DOCK_TIMER.lock().unwrap();
            let _ = timer.take();
        }

        let app_handle = app.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(HIDE_DELAY_MS));

            // Check again if we should hide
            let (should_hide, hidden_x, hidden_y, pop_x, pop_y) = {
                let state = DOCK_STATE.lock().unwrap();
                // Don't hide if in pop protection period or any hover state
                if state.is_in_pop_protection
                    || state.ball_hover
                    || state.menu_hover
                {
                    (false, 0, 0, 0, 0)
                } else if state.is_docked {
                    (
                        true,
                        state.hidden_x,
                        state.hidden_y,
                        state.pop_out_x,
                        state.pop_out_y,
                    )
                } else {
                    (false, 0, 0, 0, 0)
                }
            };

            if should_hide {
                let version = next_state_version();
                if let Some(main_window) = app_handle.webview_windows().get("main") {
                    animate_to_position(&main_window, pop_x, pop_y, hidden_x, hidden_y, version);
                }

                let mut state = DOCK_STATE.lock().unwrap();
                if state.is_docked {
                    state.is_popped_out = false;
                    state.interaction_state = InteractionState::Idle;
                }
            }
        });

        let mut timer = HIDE_DOCK_TIMER.lock().unwrap();
        *timer = Some(handle);
    }
}

// Timer for delayed menu hide
static MENU_HIDE_TIMER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

#[tauri::command]
fn menu_enter(app: tauri::AppHandle) {
    let _ = app.emit("menu-enter", ());

    // Cancel any pending hide operations
    let _ = next_state_version();
    {
        let mut timer = HIDE_DOCK_TIMER.lock().unwrap();
        let _ = timer.take();
    }
    {
        let mut timer = MENU_HIDE_TIMER.lock().unwrap();
        let _ = timer.take();
    }

    // Update hover state
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.menu_hover = true;
        state.interaction_state = InteractionState::MenuShowing;
    }
}

#[tauri::command]
fn menu_leave(app: tauri::AppHandle) {
    let _ = app.emit("menu-leave", ());

    // 立即更新 menu_hover 状态
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.menu_hover = false;
    }

    // 延迟检查是否需要隐藏菜单
    let app_handle = app.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(MENU_HIDE_DELAY_MS));

        let should_hide = {
            let state = DOCK_STATE.lock().unwrap();
            // 只有当没有任何 hover 状态时才隐藏
            !state.menu_hover && !state.ball_hover
        };

        if should_hide {
            {
                let mut state = DOCK_STATE.lock().unwrap();
                if state.interaction_state == InteractionState::MenuShowing {
                    state.interaction_state = InteractionState::HideDelaying;
                }
            }
            // 隐藏菜单窗口
            let windows = app_handle.webview_windows();
            if let Some(menu_window) = windows.get("menu") {
                let _ = menu_window.emit("menu-hidden", ());
                let _ = menu_window.hide();
            }
        }
    });

    let mut timer = MENU_HIDE_TIMER.lock().unwrap();
    *timer = Some(handle);
}

#[tauri::command]
fn scroll_ball(_app: tauri::AppHandle, _delta_y: i32) {
    // TODO: Implement scroll functionality
}

// Store window position for drag - using atomic for lock-free access
static DRAG_WINDOW_X: AtomicI32 = AtomicI32::new(0);
static DRAG_WINDOW_Y: AtomicI32 = AtomicI32::new(0);

#[tauri::command]
fn start_drag(window: tauri::Window) {
    if let Ok(pos) = window.outer_position() {
        DRAG_WINDOW_X.store(pos.x, Ordering::Relaxed);
        DRAG_WINDOW_Y.store(pos.y, Ordering::Relaxed);
    }
}

#[tauri::command]
fn move_window_by(window: tauri::Window, dx: i32, dy: i32) {
    // Use atomic fetch_add for lock-free position update
    let new_x = DRAG_WINDOW_X.fetch_add(dx, Ordering::Relaxed) + dx;
    let new_y = DRAG_WINDOW_Y.fetch_add(dy, Ordering::Relaxed) + dy;

    // Set position directly
    let _ = window.set_position(Position::Physical(PhysicalPosition { x: new_x, y: new_y }));
}

#[tauri::command]
fn drag_end(app: tauri::AppHandle) {
    let windows = app.webview_windows();
    let Some(main_window) = windows.get("main") else {
        return;
    };
    let Ok(pos) = main_window.outer_position() else {
        return;
    };
    let Ok(size) = main_window.outer_size() else {
        return;
    };

    // Get screen info
    let Some(monitor) = main_window.current_monitor().ok().flatten() else {
        return;
    };
    let screen_size = monitor.size();
    let screen_width = screen_size.width as i32;
    let screen_height = screen_size.height as i32;

    let window_width = size.width as i32;
    let window_height = size.height as i32;

    // Calculate actual ball center position (considering BALL_PADDING)
    let _ball_center_x = pos.x + window_width / 2;

    // Edge detection with priority: left/right first, then top/bottom
    let at_left = pos.x < EDGE_THRESHOLD;
    let at_right = pos.x + window_width > screen_width - EDGE_THRESHOLD;
    let at_top = pos.y < EDGE_THRESHOLD + MENUBAR_HEIGHT;
    let at_bottom = pos.y + window_height > screen_height - EDGE_THRESHOLD;

    // Determine edge with priority: horizontal edges first
    let edge = if at_left {
        Some("left")
    } else if at_right {
        Some("right")
    } else if at_top {
        Some("top")
    } else if at_bottom {
        Some("bottom")
    } else {
        None
    };

    let mut state = DOCK_STATE.lock().unwrap();

    if let Some(edge) = edge {
        let pop_offset = 5;

        // Use fixed visible amount for consistent UX
        let visible_amount = DOCK_VISIBLE_AMOUNT;
        let hide_amount = window_width / 2 - visible_amount / 2;

        state.is_docked = true;
        state.dock_edge = Some(edge.to_string());
        state.is_popped_out = false;
        state.interaction_state = InteractionState::Idle;
        state.window_width = size.width;
        state.window_height = size.height;

        // Clamp Y position within screen bounds (considering menubar on macOS)
        let clamped_y = pos.y.max(MENUBAR_HEIGHT).min(screen_height - window_height);
        // Clamp X position within screen bounds
        let clamped_x = pos.x.max(0).min(screen_width - window_width);

        match edge {
            "left" => {
                // Hide to left, show DOCK_VISIBLE_RATIO of ball
                state.hidden_x = -hide_amount;
                state.hidden_y = clamped_y;
                state.pop_out_x = pop_offset;
                state.pop_out_y = clamped_y;
            }
            "right" => {
                // Hide to right, show DOCK_VISIBLE_RATIO of ball
                state.hidden_x = screen_width - window_width + hide_amount;
                state.hidden_y = clamped_y;
                state.pop_out_x = screen_width - window_width - pop_offset;
                state.pop_out_y = clamped_y;
            }
            "top" => {
                // Hide to top, show DOCK_VISIBLE_RATIO of ball
                let top_hide_amount = window_height / 2 - visible_amount / 2;
                state.hidden_x = clamped_x;
                state.hidden_y = MENUBAR_HEIGHT - top_hide_amount;
                state.pop_out_x = clamped_x;
                state.pop_out_y = MENUBAR_HEIGHT + pop_offset;
            }
            "bottom" => {
                // Hide to bottom, show DOCK_VISIBLE_RATIO of ball
                let bottom_hide_amount = window_height / 2 - visible_amount / 2;
                state.hidden_x = clamped_x;
                state.hidden_y = screen_height - window_height + bottom_hide_amount;
                state.pop_out_x = clamped_x;
                state.pop_out_y = screen_height - window_height - pop_offset;
            }
            _ => {}
        }

        // Get target position before animation
        let hidden_x = state.hidden_x;
        let hidden_y = state.hidden_y;
        let version = next_state_version();
        state.interaction_state = InteractionState::Animating;
        drop(state);

        // Animate to hidden position
        let main_window_clone = main_window.clone();
        std::thread::spawn(move || {
            animate_to_position(
                &main_window_clone,
                pos.x,
                pos.y,
                hidden_x,
                hidden_y,
                version,
            );

            // Update state after animation
            let mut state = DOCK_STATE.lock().unwrap();
            if state.interaction_state == InteractionState::Animating {
                state.interaction_state = InteractionState::Idle;
            }
        });
    } else {
        // Undock - clear all protection state
        state.is_docked = false;
        state.dock_edge = None;
        state.is_popped_out = false;
        state.is_in_pop_protection = false;
        state.interaction_state = InteractionState::Idle;

        // Cancel pop protection timer
        drop(state);
        next_state_version(); // Cancel any pending animations
        let mut timer = POP_PROTECTION_TIMER.lock().unwrap();
        if let Some(_handle) = timer.take() {
            // Let existing thread finish
        }
    }
}

#[tauri::command]
fn hide_docked_ball(app: tauri::AppHandle) {
    // Check if we should hide
    let (should_hide, hidden_x, hidden_y, pop_x, pop_y) = {
        let state = DOCK_STATE.lock().unwrap();
        if !state.is_docked || !state.is_popped_out {
            return;
        }
        // Don't hide if any hover state is active
        if state.ball_hover || state.menu_hover || state.is_in_pop_protection
        {
            return;
        }
        (
            true,
            state.hidden_x,
            state.hidden_y,
            state.pop_out_x,
            state.pop_out_y,
        )
    };

    if should_hide {
        let version = next_state_version();
        if let Some(main_window) = app.webview_windows().get("main") {
            // Use animation in a separate thread to avoid blocking
            let main_window_clone = main_window.clone();
            std::thread::spawn(move || {
                animate_to_position(
                    &main_window_clone,
                    pop_x,
                    pop_y,
                    hidden_x,
                    hidden_y,
                    version,
                );

                let mut state = DOCK_STATE.lock().unwrap();
                if state.is_docked {
                    state.is_popped_out = false;
                    state.interaction_state = InteractionState::Idle;
                }
            });
        }
    }
}

#[tauri::command]
fn set_window_position(app: AppHandle, x: i32, y: i32) {
    // 直接更新球窗口位置，不做额外的菜单同步（拖拽时菜单已隐藏）
    if let Some(window) = app.webview_windows().get("main") {
        let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
    }
}

#[tauri::command]
fn get_window_position(window: tauri::Window) -> (i32, i32) {
    if let Ok(pos) = window.outer_position() {
        (pos.x, pos.y)
    } else {
        (0, 0)
    }
}

fn create_menu_window(app: &tauri::AppHandle, direction: &str) -> Result<tauri::WebviewWindow, tauri::Error> {
    let app_handle = app.clone();
    let menu_url_str = build_menu_url(app, direction);
    let menu_url = tauri::WebviewUrl::External(
        tauri::Url::parse(&menu_url_str).unwrap()
    );
    tauri::WebviewWindowBuilder::new(app, "menu", menu_url)
        .title("Menu")
        .inner_size(192.0, 124.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .incognito(true)
        .on_navigation(move |url| {
            // 通用命令桥：解析 hash 中的 invoke=<命令名>[&param=val...]，执行白名单内的命令
            if let Some(fragment) = url.fragment() {
                if let Some(rest) = fragment.strip_prefix("invoke=") {
                    // 解析命令名（第一个 & 之前）和参数
                    let (cmd, params_str) = match rest.find('&') {
                        Some(idx) => (&rest[..idx], &rest[idx+1..]),
                        None => (rest, ""),
                    };
                    const ALLOWED: &[&str] = &[
                        "show_optimizer_window",
                        "hide_optimizer_window",
                        "hide_menu",
                        "show_main_window",
                        "hide_main_window",
                        "show_login_window",
                        "hide_login_window",
                        "show_menu_window",
                        "menu_expand",
                        "menu_collapse",
                        "update_settings",
                    ];
                    if ALLOWED.contains(&cmd) {
                        let app2 = app_handle.clone();
                        let cmd_owned = cmd.to_string();
                        let params_owned = params_str.to_string();
                        std::thread::spawn(move || {
                            match cmd_owned.as_str() {
                                "show_optimizer_window" => show_optimizer_window(app2),
                                "hide_optimizer_window" => hide_optimizer_window(app2),
                                "hide_menu" => hide_menu(app2),
                                "show_main_window" => {
                                    if let Some(w) = app2.webview_windows().get("main") {
                                        let _ = w.show();
                                        sync_toggle_menu_item(&app2, true);
                                    }
                                }
                                "hide_main_window" => {
                                    if let Some(w) = app2.webview_windows().get("main") {
                                        let _ = w.hide();
                                        sync_toggle_menu_item(&app2, false);
                                    }
                                }
                                "show_login_window" => show_login_window(app2),
                                "hide_login_window" => {
                                    if let Some(w) = app2.webview_windows().get("login") {
                                        let _ = w.hide();
                                    }
                                }
                                "show_menu_window" => {
                                    if let Some(w) = app2.webview_windows().get("menu") {
                                        let _ = w.show();
                                    }
                                }
                                "menu_expand" => {
                                    if let Some(w) = app2.webview_windows().get("menu") {
                                        let (init_x, init_y, opens_left) = {
                                            let s = DOCK_STATE.lock().unwrap();
                                            (s.menu_window_x, s.menu_window_y, s.submenu_opens_left)
                                        };
                                        if opens_left {
                                            // 向左展开：窗口 x 左移244，宽度扩至428
                                            let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition {
                                                x: (init_x - 244) as f64,
                                                y: init_y as f64,
                                            }));
                                        }
                                        let _ = w.set_size(tauri::Size::Logical(tauri::LogicalSize {
                                            width: 428.0,
                                            height: 360.0,
                                        }));
                                    }
                                }
                                "menu_collapse" => {
                                    if let Some(w) = app2.webview_windows().get("menu") {
                                        let (init_x, init_y, opens_left) = {
                                            let s = DOCK_STATE.lock().unwrap();
                                            (s.menu_window_x, s.menu_window_y, s.submenu_opens_left)
                                        };
                                        if opens_left {
                                            // 收起：恢复初始 x，宽度缩回184
                                            let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition {
                                                x: init_x as f64,
                                                y: init_y as f64,
                                            }));
                                        }
                                        let _ = w.set_size(tauri::Size::Logical(tauri::LogicalSize {
                                            width: 192.0,
                                            height: 124.0,
                                        }));
                                    }
                                }
                                "update_settings" => {
                                    // 解析 query 参数：ball_size=N&opacity=N&color_theme=X&theme_mode=X
                                    let mut ball_size: u32 = 60;
                                    let mut opacity: u32 = 100;
                                    let mut color_theme = String::from("cyan-purple");
                                    let mut theme_mode = String::from("system");
                                    for pair in params_owned.split('&') {
                                        if let Some((k, v)) = pair.split_once('=') {
                                            match k {
                                                "ball_size" => { ball_size = v.parse().unwrap_or(60); }
                                                "opacity" => { opacity = v.parse().unwrap_or(100); }
                                                "color_theme" => { color_theme = v.to_string(); }
                                                "theme_mode" => { theme_mode = v.to_string(); }
                                                _ => {}
                                            }
                                        }
                                    }
                                    let settings = Settings { ball_size, opacity, color_theme, theme_mode };
                                    update_settings(app2, settings);
                                }
                                _ => {}
                            }
                        });
                    }
                }
            }
            true
        })
        .build()
}

// 登录窗口创建状态标志
static LOGIN_WINDOW_CREATING: AtomicBool = AtomicBool::new(false);

/// 创建登录窗口（动态创建，加载远程登录页）
fn create_login_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, tauri::Error> {
    // 检查是否正在创建中
    if LOGIN_WINDOW_CREATING.load(Ordering::SeqCst) {
        log_msg("[create_login_window] 窗口正在创建中，跳过...");
        return Err(tauri::Error::WindowNotFound);
    }

    LOGIN_WINDOW_CREATING.store(true, Ordering::SeqCst);
    log_msg("[create_login_window] 开始创建登录窗口...");
    let app_handle = app.clone();
    let login_url_str = build_login_url(app);
    log_msg(&format!("[create_login_window] 登录 URL: {}", login_url_str));

    // 先用 about:blank 创建窗口，避免 build() 郻塞
    let blank_url = tauri::WebviewUrl::External(tauri::Url::parse("about:blank").unwrap());

    log_msg("[create_login_window] 使用 about:blank 构建窗口...");
    let build_result = tauri::WebviewWindowBuilder::new(app, "login", blank_url)
        .title("AIDI 登录")
        .inner_size(360.0, 420.0)
        .decorations(true)
        .transparent(false)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .center()
        .visible(true)
        .on_navigation(move |url| {
            // 监听登录成功：解析 hash 中的 invoke=login-success&token=xxx&user=yyy
            if let Some(fragment) = url.fragment() {
                if let Some(rest) = fragment.strip_prefix("invoke=login-success") {
                    log_msg(&format!("[login] 捕获到登录成功: {}", rest));

                    // 解析参数
                    let mut token = String::new();
                    let mut user_json = String::new();

                    // rest 格式: &token=xxx&user=yyy
                    for pair in rest.trim_start_matches('&').split('&') {
                        if let Some((k, v)) = pair.split_once('=') {
                            match k {
                                "token" => {
                                    if let Ok(decoded) = urlencoding_decode(v) {
                                        token = decoded;
                                    }
                                }
                                "user" => {
                                    if let Ok(decoded) = urlencoding_decode(v) {
                                        user_json = decoded;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    if !token.is_empty() {
                        let app2 = app_handle.clone();
                        let token_owned = token.clone();
                        let user_json_owned = user_json.clone();

                        // 解析用户信息获取 userId 和 userName
                        let (user_id, user_name) = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&user_json) {
                            let id = json["id"].as_str().unwrap_or("").to_string();
                            let name = json["name"].as_str().unwrap_or("").to_string();
                            (id, name)
                        } else {
                            (String::new(), String::new())
                        };

                        // 保存登录信息到 auth.json
                        std::thread::spawn(move || {
                            log_msg(&format!("[login] 保存登录信息: userId={}, userName={}", user_id, user_name));

                            if let Some(data_dir) = dirs::data_local_dir() {
                                let aidi_dir = data_dir.join("AIDI Desktop");
                                let _ = std::fs::create_dir_all(&aidi_dir);

                                let auth_file = aidi_dir.join("auth.json");
                                let content = serde_json::json!({
                                    "token": token_owned,
                                    "userId": user_id,
                                    "userName": user_name,
                                    "user": user_json_owned,
                                    "updatedAt": chrono::Local::now().to_rfc3339(),
                                });

                                if let Err(e) = std::fs::write(&auth_file, content.to_string()) {
                                    log_msg(&format!("[login] 保存 auth.json 失败: {}", e));
                                } else {
                                    log_msg(&format!("[login] 登录信息已保存到: {:?}", auth_file));
                                }
                            }

                            // 执行登录成功逻辑（显示主窗口、更新托盘等）
                            handle_login_success(&app2);
                        });
                    }
                }
            }
            true
        })
        .build();
    log_msg("[create_login_window] 窗口 build() 调用完成");

    let login_window = match build_result {
        Ok(w) => {
            log_msg("[create_login_window] 窗口创建成功");
            w
        },
        Err(e) => {
            log_msg(&format!("[create_login_window] 窗口创建失败: {:?}", e));
            return Err(e);
        },
    };

    // 窗口创建成功后，显示窗口并导航到远程登录页
    let _ = login_window.center();
    let _ = login_window.show();
    let _ = login_window.set_focus();
    log_msg("[create_login_window] 窗口已显示，准备导航到远程登录页...");

    // 使用 navigate() 跳转到远程登录页
    let login_url = tauri::Url::parse(&login_url_str).unwrap();
    match login_window.navigate(login_url) {
        Ok(_) => log_msg(&format!("[create_login_window] 导航成功: {}", login_url_str)),
        Err(e) => log_msg(&format!("[create_login_window] 导航失败: {:?}", e)),
    }

    // 设置窗口关闭拦截：隐藏而不是销毁
    let login_window_clone = login_window.clone();
    let _ = login_window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            log_msg("login 窗口关闭请求被拦截，隐藏窗口");
            let _ = login_window_clone.hide();
            api.prevent_close();
        }
    });

    // 添加加载超时检测，防止网络问题导致窗口卡死
    let window_clone = login_window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        // 如果窗口还在加载中，记录警告
        log_msg("[Warning] 登录窗口加载可能超时（10秒），网络可能存在问题");
        // 尝试获取窗口状态
        if let Ok(is_visible) = window_clone.is_visible() {
            log_msg(&format!("[Warning] 登录窗口当前可见性: {}", is_visible));
        }
    });

    log_msg("[create_login_window] 窗口设置完成，返回窗口对象");
    Ok(login_window)
}

/// URL 解码（简单实现）
fn urlencoding_decode(s: &str) -> Result<String, ()> {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                return Err(());
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

/// 登录成功后的处理逻辑（从 on_login_success 抽取出来供 on_navigation 调用）
fn handle_login_success(app: &tauri::AppHandle) {
    log_msg("handle_login_success: 登录成功");

    // 更新登录状态
    IS_LOGGED_IN.store(true, Ordering::SeqCst);

    // 关闭登录窗口
    if let Some(w) = app.webview_windows().get("login") {
        let _ = w.hide();
        log_msg("登录窗口已隐藏");
    }

    // 更新托盘菜单为已登录状态
    rebuild_tray_menu(app, true, false);

    // 获取 main 窗口并显示
    if let Some(main_window) = app.webview_windows().get("main") {
        // 先显示窗口（确保 WebView 已初始化）
        let _ = main_window.show();
        log_msg("主窗口已显示");

        // 从 auth.json 读取登录信息并写入主窗口的 localStorage
        if let Some(data_dir) = dirs::data_local_dir() {
            let auth_file = data_dir.join("AIDI Desktop").join("auth.json");
            if let Ok(content) = std::fs::read_to_string(&auth_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let token = json["token"].as_str().unwrap_or("");
                    let user_json = json["user"].as_str().unwrap_or("{}");

                    let js_code = format!(
                        r#"(function() {{
                            var token = {};
                            var user = {};
                            try {{
                                localStorage.setItem('aidi-token', token);
                                localStorage.setItem('aidi-user', user);
                            }} catch(e) {{}}
                        }})();"#,
                        serde_json::to_string(&serde_json::json!(token)).unwrap_or_else(|_| "\"\"".to_string()),
                        user_json
                    );

                    let _ = main_window.eval(&js_code);
                    log_msg("已将登录信息写入 localStorage");
                }
            }
        }

        // 调用前端的初始化函数
        let eval_result = main_window.eval("window.__aidiHandleLoginComplete && window.__aidiHandleLoginComplete()");
        log_msg(&format!("调用前端初始化函数: {:?}", eval_result));

        // 更新托盘菜单（浮动球现在可见了）
        let visible = main_window.is_visible().unwrap_or(false);
        rebuild_tray_menu(app, true, visible);
    }
}

#[tauri::command]
fn show_menu(app: tauri::AppHandle) {
    // 1. 先更新状态，保护球不被隐藏
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.menu_hover = true;
        state.interaction_state = InteractionState::MenuShowing;
    }

    // 2. 取消可能导致隐藏的定时器
    {
        let mut timer = HIDE_DOCK_TIMER.lock().unwrap();
        let _ = timer.take();
    }
    {
        let mut timer = POP_PROTECTION_TIMER.lock().unwrap();
        let _ = timer.take();
    }
    {
        let mut timer = MENU_HIDE_TIMER.lock().unwrap();
        let _ = timer.take();
    }

    let windows = app.webview_windows();
    let Some(main_window) = windows.get("main") else {
        return;
    };

    let Some(monitor) = main_window.current_monitor().ok().flatten() else {
        return;
    };

    let scale_factor = monitor.scale_factor();
    let screen_size = monitor.size();
    let screen_width = (screen_size.width as f64 / scale_factor) as i32;
    let screen_height = (screen_size.height as f64 / scale_factor) as i32;

    // 菜单尺寸常量
    let menu_width: i32 = 192;
    let submenu_width: i32 = 244;
    let menu_height: i32 = 124;
    let menu_gap: i32 = 4;

    // 获取球的逻辑位置
    let Ok(ball_pos) = main_window.outer_position() else {
        return;
    };
    let ball_size = *BALL_SIZE.lock().unwrap();
    let visual_ball_size = (ball_size + BALL_PADDING * 2) as i32;
    let ball_x = (ball_pos.x as f64 / scale_factor) as i32;
    let ball_y = (ball_pos.y as f64 / scale_factor) as i32;

    // 计算水平方向：根据右侧空间决定菜单对齐方式和子菜单展开方向
    let ball_right_edge = ball_x + visual_ball_size;
    let space_right = screen_width - ball_right_edge;
    let opens_left = space_right < submenu_width;

    let (menu_x, submenu_direction) = if opens_left {
        // 右侧空间不足（球在右侧），子菜单向左展开，主菜单右对齐球体
        (ball_x + visual_ball_size - menu_width, "left")
    } else {
        // 右侧空间充足（球在左侧），子菜单向右展开，主菜单左对齐球体
        (ball_x, "right")
    };

    // 垂直方向：菜单在球下方（如果空间不够则上方）
    let space_below = screen_height - (ball_y + visual_ball_size);
    let show_above = space_below < menu_height + menu_gap;
    let menu_y = if show_above {
        ball_y - menu_height - menu_gap
    } else {
        ball_y + visual_ball_size + menu_gap
    };

    eprintln!("show_menu: screen=({}, {}), ball=({}, {}, size={}), space_right={}, opens_left={}, menu=({}, {})",
        screen_width, screen_height, ball_x, ball_y, visual_ball_size, space_right, opens_left, menu_x, menu_y);

    // 存入 DOCK_STATE
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.menu_window_x = menu_x;
        state.menu_window_y = menu_y;
        state.submenu_opens_left = opens_left;
    }

    // 复用或创建菜单窗口
    let menu_window = {
        let new_url = tauri::Url::parse(&build_menu_url(&app, submenu_direction)).unwrap();
        let windows = app.webview_windows();
        if let Some(existing) = windows.get("menu") {
            let _ = existing.hide();
            let _ = existing.set_size(Size::Logical(tauri::LogicalSize {
                width: menu_width as f64,
                height: menu_height as f64,
            }));
            let _ = existing.set_position(Position::Logical(LogicalPosition {
                x: menu_x as f64,
                y: menu_y as f64,
            }));
            eprintln!("show_menu: 复用窗口, 设置尺寸={}x{}, 位置=({}, {}), direction={}",
                menu_width, menu_height, menu_x, menu_y, submenu_direction);
            let _ = existing.navigate(new_url);
            existing.clone()
        } else {
            match create_menu_window(&app, submenu_direction) {
                Ok(w) => {
                    let _ = w.set_position(Position::Logical(LogicalPosition {
                        x: menu_x as f64,
                        y: menu_y as f64,
                    }));
                    w
                },
                Err(e) => {
                    eprintln!("show_menu: 创建菜单窗口失败: {}", e);
                    return;
                }
            }
        }
    };

    // 确认窗口尺寸后再显示
    if let Ok(size) = menu_window.outer_size() {
        eprintln!("show_menu: 显示窗口前, 窗口尺寸={}x{}", size.width, size.height);
    }

    // 显示窗口
    let _ = menu_window.show();
}

#[tauri::command]
fn menu_ready(app: tauri::AppHandle) {
    // Vue 组件准备好后，显示菜单窗口
    if let Some(menu_window) = app.webview_windows().get("menu") {
        let _ = menu_window.show();
        eprintln!("menu_ready: 菜单窗口已显示");
    }
}

#[tauri::command]
fn hide_menu(app: tauri::AppHandle) {
    // 重置所有 hover 状态
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.menu_hover = false;
        state.ball_hover = false;
        state.interaction_state = InteractionState::Idle;
    }

    // 隐藏菜单窗口
    let windows = app.webview_windows();
    if let Some(menu_window) = windows.get("menu") {
        // 发送菜单隐藏事件，让前端重置子菜单状态
        let _ = menu_window.emit("menu-hidden", ());
        // 重置窗口大小为主菜单尺寸，避免下次显示时出现抖动
        let _ = menu_window.set_size(Size::Logical(tauri::LogicalSize {
            width: 192.0,
            height: 124.0,
        }));
        let _ = menu_window.hide();
    }
}

#[tauri::command]
fn menu_expand(app: tauri::AppHandle) {
    if let Some(w) = app.webview_windows().get("menu") {
        let (init_x, init_y, opens_left) = {
            let s = DOCK_STATE.lock().unwrap();
            (s.menu_window_x, s.menu_window_y, s.submenu_opens_left)
        };
        eprintln!("menu_expand: init=({}, {}), opens_left={}", init_x, init_y, opens_left);
        if opens_left {
            // 向左展开：窗口 x 左移236（子菜单宽度），宽度扩至428
            let new_x = init_x - 236;
            eprintln!("menu_expand: 向左展开, 新位置 x={}", new_x);
            let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition {
                x: new_x as f64,
                y: init_y as f64,
            }));
        }
        let _ = w.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 428.0,
            height: 360.0,
        }));
    }
}

#[tauri::command]
fn menu_collapse(app: tauri::AppHandle) {
    if let Some(w) = app.webview_windows().get("menu") {
        let (init_x, init_y, opens_left) = {
            let s = DOCK_STATE.lock().unwrap();
            (s.menu_window_x, s.menu_window_y, s.submenu_opens_left)
        };
        if opens_left {
            // 收起：恢复初始 x，宽度缩回192
            let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition {
                x: init_x as f64,
                y: init_y as f64,
            }));
        }
        let _ = w.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 192.0,
            height: 124.0,
        }));
    }
}

// ==================== SETTINGS ====================

#[tauri::command]
fn update_settings(app: tauri::AppHandle, settings: Settings) {
    // Update ball size
    {
        let mut ball_size = BALL_SIZE.lock().unwrap();
        *ball_size = settings.ball_size;
    }

    let _ = app.emit("settings-updated", settings);
}

/// 设置上报认证 Token（供前端调用）
#[tauri::command]
fn set_auth_token(token: String) {
    report_worker::set_auth_token(token);
}

/// 设置上报用户信息（供前端调用）
#[tauri::command]
fn set_report_user_info(user_code: String, user_name: String) {
    report_worker::set_user_info(user_code, user_name);
    println!("[ReportWorker] 认证信息已设置");
}

/// 手动触发一次上报
#[tauri::command]
async fn trigger_report(app: tauri::AppHandle) -> Result<String, String> {
    report_worker::trigger_report_now(&app).await?;
    Ok("上报成功".to_string())
}

#[tauri::command]
fn update_window_size(app: tauri::AppHandle, size: u32) {
    if let Some(main_window) = app.webview_windows().get("main") {
        // 确保最小尺寸，外环需要 ballSize + 8，再加两边 padding
        let actual_size = size.max(30);
        let full_size = actual_size + BALL_PADDING * 2;

        // 获取当前位置和旧尺寸
        let current_pos = main_window.outer_position().ok();
        let old_size = main_window.outer_size().ok();

        if let (Some(pos), Some(old)) = (current_pos, old_size) {
            // 计算新的窗口位置，保持视觉中心不变
            // 当窗口从 120x120 缩小到 84x84 时：
            // - 旧中心 = pos + 60
            // - 新中心 = new_pos + 42
            // - 要保持中心不变: new_pos = pos + 60 - 42 = pos - 18
            let new_x = pos.x - ((old.width as u32 - full_size) / 2) as i32;
            let new_y = pos.y - ((old.height as u32 - full_size) / 2) as i32;

            // 先设置位置，再设置尺寸
            let _ = main_window.set_position(Position::Physical(PhysicalPosition { x: new_x, y: new_y }));
        }

        // 使用 LogicalSize 以正确支持高 DPI 屏幕
        let _ = main_window.set_size(Size::Logical(tauri::LogicalSize {
            width: full_size as f64,
            height: full_size as f64,
        }));

        // 设置窗口为圆形
        apply_circular_window_mask(&main_window, full_size);

        // 同步更新内部状态
        let mut ball_size = BALL_SIZE.lock().unwrap();
        *ball_size = actual_size;
    }
}

// ==================== SCRIPT EXECUTION UTILITIES ====================

/// Get the path to a script file based on the current platform
fn get_script_path(script_name: &str) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    let script_file = format!("{}.ps1", script_name);

    #[cfg(not(target_os = "windows"))]
    let script_file = format!("{}.sh", script_name);

    let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut path = exe_path.clone();
    path.pop(); // Remove executable name

    // 1. Try scripts in the same directory as executable (Windows, Linux, dev mode)
    let script_in_exe_dir = path.join("scripts").join(&script_file);
    if script_in_exe_dir.exists() {
        return script_in_exe_dir;
    }

    // 2. macOS: Try ../Resources/scripts (standard macOS bundle structure)
    #[cfg(target_os = "macos")]
    {
        let mut resources_path = path.clone();
        resources_path.pop(); // Go up to Contents/
        resources_path.push("Resources");
        resources_path.push("scripts");
        resources_path.push(&script_file);
        if resources_path.exists() {
            return resources_path;
        }
    }

    // 3. Fallback: development mode (target/debug or target/release)
    if path.ends_with("debug") || path.ends_with("release") {
        path.pop(); // Remove debug/release
        path.pop(); // Remove target
        path.push("src-tauri");
    }

    path.join("scripts").join(&script_file)
}

/// Execute a script and return its output as JSON
#[cfg(target_os = "windows")]
fn run_script(script_name: &str) -> Result<serde_json::Value, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let script_path = get_script_path(script_name);
    let script_path_str = script_path.to_string_lossy().to_string();

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &script_path_str])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to execute script: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Script failed: {}", stderr));
    }

    serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON output: {} - Output was: {}", e, stdout))
}

/// Execute a script with arguments and return its output as JSON
#[cfg(target_os = "windows")]
fn run_script_with_args(script_name: &str, args: &str) -> Result<serde_json::Value, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let script_path = get_script_path(script_name);
    let script_path_str = script_path.to_string_lossy().to_string();

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &script_path_str, args])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to execute script: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Script failed: {}", stderr));
    }

    serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON output: {} - Output was: {}", e, stdout))
}

/// Execute a script and return its output as JSON
#[cfg(not(target_os = "windows"))]
fn run_script(script_name: &str) -> Result<serde_json::Value, String> {
    use std::process::Command;

    let script_path = get_script_path(script_name);
    let script_path_str = script_path.to_string_lossy().to_string();

    let output = Command::new("/bin/bash")
        .arg(&script_path_str)
        .output()
        .map_err(|e| format!("Failed to execute script {}: {}", script_path_str, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Script {} failed with status {:?}: {}", script_path_str, output.status.code(), stderr));
    }

    serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON output from {}: {} - Output was: {}", script_path_str, e, stdout))
}

/// Execute a script with arguments and return its output as JSON
#[cfg(not(target_os = "windows"))]
fn run_script_with_args(script_name: &str, args: &str) -> Result<serde_json::Value, String> {
    use std::process::Command;

    let script_path = get_script_path(script_name);
    let script_path_str = script_path.to_string_lossy().to_string();

    let output = Command::new("/bin/bash")
        .arg(&script_path_str)
        .arg(args)
        .output()
        .map_err(|e| format!("Failed to execute script: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Script failed: {}", stderr));
    }

    serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON output: {} - Output was: {}", e, stdout))
}

// ==================== OPTIMIZER COMMANDS (Async) ====================

#[tauri::command]
async fn optimizer_scan_all(_app: tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    // Run all scans in parallel using spawn_blocking to avoid blocking the main thread
    let disk_handle = tokio::task::spawn_blocking(|| run_script("disk-scan"));
    let memory_handle = tokio::task::spawn_blocking(|| run_script("memory-status"));
    let health_handle = tokio::task::spawn_blocking(|| run_script("disk-health"));
    let startup_handle = tokio::task::spawn_blocking(|| run_script("startup-list"));
    let system_handle = tokio::task::spawn_blocking(|| run_script("system-info"));

    let mut results = Vec::new();

    // Collect results
    if let Ok(Ok(disk)) = disk_handle.await {
        results.push(disk);
    }
    if let Ok(Ok(memory)) = memory_handle.await {
        results.push(memory);
    }
    if let Ok(Ok(health)) = health_handle.await {
        results.push(health);
    }
    if let Ok(Ok(startup)) = startup_handle.await {
        results.push(startup);
    }
    if let Ok(Ok(system)) = system_handle.await {
        results.push(system);
    }

    Ok(results)
}

#[tauri::command]
async fn optimizer_disk_scan(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(|| run_script("disk-scan"))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn optimizer_disk_health(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(|| run_script("disk-health"))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn optimizer_disk_clean(
    _app: tauri::AppHandle,
    categories_json: String,
) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || {
        // Parse the categories JSON to extract array
        let categories: Vec<String> = serde_json::from_str(&categories_json)
            .unwrap_or_else(|_| vec![]);

        // Convert back to JSON array string for the script
        let categories_arg = serde_json::to_string(&categories).unwrap_or_else(|_| "[]".to_string());

        run_script_with_args("disk-clean", &categories_arg)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn optimizer_memory_status(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(|| run_script("memory-status"))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn optimizer_memory_optimize(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(|| run_script("memory-optimize"))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn optimizer_startup_list(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(|| run_script("startup-list"))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn optimizer_startup_toggle(
    _app: tauri::AppHandle,
    item_json: String,
) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || run_script_with_args("startup-toggle", &item_json))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn optimizer_system_info(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(|| run_script("system-info"))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn show_login_window(app: tauri::AppHandle) {
    // 先隐藏其他所有窗口
    let windows = app.webview_windows();
    if let Some(w) = windows.get("main") {
        let _ = w.hide();
    }
    if let Some(w) = windows.get("menu") {
        let _ = w.hide();
    }
    if let Some(w) = windows.get("optimizer") {
        let _ = w.hide();
    }
    if let Some(w) = windows.get("panel") {
        let _ = w.hide();
    }

    // 检查登录窗口是否已存在
    if let Some(w) = app.webview_windows().get("login") {
        log_msg(&format!("show_login_window: 登录窗口已存在, 可见性: {}", w.is_visible().unwrap_or(false)));

        // 重新导航到远程登录页
        let login_url = build_login_url(&app);
        log_msg(&format!("show_login_window: 重新导航到 {}", login_url));
        let _ = w.navigate(tauri::Url::parse(&login_url).unwrap());

        let _ = w.center();
        let _ = w.show();
        #[cfg(target_os = "windows")]
        {
            use tauri::{LogicalSize, Size};
            let _ = w.set_size(Size::Logical(LogicalSize { width: 361.0, height: 421.0 }));
            let _ = w.set_size(Size::Logical(LogicalSize { width: 360.0, height: 420.0 }));
        }
        let _ = w.set_focus();
    } else {
        // 窗口不存在，在后台线程创建
        log_msg("show_login_window: 登录窗口不存在，尝试动态创建...");
        let app_clone = app.clone();
        tokio::task::spawn_blocking(move || {
            match create_login_window(&app_clone) {
                Ok(w) => {
                    log_msg(&format!("show_login_window: 窗口创建成功"));
                    let _ = w.center();
                    #[cfg(target_os = "windows")]
                    {
                        use tauri::{LogicalSize, Size};
                        let _ = w.set_size(Size::Logical(LogicalSize { width: 361.0, height: 421.0 }));
                        let _ = w.set_size(Size::Logical(LogicalSize { width: 360.0, height: 420.0 }));
                    }
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                Err(e) => {
                    log_msg(&format!("show_login_window: 创建窗口失败: {:?}", e));
                }
            }
        });
    }
}

#[tauri::command]
fn close_login_window(app: tauri::AppHandle) {
    if let Some(w) = app.webview_windows().get("login") {
        let _ = w.hide();
    }
}

/// 更新登录状态并重建托盘菜单
/// 前端登录成功后调用此命令同步状态
#[tauri::command]
fn update_login_status(app: tauri::AppHandle, is_logged_in: bool) {
    log_msg(&format!("update_login_status: is_logged_in={}", is_logged_in));
    IS_LOGGED_IN.store(is_logged_in, Ordering::SeqCst);

    // 获取当前浮动球可见性
    let ball_visible = if let Some(w) = app.webview_windows().get("main") {
        w.is_visible().unwrap_or(false)
    } else {
        false
    };

    rebuild_tray_menu(&app, is_logged_in, ball_visible);
    log_msg(&format!("托盘菜单已更新: is_logged_in={}, ball_visible={}", is_logged_in, ball_visible));
}

/// 登录成功后由 login 窗口调用
/// 关闭登录窗口、更新托盘菜单、通知主窗口初始化
#[tauri::command]
async fn on_login_success(app: tauri::AppHandle) {
    log_msg("on_login_success: 登录成功");
    handle_login_success(&app);
}

/// 保存登录信息到本地文件
/// 由 WebView 内的登录页面调用
#[tauri::command]
fn save_login_info(token: String, user_id: String, user_name: String, user_json: String) -> Result<(), String> {
    log_msg(&format!("[Rust] 保存登录信息: userId={}, userName={}", user_id, user_name));

    // 保存到本地文件
    if let Some(data_dir) = dirs::data_local_dir() {
        let aidi_dir = data_dir.join("AIDI Desktop");
        if let Err(e) = std::fs::create_dir_all(&aidi_dir) {
            return Err(format!("创建目录失败: {}", e));
        }

        let auth_file = aidi_dir.join("auth.json");
        let content = serde_json::json!({
            "token": token,
            "userId": user_id,
            "userName": user_name,
            "user": user_json,
            "updatedAt": chrono::Local::now().to_rfc3339(),
        });

        if let Err(e) = std::fs::write(&auth_file, content.to_string()) {
            return Err(format!("写入文件失败: {}", e));
        }

        log_msg(&format!("[Rust] 登录信息已保存到: {:?}", auth_file));
    } else {
        return Err("无法获取本地数据目录".to_string());
    }

    Ok(())
}

/// 前端调试日志（写入桌面 aidi-debug.log）
#[tauri::command]
fn log_debug(message: String) {
    log_msg(&format!("[前端] {}", message));
}

// ==================== MAIN ENTRY POINT ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 尽早初始化日志文件（在任何其他操作之前）
    init_log_file();
    log_msg("=== AIDI 应用启动 ===");

    // 加载 .env 文件（按优先级：.env.{AIDI_ENV} > .env）
    let env_mode = std::env::var("AIDI_ENV").unwrap_or_else(|_| "test".to_string());
    let env_file = format!(".env.{}", env_mode);
    // 先尝试加载 .env.{mode}，失败则加载 .env
    if dotenv::from_filename(&env_file).is_err() {
        let _ = dotenv::dotenv();
    }
    log_msg(&format!("环境模式: {}", env_mode));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                log_msg("应用 setup 开始");

                // 监听 deep link 事件
                let app_handle = app.handle().clone();
                use tauri_plugin_deep_link::DeepLinkExt;
                app.deep_link().on_open_url(move |event| {
                    let urls = event.urls();
                    log_msg(&format!("[Rust] Deep link 收到 URLs: {:?}", urls));

                    // 转换为字符串数组
                    let url_strings: Vec<String> = urls.iter().map(|u| u.to_string()).collect();

                    // 发送事件到所有窗口
                    if let Some(window) = app_handle.webview_windows().get("login") {
                        let _ = window.emit("deep-link-received", &url_strings);
                    }
                    if let Some(window) = app_handle.webview_windows().get("main") {
                        let _ = window.emit("deep-link-received", &url_strings);
                    }
                });
                log_msg("[Rust] Deep link 监听器已注册");

                // 创建菜单栏 tray icon
                // 使用 PNG 格式（Tauri 不支持 ICO 格式）
                // Windows 使用 32x32 小图标，macOS 使用 tray-icon.png
                #[cfg(target_os = "windows")]
                let tray_icon_bytes = include_bytes!("../icons/32x32.png");
                #[cfg(not(target_os = "windows"))]
                let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");

                log_msg("[Tray] 开始创建托盘图标...");
                // 将托盘创建包装在独立块中，避免错误中断 setup
                let tray_result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let icon = tauri::image::Image::from_bytes(tray_icon_bytes)?;
                    log_msg("[Tray] 图标加载成功");

                    // 初始状态默认为未登录，菜单显示"登录"和"退出"
                    let login_item = MenuItem::with_id(app, "login", "登录", true, None::<&str>)?;
                    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                    let menu = Menu::with_items(app, &[&login_item, &quit_item])?;
                    log_msg("[Tray] 菜单项创建成功");

                    let _tray = TrayIconBuilder::with_id("main-tray")
                        .icon(icon)
                        .tooltip("AIDI Desktop")
                        .menu(&menu)
                        .show_menu_on_left_click(true)
                        .on_tray_icon_event(|_tray, event| {
                            log_msg(&format!("[Tray] 托盘事件: {:?}", event));
                        })
                        .on_menu_event(|_tray, event| {
                            log_msg(&format!("[Tray] 菜单事件: {:?}", event.id));
                        })
                        .build(app)?;

                    log_msg("[Tray] 托盘图标创建成功");
                    Ok(())
                })();

                if let Err(e) = tray_result {
                    log_msg(&format!("[Tray] 托盘创建失败: {:?}，继续初始化其他组件...", e));
                }

                // 全局菜单事件监听（菜单重建后依然有效）
                    app.on_menu_event(|app, event| match event.id.as_ref() {
                        "login" => {
                            // 显示登录窗口
                            log_msg("托盘菜单点击: 登录");
                            if let Some(w) = app.webview_windows().get("login") {
                                log_msg(&format!("托盘登录: 窗口已存在, 可见性: {}, 位置: {:?}, 大小: {:?}",
                                    w.is_visible().unwrap_or(false),
                                    w.outer_position().ok(),
                                    w.outer_size().ok()));
                                let _ = w.center();
                                let _ = w.show();
                                #[cfg(target_os = "windows")]
                                {
                                    use tauri::{LogicalSize, Size};
                                    let _ = w.set_size(Size::Logical(LogicalSize { width: 361.0, height: 421.0 }));
                                    let _ = w.set_size(Size::Logical(LogicalSize { width: 360.0, height: 420.0 }));
                                }
                                let _ = w.set_focus();
                                log_msg(&format!("托盘登录: 显示后, 可见性: {}, 位置: {:?}, 大小: {:?}",
                                    w.is_visible().unwrap_or(false),
                                    w.outer_position().ok(),
                                    w.outer_size().ok()));
                            } else {
                                // 窗口不存在，动态创建
                                log_msg("托盘登录: 窗口不存在，动态创建...");
                                match create_login_window(app) {
                                    Ok(w) => {
                                        log_msg(&format!("托盘登录: 窗口创建成功, 可见性: {}, 位置: {:?}, 大小: {:?}",
                                            w.is_visible().unwrap_or(false),
                                            w.outer_position().ok(),
                                            w.outer_size().ok()));
                                        let _ = w.center();
                                        #[cfg(target_os = "windows")]
                                        {
                                            use tauri::{LogicalSize, Size};
                                            let _ = w.set_size(Size::Logical(LogicalSize { width: 361.0, height: 421.0 }));
                                            let _ = w.set_size(Size::Logical(LogicalSize { width: 360.0, height: 420.0 }));
                                        }
                                        let _ = w.show();
                                        let _ = w.set_focus();
                                        log_msg(&format!("托盘登录: 显示后, 可见性: {}, 位置: {:?}, 大小: {:?}",
                                            w.is_visible().unwrap_or(false),
                                            w.outer_position().ok(),
                                            w.outer_size().ok()));
                                    }
                                    Err(e) => {
                                        log_msg(&format!("托盘登录: 创建窗口失败: {:?}", e));
                                    }
                                }
                            }
                        }
                        "toggle" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let visible = w.is_visible().unwrap_or(false);
                                if visible {
                                    let _ = w.hide();
                                } else {
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                }
                                sync_toggle_menu_item(app, !visible);
                            }
                        }
                        "aigc" => {
                            // 通知前端打开 AIGC 窗口（前端持有 fsUserId）
                            let _ = app.emit_to("main", "open-aigc", ());
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    });

                // Position main window at center
                if let Some(window) = app.webview_windows().get("main") {
                    // 禁用窗口阴影，避免灰色边框
                    #[cfg(any(target_os = "macos", target_os = "windows"))]
                    {
                        let _ = window.set_shadow(false);
                    }
                    // 设置窗口为圆形
                    let ball_size = *BALL_SIZE.lock().unwrap();
                    let full_size = ball_size + BALL_PADDING * 2;
                    apply_circular_window_mask(&window, full_size);
                    if let Some(monitor) = window.current_monitor().ok().flatten() {
                        let screen_size = monitor.size();
                        let scale = monitor.scale_factor();
                        let ball_size = *BALL_SIZE.lock().unwrap();
                        let size = ball_size + BALL_PADDING * 2;
                        // 初始位置：屏幕靠右中下
                        // x: 距离右边 50px
                        // y: 屏幕高度的 70% 位置
                        let margin_right = 50.0;
                        let initial_x = (screen_size.width as f64 - size as f64 * scale - margin_right * scale) as i32;
                        let initial_y = (screen_size.height as f64 * 0.7 - (size as f64 * scale) / 2.0) as i32;
                        // 设置正确的窗口尺寸（与 tauri.conf.json 中的 120x120 不同）
                        let _ = window.set_size(Size::Logical(tauri::LogicalSize {
                            width: size as f64,
                            height: size as f64,
                        }));
                        let _ = window.set_position(Position::Physical(PhysicalPosition {
                            x: initial_x,
                            y: initial_y,
                        }));
                    }
                    // show 触发 webview 初始化（App.vue 开始执行），
                    // 随即 hide 避免浮动球提前显示；
                    // App.vue 内部根据 token 再决定显示浮动球或登录窗口
                    let _ = window.show();
                    let _ = window.hide();
                }

            }
            // 注册全局快捷键 Alt+Q：切换悬浮球显示/隐藏
            // 容错处理：快捷键注册失败不应影响应用启动
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
            let shortcut: Shortcut = "Alt+Q".parse().expect("invalid shortcut");
            if let Err(e) = app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    if let Some(window) = app.webview_windows().get("main") {
                        let visible = window.is_visible().unwrap_or(false);
                        if visible {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                        }
                        sync_toggle_menu_item(app, !visible);
                    }
                }
            }) {
                log_msg(&format!("[Warning] 全局快捷键 Alt+Q 注册失败: {:?}，应用将继续运行", e));
            } else {
                log_msg("[Info] 全局快捷键 Alt+Q 注册成功");
            }

            // 拦截 optimizer 窗口关闭事件：隐藏而不是销毁
            if let Some(optimizer_window) = app.get_webview_window("optimizer") {
                let _ = optimizer_window.clone().on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let _ = optimizer_window.hide();
                        api.prevent_close();
                    }
                });
            }

            // login 窗口改为动态创建，关闭事件拦截在 create_login_window 中设置

            // 检查 main 窗口状态
            if let Some(_main_window) = app.get_webview_window("main") {
                log_msg("main 窗口已找到，等待前端调用 update_login_status");
            } else {
                log_msg("错误: main 窗口未找到！");
            }

            // 超时机制：如果前端 3 秒内没有调用 update_login_status，自动显示登录窗口
            // 这处理了 Windows 上 main 窗口 visible=false 导致 WebView 不加载的问题
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                if !IS_LOGGED_IN.load(Ordering::SeqCst) {
                    log_msg("前端 3 秒内未响应，自动显示登录窗口");
                    if let Some(w) = app_handle.webview_windows().get("login") {
                        log_msg(&format!("login 窗口已存在, 可见性: {}, 位置: {:?}, 大小: {:?}",
                            w.is_visible().unwrap_or(false),
                            w.outer_position().ok(),
                            w.outer_size().ok()));
                        let _ = w.center();
                        let _ = w.show();
                        #[cfg(target_os = "windows")]
                        {
                            use tauri::{LogicalSize, Size};
                            let _ = w.set_size(Size::Logical(LogicalSize { width: 361.0, height: 421.0 }));
                            let _ = w.set_size(Size::Logical(LogicalSize { width: 360.0, height: 420.0 }));
                        }
                        let _ = w.set_focus();
                        log_msg(&format!("login 窗口显示后, 可见性: {}, 位置: {:?}, 大小: {:?}",
                            w.is_visible().unwrap_or(false),
                            w.outer_position().ok(),
                            w.outer_size().ok()));
                    } else {
                        // login 窗口不存在，动态创建
                        log_msg("login 窗口不存在，动态创建...");
                        match create_login_window(&app_handle) {
                            Ok(w) => {
                                log_msg(&format!("login 窗口创建成功, 可见性: {}, 位置: {:?}, 大小: {:?}",
                                    w.is_visible().unwrap_or(false),
                                    w.outer_position().ok(),
                                    w.outer_size().ok()));
                                // Windows 上先居中再显示
                                let _ = w.center();
                                #[cfg(target_os = "windows")]
                                {
                                    use tauri::{LogicalSize, Size};
                                    let _ = w.set_size(Size::Logical(LogicalSize { width: 361.0, height: 421.0 }));
                                    let _ = w.set_size(Size::Logical(LogicalSize { width: 360.0, height: 420.0 }));
                                }
                                let _ = w.show();
                                let _ = w.set_focus();
                                log_msg(&format!("login 窗口显示后, 可见性: {}, 位置: {:?}, 大小: {:?}",
                                    w.is_visible().unwrap_or(false),
                                    w.outer_position().ok(),
                                    w.outer_size().ok()));
                            }
                            Err(e) => {
                                log_msg(&format!("创建 login 窗口失败: {:?}", e));
                            }
                        }
                    }
                }
            });

            // 启动守护线程
            report_worker::start_report_worker(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_main_window,
            hide_main_window,
            show_menu_window,
            hide_menu_window,
            show_optimizer_window,
            hide_optimizer_window,
            open_panel,
            prepare_drag,
            ball_enter,
            ball_leave,
            menu_enter,
            menu_leave,
            menu_expand,
            menu_collapse,
            update_settings,
            set_auth_token,
            set_report_user_info,
            trigger_report,
            update_window_size,
            start_drag,
            move_window_by,
            drag_end,
            hide_docked_ball,
            set_window_position,
            get_window_position,
            show_menu,
            hide_menu,
            menu_ready,
            scroll_ball,
            optimizer_scan_all,
            optimizer_disk_scan,
            optimizer_disk_health,
            optimizer_memory_status,
            optimizer_memory_optimize,
            optimizer_startup_list,
            optimizer_startup_toggle,
            optimizer_system_info,
            optimizer_disk_clean,
            show_login_window,
            close_login_window,
            update_login_status,
            on_login_success,
            log_debug,
            save_login_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
