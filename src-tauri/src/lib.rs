// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(unexpected_cfgs)]
#![allow(deprecated)]

mod report_worker;
mod feishu;

// ── Carbon 全局快捷键（macOS）────────────────────────────────────────────────
// 使用 RegisterEventHotKey 代替 tauri-plugin-global-shortcut（后者底层用
// CGEventTap 监听所有键盘事件，会触发 Apple Music 媒体库权限弹窗）。
// Carbon RegisterEventHotKey 只注册指定组合键，不访问媒体框架。
#[cfg(target_os = "macos")]
mod hotkey {
    use super::*;
    use std::ffi::c_void;

    // kVK_ANSI_D = 0x02，cmdKey modifier = 0x0100
    const KVK_ANSI_D: u32 = 0x02;
    const CMD_KEY: u32 = 0x0100;
    const HOTKEY_ID: u32 = 1;

    #[repr(C)]
    struct EventHotKeyID { signature: u32, id: u32 }

    #[repr(C)]
    struct EventHotKeyRef(*mut c_void);

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn RegisterEventHotKey(
            inHotKeyCode: u32,
            inHotKeyModifiers: u32,
            inHotKeyID: EventHotKeyID,
            inTarget: *mut c_void,
            inOptions: u32,
            outRef: *mut EventHotKeyRef,
        ) -> i32;
        fn GetApplicationEventTarget() -> *mut c_void;
        fn InstallEventHandler(
            inTarget: *mut c_void,
            inHandler: *const c_void,
            inNumTypes: usize,
            inList: *const EventTypeSpec,
            inUserData: *mut c_void,
            outRef: *mut *mut c_void,
        ) -> i32;
    }

    #[repr(C)]
    struct EventTypeSpec { event_class: u32, event_kind: u32 }

    // kEventClassKeyboard = 'keyb' = 0x6B657962，kEventHotKeyPressed = 5
    const K_EVENT_CLASS_KEYBOARD: u32 = 0x6B657962;
    const K_EVENT_HOT_KEY_PRESSED: u32 = 5;

    // 全局存 AppHandle，供 C 回调使用
    static APP_HANDLE: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

    unsafe extern "C" fn hotkey_handler(
        _call_ref: *mut c_void,
        _event: *mut c_void,
        _user_data: *mut c_void,
    ) -> i32 {
        if let Some(app) = APP_HANDLE.get() {
            toggle_ball(app);
        }
        0 // noErr
    }

    fn toggle_ball(app: &AppHandle) {
        if let Some(window) = app.webview_windows().get("main") {
            let visible = BALL_VISIBLE.load(Ordering::SeqCst);
            if visible {
                let _ = window.hide();
                BALL_VISIBLE.store(false, Ordering::SeqCst);
                sync_toggle_menu_item(app, false);
            } else {
                let _ = window.show();
                BALL_VISIBLE.store(true, Ordering::SeqCst);
                sync_toggle_menu_item(app, true);
                let ball_size_val = *BALL_SIZE.lock().unwrap();
                let full_size = ball_size_val + BALL_PADDING * 2;
                apply_circular_window_mask(window, full_size, "shortcut_toggle");
            }
        }
    }

    pub fn register(app: AppHandle) {
        let _ = APP_HANDLE.set(app);
        unsafe {
            let target = GetApplicationEventTarget();
            let spec = EventTypeSpec {
                event_class: K_EVENT_CLASS_KEYBOARD,
                event_kind: K_EVENT_HOT_KEY_PRESSED,
            };
            let mut handler_ref: *mut c_void = std::ptr::null_mut();
            InstallEventHandler(
                target,
                hotkey_handler as *const c_void,
                1,
                &spec,
                std::ptr::null_mut(),
                &mut handler_ref,
            );
            let mut hotkey_ref = EventHotKeyRef(std::ptr::null_mut());
            RegisterEventHotKey(
                KVK_ANSI_D,
                CMD_KEY,
                EventHotKeyID { signature: u32::from_be_bytes(*b"AIDI"), id: HOTKEY_ID },
                target,
                0,
                &mut hotkey_ref,
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn register_hotkey_cmd_d(app: AppHandle) {
    hotkey::register(app);
}

#[cfg(not(target_os = "macos"))]
fn register_hotkey_cmd_d(app: AppHandle) {
    // Windows/Linux：用系统线程 + RegisterHotKey (Win32)
    register_hotkey_windows(app);
}

#[cfg(target_os = "windows")]
fn register_hotkey_windows(app: AppHandle) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_CONTROL, VK_D};
    use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
    std::thread::spawn(move || unsafe {
        RegisterHotKey(None, 1, MOD_CONTROL, VK_D.0 as u32).ok();
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                let visible = BALL_VISIBLE.load(Ordering::SeqCst);
                if let Some(window) = app.webview_windows().get("main") {
                    if visible {
                        let _ = window.hide();
                        BALL_VISIBLE.store(false, Ordering::SeqCst);
                        sync_toggle_menu_item(&app, false);
                    } else {
                        let _ = window.show();
                        BALL_VISIBLE.store(true, Ordering::SeqCst);
                        sync_toggle_menu_item(&app, true);
                        let ball_size_val = *BALL_SIZE.lock().unwrap();
                        let full_size = ball_size_val + BALL_PADDING * 2;
                        apply_circular_window_mask(window, full_size, "shortcut_toggle");
                    }
                }
            }
        }
    });
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn register_hotkey_windows(_app: AppHandle) {}
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Position, Size};
use tauri::tray::TrayIconBuilder;
use tauri::menu::{Menu, MenuItem};


/// 确保悬浮球窗口在聊天窗口之上
fn ensure_ball_above_chat(app: &tauri::AppHandle) {
    let windows = app.webview_windows();
    let Some(main_window) = windows.get("main") else { return };
    let Some(chat_window) = windows.get("chat") else { return };

    // 确保两个窗口都可见
    if let Ok(true) = chat_window.is_visible() {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
            use windows::Win32::UI::WindowsAndMessaging::{SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE};
            use windows::Win32::Foundation::HWND;

            if let (Ok(main_hwnd), Ok(chat_hwnd)) = (
                main_window.hwnd(),
                chat_window.hwnd()
            ) {
                // 将聊天窗口放在悬浮球之后
                unsafe {
                    let _ = SetWindowPos(
                        HWND(chat_hwnd.0),
                        Some(HWND(main_hwnd.0)),
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE
                    );
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: 悬浮球窗口调用 set_focus() 可以将它带到前面
            let _ = main_window.set_focus();
        }
    }
}

// ==================== GLOBAL LOGIN STATUS ====================

/// 全局登录状态（用于动态切换托盘菜单）
static IS_LOGGED_IN: AtomicBool = AtomicBool::new(false);

/// 浮动球预期可见状态（不依赖 is_visible() API，避免 macOS 平台问题）
static BALL_VISIBLE: AtomicBool = AtomicBool::new(false);

/// Windows 专用：WM_NCCALCSIZE 子类化是否已注册（只注册一次）
#[cfg(target_os = "windows")]
static SUBCLASS_INSTALLED: AtomicBool = AtomicBool::new(false);

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
    let env = std::env::var("AIDI_ENV").unwrap_or_else(|_| env!("AIDI_ENV_BAKED").to_string());

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
    let theme_mode = {
        let tm = THEME_MODE.lock().unwrap();
        tm.clone()
    };
    // 只有 THEME_MODE 被显式设置过（用户切换过主题）才带参数；
    // 为空时不带，让前端回落到 localStorage，避免覆盖重启后的持久化设置
    if theme_mode.is_empty() {
        format!("{}/#/menu?direction={}", base_url, direction)
    } else {
        format!("{}/#/menu?direction={}&themeMode={}", base_url, direction, theme_mode)
    }
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
static THEME_MODE: Mutex<String> = Mutex::new(String::new());
const BALL_PADDING: u32 = 6; // 窗口尺寸 = ballSize + BALL_PADDING * 2，增加 6px 上下边距

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
const MENU_HIDE_DELAY_MS: u64 = 300;
#[cfg(not(target_os = "windows"))]
const MENU_HIDE_DELAY_MS: u64 = 80;

// Ease-out cubic function: 1 - (1-t)^3
fn ease_out_cubic(t: f32) -> f32 {
    let t_inv = 1.0 - t;
    1.0 - t_inv * t_inv * t_inv
}

/// Windows 专用：WndProc 子类化回调，拦截 WM_NCCALCSIZE 将非客户区归零
/// 这是从协议层彻底消除标题栏热区的标准方案（Chromium/Electron 同款）
#[cfg(target_os = "windows")]
unsafe extern "system" fn ball_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _uid_subclass: usize,
    _ref_data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{WM_NCCALCSIZE, WM_NCACTIVATE, WM_NCPAINT};
    use windows::Win32::UI::Shell::DefSubclassProc;

    if msg == WM_NCCALCSIZE && wparam.0 != 0 {
        // 强制非客户区大小为零，系统不再分配标题栏/边框区域
        // 返回 0 时 Windows 会将整个窗口矩形用作客户区
        return windows::Win32::Foundation::LRESULT(0);
    }

    // 拦截 NC 激活重绘：返回 TRUE(1) 阻止 DWM 绘制灰色标题栏
    if msg == WM_NCACTIVATE {
        return windows::Win32::Foundation::LRESULT(1);
    }

    // 拦截 NC 绘制：直接吞掉，不绘制任何 NC 内容
    if msg == WM_NCPAINT {
        return windows::Win32::Foundation::LRESULT(0);
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

/// 设置窗口为透明矩形（悬浮球 + 搜索按钮并排布局）
/// caller: 调用来源标识，用于诊断日志对比（如 "init", "on_blur", "after_menu", "show"）
#[allow(unused_variables)]
fn apply_circular_window_mask(window: &tauri::WebviewWindow, _size: u32, _caller: &str) {
    #[cfg(target_os = "macos")]
    {
        // macOS: 不需要设置圆角，前端 CSS 处理
    }

    #[cfg(windows)]
    {
        // Windows: 设置窗口样式但不再应用圆形遮罩
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;
        use windows::Win32::Graphics::Dwm::*;
        use std::ffi::c_void;

        if let Ok(hwnd) = window.hwnd() {
            let hwnd = HWND(hwnd.0);

            unsafe {
                // 1. 清除标题栏装饰相关样式位
                const DECORATION_MASK: i32 = 0x00CF0000u32 as i32;
                let old_style = GetWindowLongW(hwnd, GWL_STYLE);
                let new_style = old_style & !DECORATION_MASK;
                SetWindowLongW(hwnd, GWL_STYLE, new_style);

                // 2. 添加 WS_EX_LAYERED（分层窗口，支持透明）
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as isize);

                // 3. 禁用 DWM NC 渲染
                const DWMWA_NCRENDERING_POLICY_VAL: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE =
                    windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(2);
                const DWMNCRP_DISABLED: i32 = 1;
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_NCRENDERING_POLICY_VAL,
                    &DWMNCRP_DISABLED as *const i32 as *const c_void, std::mem::size_of::<i32>() as u32);

                // 4. 禁用系统背景
                const DWMSBT_NONE: i32 = 1;
                let backdrop_type: i32 = DWMSBT_NONE;
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE,
                    &backdrop_type as *const i32 as *const c_void, std::mem::size_of::<i32>() as u32);

                // 5. 触发刷新
                let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);

                // 6. WndProc 子类化：拦截 WM_NCCALCSIZE
                if !SUBCLASS_INSTALLED.load(Ordering::Relaxed) {
                    use windows::Win32::UI::Shell::SetWindowSubclass;
                    let ok = SetWindowSubclass(hwnd, Some(ball_window_proc), 1, 0);
                    if ok.as_bool() {
                        SUBCLASS_INSTALLED.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
    }
}

/// 为无边框透明窗口应用 Windows 样式（无标题栏、无遮罩）
/// 用于聊天窗口等矩形无边框窗口
#[allow(unused_variables)]
fn apply_borderless_window_style(window: &tauri::WebviewWindow, _caller: &str) {
    #[cfg(target_os = "macos")]
    {
        // macOS: 不需要额外处理
    }

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;
        use windows::Win32::Graphics::Dwm::*;
        use std::ffi::c_void;

        if let Ok(hwnd) = window.hwnd() {
            let hwnd = HWND(hwnd.0);

            unsafe {
                // 1. 清除标题栏装饰相关样式位（WS_CAPTION | WS_BORDER | WS_DLGFRAME | WS_THICKFRAME 等）
                const DECORATION_MASK: i32 = 0x00CF0000u32 as i32;
                let old_style = GetWindowLongW(hwnd, GWL_STYLE);
                let new_style = old_style & !DECORATION_MASK;
                SetWindowLongW(hwnd, GWL_STYLE, new_style);

                // 2. 添加 WS_EX_LAYERED（分层窗口，支持透明）
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as isize);

                // 3. 禁用 DWM NC 渲染（阻止 DWM 绘制非客户区）
                const DWMWA_NCRENDERING_POLICY_VAL: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE =
                    windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(2);
                const DWMNCRP_DISABLED: i32 = 1;
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_NCRENDERING_POLICY_VAL,
                    &DWMNCRP_DISABLED as *const i32 as *const c_void, std::mem::size_of::<i32>() as u32);

                // 4. 禁用系统背景
                const DWMSBT_NONE: i32 = 1;
                let backdrop_type: i32 = DWMSBT_NONE;
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE,
                    &backdrop_type as *const i32 as *const c_void, std::mem::size_of::<i32>() as u32);

                // 5. 触发 SWP_FRAMECHANGED 刷新窗口框架
                let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
            }
        }
    }
}

/// Windows 专用：延迟刷新悬浮球窗口遮罩
/// 用于解决 Z-order 变化后出现灰色背景的问题
#[cfg(target_os = "windows")]
fn schedule_refresh_ball_mask(app: &tauri::AppHandle) {
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));  // 短暂延迟，等待 Z-order 稳定
        if let Some(main_w) = app_clone.webview_windows().get("main") {
            let ball_size_val = *BALL_SIZE.lock().unwrap();
            let full_size = ball_size_val + BALL_PADDING * 2;
            apply_circular_window_mask(&main_w, full_size, "schedule_refresh");
        }
    });
}

/// Windows 专用：诊断悬浮球窗口状态
/// 用于排查灰色弧形背景问题
#[cfg(windows)]
fn diagnose_window_state(window: &tauri::WebviewWindow) -> String {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_SYSTEMBACKDROP_TYPE,
    };
    use windows::Win32::Graphics::Gdi::GetWindowRgn;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowLongPtrW, GetWindowLongW, GetWindowRect, GWL_EXSTYLE, GWL_STYLE,
    };

    let mut result = String::new();
    result.push_str("=== Window Diagnosis ===\n");

    if let Ok(hwnd) = window.hwnd() {
        let hwnd = HWND(hwnd.0);
        result.push_str(&format!("HWND: {:p}\n", hwnd.0));

        unsafe {
            // 1. 窗口样式
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let style_u32 = style as u32;
            result.push_str(&format!("Style: 0x{:08X}\n", style_u32));

            // 解析样式位
            let ws_visible = style_u32 & 0x10000000 != 0;
            let ws_popup = style_u32 & 0x80000000 != 0;
            let ws_caption = style_u32 & 0x00C00000 != 0;
            let ws_border = style_u32 & 0x00800000 != 0;
            let ws_thickframe = style_u32 & 0x00040000 != 0;
            result.push_str(&format!(
                "  WS_VISIBLE={} WS_POPUP={} WS_CAPTION={} WS_BORDER={} WS_THICKFRAME={}\n",
                ws_visible, ws_popup, ws_caption, ws_border, ws_thickframe
            ));

            // 2. 扩展样式
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let ex_style_u32 = ex_style as u32;
            result.push_str(&format!("ExStyle: 0x{:08X}\n", ex_style_u32));

            let ws_ex_layered = ex_style_u32 & 0x00080000 != 0;
            let ws_ex_transparent = ex_style_u32 & 0x00000020 != 0;
            let ws_ex_toolwindow = ex_style_u32 & 0x00000080 != 0;
            let ws_ex_topmost = ex_style_u32 & 0x00000008 != 0;
            result.push_str(&format!(
                "  WS_EX_LAYERED={} WS_EX_TRANSPARENT={} WS_EX_TOOLWINDOW={} WS_EX_TOPMOST={}\n",
                ws_ex_layered, ws_ex_transparent, ws_ex_toolwindow, ws_ex_topmost
            ));

            // 3. 窗口矩形
            let mut rect = windows::Win32::Foundation::RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            result.push_str(&format!(
                "WindowRect: ({}, {}) - ({}, {}) [{}x{}]\n",
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                rect.right - rect.left,
                rect.bottom - rect.top
            ));

            // 4. 客户区矩形
            let mut client_rect = windows::Win32::Foundation::RECT::default();
            let _ = GetClientRect(hwnd, &mut client_rect);
            result.push_str(&format!(
                "ClientRect: ({}, {}) - ({}, {}) [{}x{}]\n",
                client_rect.left,
                client_rect.top,
                client_rect.right,
                client_rect.bottom,
                client_rect.right - client_rect.left,
                client_rect.bottom - client_rect.top
            ));

            // 5. 窗口区域 (Region)
            let region_result = GetWindowRgn(hwnd, windows::Win32::Graphics::Gdi::HRGN::default());
            let region_code = region_result.0;
            result.push_str(&format!("Region result: {} (0=ERROR, 1=NULL, 2=SIMPLE, 3=COMPLEX)\n", region_code));

            // 6. DWM 扩展帧边界
            let mut bounds = windows::Win32::Foundation::RECT::default();
            let dwm_result = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut bounds as *mut _ as *mut c_void,
                std::mem::size_of::<windows::Win32::Foundation::RECT>() as u32,
            );
            let dwm_ok = dwm_result.is_ok();
            result.push_str(&format!(
                "DWM ExtendedFrameBounds: ({}, {}) - ({}, {}) [ok={}]\n",
                bounds.left, bounds.top, bounds.right, bounds.bottom, dwm_ok
            ));

            // 计算与 WindowRect 的差异（表示非客户区大小）
            let nc_left = rect.left - bounds.left;
            let nc_top = rect.top - bounds.top;
            let nc_right = bounds.right - rect.right;
            let nc_bottom = bounds.bottom - rect.bottom;
            result.push_str(&format!(
                "  Non-client margins: left={} top={} right={} bottom={}\n",
                nc_left, nc_top, nc_right, nc_bottom
            ));

            // 7. DWM 系统背景类型
            let mut backdrop_type: i32 = 0;
            let backdrop_result = DwmGetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &mut backdrop_type as *mut i32 as *mut c_void,
                std::mem::size_of::<i32>() as u32,
            );
            let backdrop_ok = backdrop_result.is_ok();
            result.push_str(&format!(
                "DWM SystemBackdropType: {} [ok={}] (1=NONE, 2=MICA, 3=ACRYLIC, 4=TABBED)\n",
                backdrop_type, backdrop_ok
            ));
        }
    } else {
        result.push_str("Error: Failed to get HWND\n");
    }

    result.push_str("=======================\n");
    result
}

#[cfg(not(windows))]
fn diagnose_window_state(_window: &tauri::WebviewWindow) -> String {
    "Diagnosis only available on Windows".to_string()
}

/// Tauri 命令：诊断悬浮球窗口状态
#[tauri::command]
fn diagnose_window(window: tauri::WebviewWindow) -> String {
    diagnose_window_state(&window)
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

        // 动画结束后重新应用圆形遮罩，解决灰色弧形背景问题
        #[cfg(target_os = "windows")]
        {
            let ball_size_val = *BALL_SIZE.lock().unwrap();
            let full_size = ball_size_val + BALL_PADDING * 2;
            apply_circular_window_mask(window, full_size, "after_animation");
        }
    }
}

// ==================== POSITION DETECTION FUNCTIONS ====================

// ==================== WINDOW MANAGEMENT ====================

#[tauri::command]
fn show_main_window(app: tauri::AppHandle, window: tauri::Window) {
    let _ = window.show();
    BALL_VISIBLE.store(true, Ordering::SeqCst);
    sync_toggle_menu_item(&app, true);
    // Windows 上 show() 后重新应用圆形遮罩，防止 WS_CAPTION 热区重现
    if let Some(w) = app.get_webview_window("main") {
        let ball_size_val = *BALL_SIZE.lock().unwrap();
        let full_size = ball_size_val + BALL_PADDING * 2;
        apply_circular_window_mask(&w, full_size, "show_main");
    }
}

#[tauri::command]
fn hide_main_window(app: tauri::AppHandle, window: tauri::Window) {
    let _ = window.hide();
    // 隐藏其他所有打开的窗口
    let windows = app.webview_windows();
    for (label, win) in &windows {
        if label != "main" {
            let _ = win.hide();
        }
    }
    BALL_VISIBLE.store(false, Ordering::SeqCst);
    sync_toggle_menu_item(&app, false);
}

/// 根据登录状态重建托盘菜单
/// - 未登录：只显示"登录"选项
/// - 已登录：显示"打开AIDI"、"显示/隐藏浮动球"、"退出"
fn rebuild_tray_menu(app: &tauri::AppHandle, is_logged_in: bool, ball_visible: bool) {
    if let Some(tray) = app.tray_by_id("main-tray") {
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
                }
            }
        }
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
fn prepare_drag(app: tauri::AppHandle) -> (i32, i32) {
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

    // 立即记录当前物理位置到 DRAG_WINDOW_X/Y，避免 start_drag 的竞态问题
    // （动画线程可能正在修改位置，这里在取消动画后立即抢占）
    let result = if let Some(w) = app.webview_windows().get("main") {
        if let Ok(pos) = w.outer_position() {
            DRAG_WINDOW_X.store(pos.x, Ordering::SeqCst);
            DRAG_WINDOW_Y.store(pos.y, Ordering::SeqCst);
            (pos.x, pos.y)
        } else {
            (DRAG_WINDOW_X.load(Ordering::SeqCst), DRAG_WINDOW_Y.load(Ordering::SeqCst))
        }
    } else {
        (DRAG_WINDOW_X.load(Ordering::SeqCst), DRAG_WINDOW_Y.load(Ordering::SeqCst))
    };

    // 更新状态，但不重置 is_docked（让 drag_end 处理）
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.ball_hover = true;
        state.interaction_state = InteractionState::Dragging;
        // 不要设置 is_docked = false，让 drag_end 来决定
        // 只重置 popped_out 状态，因为拖拽开始时球已经弹出了
        state.is_popped_out = false;
    }

    result
}

#[tauri::command]
fn ball_enter(app: tauri::AppHandle) {
    // 如果浮动球被托盘菜单隐藏，不响应鼠标进入事件
    if !BALL_VISIBLE.load(Ordering::SeqCst) {
        return;
    }

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
    // 先读取窗口实际当前位置，消除 prepare_drag 竞态（避免 DRAG_WINDOW_X/Y 未初始化时窗口跳到左上角）
    let (cur_x, cur_y) = match window.outer_position() {
        Ok(pos) => (pos.x, pos.y),
        Err(_) => {
            let x = DRAG_WINDOW_X.load(Ordering::Relaxed);
            let y = DRAG_WINDOW_Y.load(Ordering::Relaxed);
            (x, y)
        }
    };
    let new_x = cur_x + dx;
    let new_y = cur_y + dy;
    DRAG_WINDOW_X.store(new_x, Ordering::Relaxed);
    DRAG_WINDOW_Y.store(new_y, Ordering::Relaxed);
    let _ = window.set_position(Position::Physical(PhysicalPosition { x: new_x, y: new_y }));
}

/// 直接设置窗口绝对坐标（物理像素），不调用 outer_position，Windows 上性能更优
#[tauri::command]
fn move_window_to(app: tauri::AppHandle, x: i32, y: i32) {
    DRAG_WINDOW_X.store(x, Ordering::Relaxed);
    DRAG_WINDOW_Y.store(y, Ordering::Relaxed);

    // 移动悬浮球窗口
    if let Some(main_window) = app.webview_windows().get("main") {
        let _ = main_window.set_position(Position::Physical(PhysicalPosition { x, y }));

        // 同步更新聊天窗口位置（如果可见）
        if let Some(chat_window) = app.webview_windows().get("chat") {
            if let Ok(true) = chat_window.is_visible() {
                if let Ok(ball_size) = main_window.outer_size() {
                    if let Ok(chat_size) = chat_window.outer_size() {
                        // 聊天窗口与悬浮球窗口居中对齐
                        let ball_center = x + ball_size.width as i32 / 2;
                        let chat_x = ball_center - chat_size.width as i32 / 2;
                        let chat_y = y + ball_size.height as i32;
                        let _ = chat_window.set_position(Position::Physical(PhysicalPosition {
                            x: chat_x,
                            y: chat_y,
                        }));

                        // 确保 z-order：悬浮球在聊天窗口之上
                        ensure_ball_above_chat(&app);
                    }
                }
            }
        }
    }
}

#[tauri::command]
fn drag_end(_app: tauri::AppHandle) {
    // 移除边缘吸附效果 - 仅重置交互状态
    let mut state = DOCK_STATE.lock().unwrap();
    state.is_docked = false;
    state.dock_edge = None;
    state.is_popped_out = false;
    state.is_in_pop_protection = false;
    state.interaction_state = InteractionState::Idle;
    drop(state);

    // 取消任何待处理的动画和弹出保护定时器
    next_state_version();
    let mut timer = POP_PROTECTION_TIMER.lock().unwrap();
    if let Some(_handle) = timer.take() {
        // Let existing thread finish
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

    // 先用 about:blank 创建窗口，避免 build() 阻塞 UI 线程（与登录窗口保持一致的模式）
    let blank_url = tauri::WebviewUrl::External(tauri::Url::parse("about:blank").unwrap());
    // 使用 about:blank 先创建再 navigate，可绕过 Windows WebView2 transparent+直接加载远端URL 的挂起 bug
    let menu_transparent = true;

    let builder = tauri::WebviewWindowBuilder::new(app, "menu", blank_url)
        .title("Menu")
        .inner_size(192.0, 152.0)
        .decorations(false)
        .transparent(menu_transparent)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .devtools(true);

    #[cfg(target_os = "macos")]
    let builder = builder.hidden_title(true);

    let menu_window = builder
        .on_navigation(move |url: &tauri::Url| {
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
                                        // 重新应用圆形遮罩，防止 WS_CAPTION 热区重现
                                        let ball_size_val = *BALL_SIZE.lock().unwrap();
                                        let full_size = ball_size_val + BALL_PADDING * 2;
                                        apply_circular_window_mask(w, full_size, "menu_show_main");
                                    }
                                }
                                "hide_main_window" => {
                                    if let Some(w) = app2.webview_windows().get("main") {
                                        let _ = w.hide();
                                        sync_toggle_menu_item(&app2, false);
                                    }
                                }
                                "show_login_window" => {
                                    let app3 = app2.clone();
                                    tauri::async_runtime::spawn(async move {
                                        show_login_window(app3).await;
                                    });
                                }
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
                                            // 向左展开：窗口 x 左移236，宽度扩至428
                                            let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition {
                                                x: (init_x - 236) as f64,
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
                                            height: 152.0,
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
        // Windows WebView2 的 hash-only 导航不触发 NavigationStarting 事件，因此 on_navigation 不被调用。
        // 通过 initialization_script 监听 hashchange 事件，直接 invoke Tauri 命令作为补充方案。
        // 与现有 on_navigation 回调共存不冲突：macOS 走 on_navigation，Windows 走此脚本。
        .initialization_script(r#"
(function() {
    // 主题初始化：直接在 <html> 上设置内联 CSS 变量
    // 内联样式优先级最高，不依赖远端 CSS 版本、.dark 类或 localStorage 是否可用
    try {
        var __m = 'system';
        try {
            var __saved = localStorage.getItem('aidi-settings');
            if (__saved) __m = JSON.parse(__saved).themeMode || 'system';
        } catch(e2) {}
        var __d = __m === 'dark' ||
            (__m === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
        var __el = document.documentElement;
        __el.classList.toggle('dark', __d);
        if (__d) {
            __el.style.setProperty('--foreground',          'oklch(0.985 0 0)');
            __el.style.setProperty('--background',          'oklch(0.145 0 0)');
            __el.style.setProperty('--card',                'oklch(0.205 0 0)');
            __el.style.setProperty('--card-foreground',     'oklch(0.985 0 0)');
            __el.style.setProperty('--muted',               'oklch(0.269 0 0)');
            __el.style.setProperty('--muted-foreground',    'oklch(0.708 0 0)');
            __el.style.setProperty('--accent',              'oklch(0.269 0 0)');
            __el.style.setProperty('--accent-foreground',   'oklch(0.985 0 0)');
            __el.style.setProperty('--primary',             'oklch(0.922 0 0)');
            __el.style.setProperty('--primary-foreground',  'oklch(0.205 0 0)');
            __el.style.setProperty('--border',              'oklch(1 0 0 / 10%)');
            __el.style.setProperty('--input',               'oklch(1 0 0 / 15%)');
            __el.style.setProperty('--secondary',           'oklch(0.269 0 0)');
            __el.style.setProperty('--secondary-foreground','oklch(0.985 0 0)');
        } else {
            __el.style.setProperty('--foreground',          'oklch(0.145 0 0)');
            __el.style.setProperty('--background',          'oklch(1 0 0)');
            __el.style.setProperty('--card',                'oklch(1 0 0)');
            __el.style.setProperty('--card-foreground',     'oklch(0.145 0 0)');
            __el.style.setProperty('--muted',               'oklch(0.97 0 0)');
            __el.style.setProperty('--muted-foreground',    'oklch(0.556 0 0)');
            __el.style.setProperty('--accent',              'oklch(0.97 0 0)');
            __el.style.setProperty('--accent-foreground',   'oklch(0.205 0 0)');
            __el.style.setProperty('--primary',             'oklch(0.205 0 0)');
            __el.style.setProperty('--primary-foreground',  'oklch(0.985 0 0)');
            __el.style.setProperty('--border',              'oklch(0.922 0 0)');
            __el.style.setProperty('--input',               'oklch(0.922 0 0)');
            __el.style.setProperty('--secondary',           'oklch(0.97 0 0)');
            __el.style.setProperty('--secondary-foreground','oklch(0.205 0 0)');
        }
    } catch(e) {}

    var _ALLOWED = ['hide_menu','show_optimizer_window','hide_optimizer_window',
        'show_main_window','hide_main_window','show_login_window','hide_login_window',
        'show_menu_window','menu_expand','menu_collapse','update_settings'];
    function handleHash() {
        var h = window.location.hash;
        if (!h || h.indexOf('invoke=') === -1) return;
        var params = {};
        h.replace(/^#/, '').split('&').forEach(function(pair) {
            var idx = pair.indexOf('=');
            if (idx > 0) params[decodeURIComponent(pair.slice(0,idx))] = decodeURIComponent(pair.slice(idx+1));
        });
        var cmd = params['invoke'];
        if (!cmd || _ALLOWED.indexOf(cmd) === -1) return;
        history.replaceState(null, '', window.location.pathname + window.location.search);
        var args = {};
        if (cmd === 'update_settings') {
            ['ball_size','opacity','color_theme','theme_mode'].forEach(function(k){
                if (params[k] !== undefined) args[k] = params[k];
            });
        } else if (cmd === 'menu_expand' || cmd === 'menu_collapse') {
            if (params['opens_left'] !== undefined) args['opens_left'] = params['opens_left'] === 'true';
        }
        window.__TAURI_INTERNALS__.invoke(cmd, args)
            .catch(function(e){ console.warn('[Menu] invoke failed:', cmd, e); });
    }
    window.addEventListener('hashchange', handleHash);
    handleHash();
})();
"#)
        .build()?;

    // 禁用系统阴影/边框，与 main 窗口保持一致
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let _ = menu_window.set_shadow(false);
    }

    // 监听窗口失焦事件，点击外部时自动隐藏菜单
    let app_for_event = app.clone();
    let _ = menu_window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            let app_clone = app_for_event.clone();
            // 延迟检查，避免菜单内部点击时短暂失焦误触发
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if let Some(w) = app_clone.webview_windows().get("menu") {
                    if w.is_visible().unwrap_or(false) {
                        hide_menu(app_clone);
                    }
                }
            });
        }
    });

    // build() 返回后再异步 navigate 到远程 URL，避免阻塞 UI 线程
    let menu_url = tauri::Url::parse(&menu_url_str).unwrap();
    let _ = menu_window.navigate(menu_url);

    Ok(menu_window)
}

// 登录窗口创建状态标志
static LOGIN_WINDOW_CREATING: AtomicBool = AtomicBool::new(false);

/// 创建登录窗口（动态创建，加载远程登录页）
fn create_login_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, tauri::Error> {
    // 检查是否正在创建中
    if LOGIN_WINDOW_CREATING.load(Ordering::SeqCst) {
        return Err(tauri::Error::WindowNotFound);
    }

    LOGIN_WINDOW_CREATING.store(true, Ordering::SeqCst);
    let app_handle = app.clone();
    let login_url_str = build_login_url(app);

    // 先用 about:blank 创建窗口，避免 build() 郻塞
    let blank_url = tauri::WebviewUrl::External(tauri::Url::parse("about:blank").unwrap());

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
        .devtools(true)
        // 注入脚本：监听 hash 变更，检测到登录成功后直接 invoke 通知 Rust
        .initialization_script(r#"
            (function() {
                var _handled = false;
                function checkLoginSuccess() {
                    if (_handled) return;
                    var h = window.location.hash;
                    if (!h || h.indexOf('invoke=login-success') === -1) return;
                    _handled = true;
                    clearInterval(_t);
                    // 解析 hash 参数
                    var params = {};
                    h.replace(/^#/, '').split('&').forEach(function(pair) {
                        var idx = pair.indexOf('=');
                        if (idx > 0) params[decodeURIComponent(pair.slice(0, idx))] = decodeURIComponent(pair.slice(idx + 1));
                    });
                    var token = params['token'] || '';
                    var user = params['user'] || '';
                    // 直接 invoke 带参通知 Rust（initialization_script 注入时 IPC bridge 已就绪）
                    window.__TAURI_INTERNALS__.invoke('on_login_success', { token: token, user: user })
                        .catch(function(e) { console.warn('[AIDI] on_login_success failed:', e); });
                }
                window.addEventListener('hashchange', checkLoginSuccess);
                var _t = setInterval(checkLoginSuccess, 300);
            })();
        "#)
        .on_navigation(move |url: &tauri::Url| {
            let _url_str = url.to_string();
            // 前端通过 URL 报告的错误（invoke 不可用时的兜底诊断）
            if url.path().contains("aidi-login-error") {
                let _msg = url.query_pairs()
                    .find(|(k, _)| k == "msg")
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
                return false; // 阻止导航到不存在的页面
            }

            // 监听登录成功：解析 hash 中的 invoke=login-success&token=xxx&user=yyy
            if let Some(fragment) = url.fragment() {
                if let Some(rest) = fragment.strip_prefix("invoke=login-success") {
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

                                if let Err(_) = std::fs::write(&auth_file, content.to_string()) {
                                } else {
                                }
                            }

                            // 执行登录成功逻辑（显示主窗口、更新托盘等）
                            handle_login_success(&app2);
                        });
                        return false; // 阻止跳转到 about:blank，保持登录页可见直到窗口被隐藏
                    }
                }
            }
            true
        })
        .build();
    let login_window = match build_result {
        Ok(w) => {
            w
        },
        Err(e) => {
            LOGIN_WINDOW_CREATING.store(false, Ordering::SeqCst);
            return Err(e);
        },
    };

    // 窗口创建成功后，显示窗口并导航到远程登录页
    let _ = login_window.center();
    let _ = login_window.show();
    let _ = login_window.set_focus();
    // 使用 navigate() 跳转到远程登录页
    let login_url = tauri::Url::parse(&login_url_str).unwrap();
    let _ = login_window.navigate(login_url);

    // 设置窗口关闭拦截：隐藏而不是销毁
    let login_window_clone = login_window.clone();
    let _ = login_window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let _ = login_window_clone.hide();
            api.prevent_close();
        }
    });

    // 添加加载超时检测，防止网络问题导致窗口卡死
    let window_clone = login_window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        // 如果窗口还在加载中，尝试获取窗口状态
        let _ = window_clone.is_visible();
    });

    LOGIN_WINDOW_CREATING.store(false, Ordering::SeqCst);
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

// ==================== CHAT WINDOW ====================

/// 聊天窗口创建状态标志
static CHAT_WINDOW_CREATING: AtomicBool = AtomicBool::new(false);

/// 创建聊天气泡窗口
fn create_chat_window(app: &tauri::AppHandle, initial_message: Option<&str>) -> Result<tauri::WebviewWindow, tauri::Error> {
    // 检查是否正在创建中
    if CHAT_WINDOW_CREATING.load(Ordering::SeqCst) {
        return Err(tauri::Error::WindowNotFound);
    }

    CHAT_WINDOW_CREATING.store(true, Ordering::SeqCst);
    // 构建聊天 URL，如果有初始消息则通过 query 参数传递
    let base_url = get_external_url_base(app);
    let chat_url_str = match initial_message {
        Some(msg) => {
            let encoded = urlencoding::encode(msg);
            format!("{}/#/chat?message={}", base_url, encoded)
        }
        None => format!("{}/#/chat", base_url)
    };
    // 先用 about:blank 创建窗口，避免 build() 阻塞
    let blank_url = tauri::WebviewUrl::External(tauri::Url::parse("about:blank").unwrap());
    let builder = tauri::WebviewWindowBuilder::new(app, "chat", blank_url)
        .title("AIDI 聊天")
        .inner_size(320.0, 428.0)  // 280px + padding + 8px 尖头区域
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)                // 允许用户拖拽调整（无尺寸限制）
        .visible(false)
        .devtools(true);

    #[cfg(target_os = "macos")]
    let builder = builder.hidden_title(true);

    let chat_window = builder
        .initialization_script(r#"
            (function() {
                // 监听关闭聊天窗口的命令
                window.addEventListener('message', function(e) {
                    if (e.data && e.data.type === 'close-chat') {
                        window.__TAURI_INTERNALS__.invoke('hide_chat_window');
                    }
                });
            })();
        "#)
        .build()?;

    // 处理 Windows 无边框样式，确保完全移除标题栏
    apply_borderless_window_style(&chat_window, "chat_window");

    // 禁用系统阴影
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let _ = chat_window.set_shadow(false);
    }

    // 获取悬浮球窗口位置，将聊天窗口定位在悬浮球正下方
    if let Some(main_window) = app.webview_windows().get("main") {
        if let Ok(ball_pos) = main_window.outer_position() {
            if let Ok(ball_size) = main_window.outer_size() {
                if let Ok(chat_size) = chat_window.outer_size() {
                    // 聊天窗口与悬浮球窗口居中对齐
                    let ball_center = ball_pos.x + ball_size.width as i32 / 2;
                    let chat_x = ball_center - chat_size.width as i32 / 2;
                    let chat_y = ball_pos.y + ball_size.height as i32;

                    let _ = chat_window.set_position(Position::Physical(PhysicalPosition {
                        x: chat_x,
                        y: chat_y,
                    }));
                }
            }
        }
    }

    // build() 返回后再异步 navigate 到远程 URL
    let chat_url = tauri::Url::parse(&chat_url_str).unwrap();
    let _ = chat_window.navigate(chat_url);

    // 设置窗口事件处理：关闭拦截 + 大小变化时重新定位
    let chat_window_clone = chat_window.clone();
    let app_clone = app.clone();
    let _ = chat_window.on_window_event(move |event| {
        match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let _ = chat_window_clone.hide();
                api.prevent_close();
            }
            tauri::WindowEvent::Resized(new_size) => {
                // 窗口大小改变后，重新定位到悬浮球正下方（使用新尺寸计算位置）
                if let Some(main_window) = app_clone.webview_windows().get("main") {
                    if let Ok(ball_pos) = main_window.outer_position() {
                        if let Ok(ball_size) = main_window.outer_size() {
                            let ball_center = ball_pos.x + ball_size.width as i32 / 2;
                            let chat_x = ball_center - new_size.width as i32 / 2;
                            let chat_y = ball_pos.y + ball_size.height as i32;
                            let _ = chat_window_clone.set_position(Position::Physical(PhysicalPosition {
                                x: chat_x,
                                y: chat_y,
                            }));
                        }
                    }
                }
            }
            _ => {}
        }
    });

    CHAT_WINDOW_CREATING.store(false, Ordering::SeqCst);
    Ok(chat_window)
}

/// 显示聊天窗口
#[tauri::command]
async fn show_chat_window(app: tauri::AppHandle, initial_message: Option<String>, visible: Option<bool>) {
    let should_show = visible.unwrap_or(true);
    // 如果窗口已存在，显示并更新位置
    if let Some(chat_window) = app.webview_windows().get("chat") {
        // 更新窗口位置到悬浮球窗口正下方
        if let Some(main_window) = app.webview_windows().get("main") {
            if let Ok(ball_pos) = main_window.outer_position() {
                if let Ok(ball_size) = main_window.outer_size() {
                    if let Ok(chat_size) = chat_window.outer_size() {
                        // 聊天窗口与悬浮球窗口居中对齐
                        let ball_center = ball_pos.x + ball_size.width as i32 / 2;
                        let chat_x = ball_center - chat_size.width as i32 / 2;
                        let chat_y = ball_pos.y + ball_size.height as i32;
                        let _ = chat_window.set_position(Position::Physical(PhysicalPosition {
                            x: chat_x,
                            y: chat_y,
                        }));
                    }
                }
            }
        }

        if should_show {
            let _ = chat_window.show();
            let _ = chat_window.set_focus();
            // 确保悬浮球在聊天窗口之上
            ensure_ball_above_chat(&app);
        }

        // 如果有初始消息，发送到聊天窗口
        if let Some(msg) = initial_message {
            let _ = chat_window.emit("chat-initial-message", msg);
        }
    } else {
        // 窗口不存在，创建新窗口
        match create_chat_window(&app, initial_message.as_deref()) {
            Ok(w) => {
                if should_show {
                    let _ = w.show();
                    let _ = w.set_focus();
                    // 确保悬浮球在聊天窗口之上
                    ensure_ball_above_chat(&app);
                }
                // 如果 should_show = false，窗口创建后保持隐藏
            }
            Err(_) => {
            }
        }
    }
}

/// 隐藏聊天窗口
#[tauri::command]
fn hide_chat_window(app: tauri::AppHandle) {
    if let Some(chat_window) = app.webview_windows().get("chat") {
        let _ = chat_window.hide();
    }
}

/// 动态调整聊天窗口大小
/// 位置更新由 Resized 事件自动处理
#[tauri::command]
fn resize_chat_window(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let windows = app.webview_windows();
    let Some(chat_window) = windows.get("chat") else {
        return Err("Chat window not found".to_string());
    };

    // 尺寸限制（横屏 3:4 比例）
    const MIN_WIDTH: f64 = 400.0;
    const MAX_WIDTH: f64 = 600.0;
    const MIN_HEIGHT: f64 = 300.0;
    const MAX_HEIGHT: f64 = 450.0;

    let final_width = width.clamp(MIN_WIDTH, MAX_WIDTH);
    let final_height = height.clamp(MIN_HEIGHT, MAX_HEIGHT);

    // 设置大小后，Resized 事件会自动更新位置
    chat_window.set_size(tauri::Size::Logical(tauri::LogicalSize {
        width: final_width,
        height: final_height,
    })).map_err(|e| format!("Failed to set size: {:?}", e))?;
    Ok(())
}

/// 重置聊天窗口到默认大小
/// 位置更新由 Resized 事件自动处理
#[tauri::command]
fn reset_chat_window_size(app: tauri::AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();
    let Some(chat_window) = windows.get("chat") else {
        return Err("Chat window not found".to_string());
    };

    // 设置大小后，Resized 事件会自动更新位置
    chat_window.set_size(tauri::Size::Logical(tauri::LogicalSize {
        width: 320.0,
        height: 428.0,
    })).map_err(|e| format!("Failed to reset size: {:?}", e))?;
    Ok(())
}

/// 关闭并销毁聊天窗口
#[tauri::command]
fn close_chat_window(app: tauri::AppHandle) {
    if let Some(chat_window) = app.webview_windows().get("chat") {
        // 由于设置了关闭拦截，这里需要先移除拦截或者直接使用 destroy
        // 简单起见，直接隐藏
        let _ = chat_window.hide();
    }
}

/// 发送聊天消息并显示聊天窗口
#[tauri::command]
async fn send_chat_message(app: tauri::AppHandle, message: String) {
    // 显示聊天窗口并传入初始消息
    show_chat_window(app, Some(message), Some(true)).await;
}

/// 更新聊天窗口位置（当悬浮球移动时调用）
#[tauri::command]
fn update_chat_window_position(app: tauri::AppHandle) {
    if let (Some(chat_window), Some(main_window)) =
        (app.webview_windows().get("chat"), app.webview_windows().get("main")) {
        // 只有聊天窗口可见时才更新位置
        if let Ok(true) = chat_window.is_visible() {
            if let Ok(ball_pos) = main_window.outer_position() {
                if let Ok(ball_size) = main_window.outer_size() {
                    if let Ok(chat_size) = chat_window.outer_size() {
                        // 聊天窗口与悬浮球窗口居中对齐
                        // chat_x = ball_pos.x + ball_width/2 - chat_width/2
                        let ball_center = ball_pos.x + ball_size.width as i32 / 2;
                        let chat_x = ball_center - chat_size.width as i32 / 2;
                        let chat_y = ball_pos.y + ball_size.height as i32;
                        let _ = chat_window.set_position(Position::Physical(PhysicalPosition {
                            x: chat_x,
                            y: chat_y,
                        }));
                    }
                }
            }
        }
    }
}

/// 登录成功后的处理逻辑（从 on_login_success 抽取出来供 on_navigation 调用）
fn handle_login_success(app: &tauri::AppHandle) {
    // 更新登录状态
    IS_LOGGED_IN.store(true, Ordering::SeqCst);

    // 隐藏登录窗口
    if let Some(w) = app.webview_windows().get("login") {
        let _ = w.hide();
    } else {
    }

    // 更新托盘菜单为已登录状态
    rebuild_tray_menu(app, true, false);

    // 获取 main 窗口并显示
    if let Some(main_window) = app.webview_windows().get("main") {
        // 从 auth.json 读取登录信息并写入主窗口的 localStorage
        let js_inject = if let Some(data_dir) = dirs::data_local_dir() {
            let auth_file = data_dir.join("AIDI Desktop").join("auth.json");
            if let Ok(content) = std::fs::read_to_string(&auth_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let token = json["token"].as_str().unwrap_or("");
                    // user 字段可能是字符串或对象，统一序列化为 JSON 字符串再嵌入 JS
                    let user_json_str = if let Some(s) = json["user"].as_str() {
                        s.to_string()
                    } else {
                        json["user"].to_string()
                    };
                    // 将 JSON 字符串作为 JS 字符串字面量嵌入（带引号、转义）
                    let user_js_literal = serde_json::to_string(&user_json_str)
                        .unwrap_or_else(|_| "\"{}\"".to_string());
                    format!(
                        r#"(function() {{
                            try {{
                                localStorage.setItem('aidi-token', {token_lit});
                                localStorage.setItem('aidi-user', {user_lit});
                            }} catch(e) {{ console.error('[inject] localStorage error', e); }}
                            window.__aidiHandleLoginComplete && window.__aidiHandleLoginComplete();
                        }})();"#,
                        token_lit = serde_json::to_string(&serde_json::json!(token)).unwrap_or_else(|_| "\"\"".to_string()),
                        user_lit = user_js_literal,
                    )
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // 通过 Tauri 运行时显示窗口，确保在正确线程执行
        let app_clone = app.clone();
        let main_window_clone = main_window.clone();
        tauri::async_runtime::spawn(async move {
            let show_result = main_window_clone.show();
            eprintln!("[login_success] show() result: {:?}", show_result);
            // 登录成功后强制移回主屏幕中央，避免残留副屏坐标导致不可见
            eprintln!("[login_success] centering...");
            let _ = main_window_clone.center();
            BALL_VISIBLE.store(true, Ordering::SeqCst);
            // Windows 上 SetWindowRgn 在窗口隐藏后重新显示时可能失效，重新应用圆形遮罩
            let ball_size_val = *BALL_SIZE.lock().unwrap();
            let full_size = ball_size_val + BALL_PADDING * 2;
            eprintln!("[login_success] ball_size={}, full_size={}", ball_size_val, full_size);
            apply_circular_window_mask(&main_window_clone, full_size, "login_success");
            eprintln!("[login_success] is_visible={:?}", main_window_clone.is_visible());
            eprintln!("[login_success] outer_position={:?}", main_window_clone.outer_position());
            let _ = main_window_clone.set_always_on_top(true);
            let _ = main_window_clone.set_focus();
            if !js_inject.is_empty() {
                let _ = main_window_clone.eval(&js_inject);
            } else {
                let _ = main_window_clone.eval("window.__aidiHandleLoginComplete && window.__aidiHandleLoginComplete()");
            }
            // 额外通过事件通知 main 窗口登录完成（兜底，避免 eval 时机问题）
            let _ = main_window_clone.emit("login-complete", ());

            rebuild_tray_menu(&app_clone, true, true);
        });
    } else {
        eprintln!("[login_success] ERROR: main window not found!");
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
    let menu_height: i32 = 152;
    let menu_gap: i32 = 0;

    // 获取球的逻辑位置
    let Ok(ball_pos) = main_window.outer_position() else {
        return;
    };
    let ball_size = *BALL_SIZE.lock().unwrap();
    let visual_ball_size = (ball_size + BALL_PADDING * 2) as i32;
    let ball_x = (ball_pos.x as f64 / scale_factor) as i32;
    let ball_y = (ball_pos.y as f64 / scale_factor) as i32;
    // 胶囊收起态固定宽度：ball(60) + divider(1) + search(36) + border(2) = 99
    // 用常量避免 outer_size() 在展开/收起动画中返回错误值
    let pill_width: i32 = ball_size as i32 + 1 + 36 + 2;
    let main_window_right = ball_x + pill_width;

    // 计算水平方向：胶囊窗口（含搜索框）中心在屏幕右半则向左展开，否则向右展开
    let pill_center_x = ball_x + pill_width / 2;
    let opens_left = pill_center_x > screen_width / 2;

    let (menu_x, submenu_direction) = if opens_left {
        // 向左展开：菜单右边缘对齐主窗口（悬浮球+搜索框）右边缘
        (main_window_right - menu_width, "left")
    } else {
        // 向右展开：菜单左边缘对齐主窗口左边缘
        (ball_x, "right")
    };

    // 垂直方向：菜单在球下方（如果空间不够则上方）
    // 注意：ball_y 是窗口顶部，实际球体视觉顶部在 ball_y + BALL_PADDING
    let ball_visual_top = ball_y + BALL_PADDING as i32;
    let ball_visual_bottom = ball_visual_top + ball_size as i32;
    let space_below = screen_height - ball_visual_bottom;
    let show_above = space_below < menu_height + menu_gap;
    let menu_y = if show_above {
        ball_visual_top - menu_height - menu_gap
    } else {
        ball_visual_bottom + menu_gap
    };
    // 存入 DOCK_STATE
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.menu_window_x = menu_x;
        state.menu_window_y = menu_y;
        state.submenu_opens_left = opens_left;
    }

    // 判断窗口是否已存在（在当前线程做，避免进入 run_on_main_thread 再判断）
    let menu_exists = app.webview_windows().contains_key("menu");

    if menu_exists {
        // 复用路径：仍用 run_on_main_thread（只做 navigate/set_size，不涉及 WebView2 初始化，无死锁风险）
        let app_for_main = app.clone();
        let direction_owned = submenu_direction.to_string();
        let _ = app.run_on_main_thread(move || {
            let new_url = tauri::Url::parse(&build_menu_url(&app_for_main, &direction_owned)).unwrap();
            if let Some(existing) = app_for_main.webview_windows().get("menu") {
                let _ = existing.hide();
                let _ = existing.set_size(Size::Logical(tauri::LogicalSize {
                    width: menu_width as f64,
                    height: menu_height as f64,
                }));
                let _ = existing.set_position(Position::Logical(LogicalPosition {
                    x: menu_x as f64,
                    y: menu_y as f64,
                }));
                let _ = existing.navigate(new_url);
                // Windows 上复用窗口 navigate 也可能触发系统恢复 WS_CAPTION，延迟修复 main 窗口遮罩
                // 菜单显示由前端 menu_ready 命令触发，不在此处 show()
                #[cfg(target_os = "windows")]
                {
                    let app2 = app_for_main.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                        if let Some(main_w) = app2.webview_windows().get("main") {
                            let ball_size_val = *BALL_SIZE.lock().unwrap();
                            let full_size = ball_size_val + BALL_PADDING * 2;
                            apply_circular_window_mask(&main_w, full_size, "menu_reuse");
                        }
                        schedule_refresh_ball_mask(&app2);
                    });
                }
            }
        });
    } else {
        // 创建路径：用 spawn_blocking，主线程在 build() 期间保持空闲
        // Windows 上 WebView2 的 CreateCoreWebView2Controller 需要主线程消息泵响应
        // 若在 run_on_main_thread 中调用 build()，主线程忙于闭包无法处理消息，导致死锁
        // 改为 spawn_blocking 后，主线程空闲，与 create_login_window 模式完全一致
        let app_clone = app.clone();
        let direction_clone = submenu_direction.to_string();
        tauri::async_runtime::spawn(async move {
            let app2 = app_clone.clone();
            let dir2 = direction_clone.clone();
            let result = tokio::task::spawn_blocking(move || {
                create_menu_window(&app2, &dir2)
            }).await;

            match result {
                Ok(Ok(w)) => {
                    // 从 DOCK_STATE 读取位置（spawn_blocking 完成后可能与 show_menu 入参一致）
                    let (mx, my) = {
                        let s = DOCK_STATE.lock().unwrap();
                        (s.menu_window_x, s.menu_window_y)
                    };
                    let _ = w.set_position(Position::Logical(LogicalPosition {
                        x: mx as f64,
                        y: my as f64,
                    }));
                    // 菜单显示由前端 menu_ready 命令触发，不在此处 show()
                    // Windows 上新建 WebView2 窗口会导致系统恢复 WS_CAPTION，延迟修复 main 窗口遮罩
                    #[cfg(target_os = "windows")]
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                        if let Some(main_w) = app_clone.webview_windows().get("main") {
                            let ball_size_val = *BALL_SIZE.lock().unwrap();
                            let full_size = ball_size_val + BALL_PADDING * 2;
                            apply_circular_window_mask(&main_w, full_size, "after_menu_create");
                        }
                        schedule_refresh_ball_mask(&app_clone);
                    }
                }
                Ok(Err(_)) => {}
                Err(_) => {}
            }
        });
    }
}

#[tauri::command]
fn menu_ready(app: tauri::AppHandle) {
    // Vue 组件准备好后，显示菜单窗口
    if let Some(menu_window) = app.webview_windows().get("menu") {
        let _ = menu_window.show();
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
            height: 152.0,
        }));
        let _ = menu_window.hide();
    }
}

/// show_submenu / hide_submenu：前端 Menu.vue 调用的别名命令
#[tauri::command]
fn show_submenu(app: tauri::AppHandle) {
    menu_expand(app);
}

#[tauri::command]
fn hide_submenu(app: tauri::AppHandle) {
    menu_collapse(app);
}

#[tauri::command]
fn menu_expand(app: tauri::AppHandle) {
    if let Some(w) = app.webview_windows().get("menu") {
        let (init_x, init_y, opens_left) = {
            let s = DOCK_STATE.lock().unwrap();
            (s.menu_window_x, s.menu_window_y, s.submenu_opens_left)
        };
        if opens_left {
            // 向左展开：窗口 x 左移236（子菜单宽度），宽度扩至428
            let new_x = init_x - 236;
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
            height: 152.0,
        }));
    }
}

/// 静默刷新菜单窗口的锁，防止重入
static MENU_REPAINT_LOCK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 静默刷新菜单窗口（解决 Tauri 透明窗口渲染 bug）
/// 通过 hide → navigate 2次 → show 实现，使用原子锁防止重入
#[tauri::command]
fn menu_force_repaint(app: tauri::AppHandle, submenu_expanded: bool) {
    // 使用 compare_exchange 防止重入
    if MENU_REPAINT_LOCK.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
    ).is_err() {
        // 已经有操作在进行，直接返回
        return;
    }

    // 获取 direction 参数
    let direction = {
        let state = DOCK_STATE.lock().unwrap();
        if state.submenu_opens_left { "left" } else { "right" }
    };
    // 构建带子菜单状态的 URL
    let base_url = build_menu_url(&app, direction);
    let url = if submenu_expanded {
        format!("{}&submenu=1", base_url)
    } else {
        base_url
    };

    let app_clone = app.clone();
    let url_clone = url.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(w) = app_clone.webview_windows().get("menu") {
            // 只 hide 一次
            let _ = w.hide();
            // 第一次 navigate
            if let Ok(parsed_url) = tauri::Url::parse(&url_clone) {
                let _ = w.navigate(parsed_url);
            }
            // 短暂延迟
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            // 第二次 navigate
            if let Ok(parsed_url) = tauri::Url::parse(&url_clone) {
                let _ = w.navigate(parsed_url);
            }
            // 等待页面加载后显示
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = w.show();
        }
        // 释放锁
        MENU_REPAINT_LOCK.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

// ==================== SETTINGS ====================

#[tauri::command]
fn update_settings(app: tauri::AppHandle, settings: Settings) {
    // Update ball size
    {
        let mut ball_size = BALL_SIZE.lock().unwrap();
        *ball_size = settings.ball_size;
    }
    // 保存 theme_mode 供 build_menu_url 使用
    {
        let mut theme_mode = THEME_MODE.lock().unwrap();
        *theme_mode = settings.theme_mode.clone();
    }

    let _ = app.emit("settings-updated", settings);
}

/// 设置上报用户信息（供前端调用）
#[tauri::command]
fn set_report_user_info(user_code: String, user_name: String) {
    report_worker::set_user_info(user_code, user_name);
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

        // 窗口尺寸计算：
        // - 宽度 = ballSize(悬浮球) + 分割线(1px) + 36(搜索按钮) + border*2(2px)，精确匹配内容宽度
        // - 高度 = ballSize + padding * 2
        let divider_width = 1u32;
        let search_width = 36u32;
        let border = 1u32;
        let window_width = actual_size + divider_width + search_width + border * 2;
        let window_height = actual_size + BALL_PADDING * 2;

        // 获取当前位置和旧尺寸
        let current_pos = main_window.outer_position().ok();
        let old_size = main_window.outer_size().ok();

        if let (Some(pos), Some(old)) = (current_pos, old_size) {
            // 计算新的窗口位置，保持视觉中心不变
            let new_x = pos.x - ((old.width as u32 - window_width) / 2) as i32;
            let new_y = pos.y - ((old.height as u32 - window_height) / 2) as i32;

            // 先设置位置，再设置尺寸
            let _ = main_window.set_position(Position::Physical(PhysicalPosition { x: new_x, y: new_y }));
        }

        // 使用 LogicalSize 以正确支持高 DPI 屏幕
        let _ = main_window.set_size(Size::Logical(tauri::LogicalSize {
            width: window_width as f64,
            height: window_height as f64,
        }));

        // 注意：不再使用圆形遮罩，因为窗口现在是矩形的
        // 圆形效果由前端 CSS 实现

        // 同步更新内部状态
        let mut ball_size = BALL_SIZE.lock().unwrap();
        *ball_size = actual_size;
    }
}

// 展开输入框：窗口宽度扩至 ballSize + 分割线 + 36(搜索按钮) + inputWidth + border*2，精确匹配内容宽度
#[tauri::command]
fn expand_input_window(app: tauri::AppHandle) {
    if let Some(main_window) = app.webview_windows().get("main") {
        let ball_size = *BALL_SIZE.lock().unwrap();
        // 展开态宽度：浮动球 60 + 分割线 1 + 搜索按钮 36 + 输入框 240 + 边框 2 = 339px
        // 但实际前端 pill-shell 宽度为 303px，使用固定值以保持一致
        let window_width = 303u32;
        let window_height = ball_size + BALL_PADDING * 2;
        let _ = main_window.set_size(Size::Logical(tauri::LogicalSize {
            width: window_width as f64,
            height: window_height as f64,
        }));

        // 窗口大小改变后，更新聊天窗口位置
        update_chat_window_position(app.clone());
    }
}

// 收起输入框：恢复收起态宽度，精确匹配内容宽度
#[tauri::command]
fn collapse_input_window(app: tauri::AppHandle) {
    if let Some(main_window) = app.webview_windows().get("main") {
        let ball_size = *BALL_SIZE.lock().unwrap();
        // 收起态宽度：浮动球 60 + 分割线 1 + 搜索按钮 36 + 边框 2 = 99px
        // 硬编码以避免 ball_size 获取不正确导致的问题
        let window_width = 99u32;
        let window_height = ball_size + BALL_PADDING * 2;
        let _ = main_window.set_size(Size::Logical(tauri::LogicalSize {
            width: window_width as f64,
            height: window_height as f64,
        }));

        // 窗口大小改变后，更新聊天窗口位置
        update_chat_window_position(app.clone());
    }
}

// 调整输入框窗口高度（用于长文本输入时自动扩展）
#[tauri::command]
fn resize_input_window_height(app: tauri::AppHandle, height: u32) {
    if let Some(main_window) = app.webview_windows().get("main") {
        let window_height = height + BALL_PADDING * 2;
        let _ = main_window.set_size(Size::Logical(tauri::LogicalSize {
            width: 303f64,
            height: window_height as f64,
        }));

        // 窗口大小改变后，更新聊天窗口位置
        update_chat_window_position(app.clone());
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

    let stdout_raw = output.stdout.clone();
    let stdout_lossy = String::from_utf8_lossy(&stdout_raw).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !stderr.is_empty() {
    }

    if !output.status.success() {
        return Err(format!("Script failed: {}", stderr));
    }

    // 去除 UTF-8 BOM（PowerShell 有时在输出中也带 BOM）
    let stdout = stdout_lossy.trim_start_matches('\u{FEFF}').trim().to_string();

    serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON: {} - Output: {}", e, stdout))
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
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &script_path_str])
        .env("SCRIPT_ARGS", args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to execute script: {}", e))?;

    let stdout_raw = output.stdout.clone();
    let stdout_lossy = String::from_utf8_lossy(&stdout_raw).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !stderr.is_empty() {
    }

    if !output.status.success() {
        return Err(format!("Script failed: {}", stderr));
    }

    // 去除 UTF-8 BOM
    let stdout = stdout_lossy.trim_start_matches('\u{FEFF}').trim().to_string();

    serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON: {} - Output: {}", e, stdout))
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
    // 先隐藏所有非 login 窗口
    let windows = app.webview_windows();
    for (label, w) in &windows {
        if label != "login" {
            let _ = w.hide();
        }
    }

    // 检查登录窗口是否已存在
    if let Some(w) = app.webview_windows().get("login") {
        // 重新导航到远程登录页
        let login_url = build_login_url(&app);
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
        let app_clone = app.clone();
        tokio::task::spawn_blocking(move || {
            match create_login_window(&app_clone) {
                Ok(w) => {
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
                Err(_) => {}
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
    IS_LOGGED_IN.store(is_logged_in, Ordering::SeqCst);

    // 直接读 BALL_VISIBLE，避免 is_visible() 与 BALL_VISIBLE 状态不一致
    let ball_visible = BALL_VISIBLE.load(Ordering::SeqCst);

    rebuild_tray_menu(&app, is_logged_in, ball_visible);
}

/// 登录成功后由 login 窗口调用
/// token/user 可选：Windows 注入脚本带参传入并直接保存，macOS 不传则读 auth.json
#[tauri::command]
async fn on_login_success(app: tauri::AppHandle, token: Option<String>, user: Option<String>) {
    // 带参时直接保存 auth.json
    if let (Some(token_val), Some(user_val)) = (&token, &user) {
        if let Some(data_dir) = dirs::data_local_dir() {
            let aidi_dir = data_dir.join("AIDI Desktop");
            let _ = std::fs::create_dir_all(&aidi_dir);
            let user_json: serde_json::Value = serde_json::from_str(user_val).unwrap_or(serde_json::Value::Null);
            let content = serde_json::json!({
                "token": token_val,
                "userId": user_json["id"].as_str().unwrap_or(""),
                "userName": user_json["name"].as_str().unwrap_or(""),
                "user": user_val,
                "updatedAt": chrono::Local::now().to_rfc3339(),
            });
            let _ = std::fs::write(aidi_dir.join("auth.json"), content.to_string());
        }
    }
    handle_login_success(&app);
}

/// 保存登录信息到本地文件
/// 由 WebView 内的登录页面调用
#[tauri::command]
fn save_login_info(token: String, user_id: String, user_name: String, user_json: String) -> Result<(), String> {
    // 保存到本地文件
    if let Some(data_dir) = dirs::data_local_dir() {
        let aidi_dir = data_dir.join("AIDI Desktop");
        if let Err(e) = std::fs::create_dir_all(&aidi_dir) {
            return Err(format!("创建目录失败: {}", e));
        }

        // 解析 user_json 字符串为 JSON 对象，避免双重转义
        let user_value: serde_json::Value = serde_json::from_str(&user_json)
            .unwrap_or_else(|_| serde_json::json!(user_json));

        let auth_file = aidi_dir.join("auth.json");
        let content = serde_json::json!({
            "token": token,
            "userId": user_id,
            "userName": user_name,
            "user": user_value,
            "updatedAt": chrono::Local::now().to_rfc3339(),
        });

        if let Err(e) = std::fs::write(&auth_file, content.to_string()) {
            return Err(format!("写入文件失败: {}", e));
        }
    } else {
        return Err("无法获取本地数据目录".to_string());
    }

    Ok(())
}

/// 读取本地登录信息
#[tauri::command]
fn get_login_info() -> Result<Option<serde_json::Value>, String> {
    if let Some(data_dir) = dirs::data_local_dir() {
        let auth_file = data_dir.join("AIDI Desktop").join("auth.json");
        if auth_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&auth_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    return Ok(Some(json));
                }
            }
        }
    }
    Ok(None)
}

/// 前端调试日志（空实现，保留接口兼容）
#[tauri::command]
fn log_debug(_message: String) {
    // 生产环境不输出日志
}

/// 清除登录状态（删除 auth.json + 重置全局状态）
#[tauri::command]
fn clear_login_state(app: tauri::AppHandle) -> Result<(), String> {
    // 1. 删除 auth.json 文件
    if let Some(data_dir) = dirs::data_local_dir() {
        let auth_file = data_dir.join("AIDI Desktop").join("auth.json");
        if auth_file.exists() {
            if let Err(e) = std::fs::remove_file(&auth_file) {
                return Err(format!("删除登录文件失败: {}", e));
            }
        }
    }

    // 2. 重置全局登录状态
    IS_LOGGED_IN.store(false, Ordering::SeqCst);

    // 3. 清除上报线程的用户信息
    report_worker::set_user_info(String::new(), String::new());

    // 4. 更新托盘菜单
    rebuild_tray_menu(&app, false, false);

    Ok(())
}

// ==================== MAIN ENTRY POINT ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 加载 .env 文件（按优先级：.env.{AIDI_ENV} > .env）
    let env_mode = std::env::var("AIDI_ENV").unwrap_or_else(|_| "test".to_string());
    let env_file = format!(".env.{}", env_mode);
    // 先尝试加载 .env.{mode}，失败则加载 .env
    if dotenv::from_filename(&env_file).is_err() {
        let _ = dotenv::dotenv();
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                // 监听 deep link 事件
                let app_handle = app.handle().clone();
                use tauri_plugin_deep_link::DeepLinkExt;
                app.deep_link().on_open_url(move |event| {
                    let urls = event.urls();
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
                // 创建菜单栏 tray icon
                // 使用 PNG 格式（Tauri 不支持 ICO 格式）
                // Windows 使用 32x32 小图标，macOS 使用 tray-icon.png
                #[cfg(target_os = "windows")]
                let tray_icon_bytes = include_bytes!("../icons/32x32.png");
                #[cfg(not(target_os = "windows"))]
                let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");
                // 将托盘创建包装在独立块中，避免错误中断 setup
                let tray_result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let icon = tauri::image::Image::from_bytes(tray_icon_bytes)?;
                    // 初始状态默认为未登录，菜单显示"登录"和"退出"
                    let login_item = MenuItem::with_id(app, "login", "登录", true, None::<&str>)?;
                    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                    let menu = Menu::with_items(app, &[&login_item, &quit_item])?;
                    let _tray = TrayIconBuilder::with_id("main-tray")
                        .icon(icon)
                        .tooltip("AIDI Desktop")
                        .menu(&menu)
                        .show_menu_on_left_click(true)
                        .on_tray_icon_event(|_tray, _event| {
                        })
                        .on_menu_event(|tray, event| {
                            // Tauri v2 中 TrayIconBuilder::on_menu_event 会拦截托盘菜单事件，
                            // 全局 app.on_menu_event 收不到，因此在这里处理所有托盘菜单事件
                            let app = tray.app_handle().clone();
                            match event.id.as_ref() {
                                "quit" => {
                                    app.exit(0);
                                }
                                "login" => {
                                    if let Some(w) = app.webview_windows().get("login") {
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
                                        match create_login_window(&app) {
                                            Ok(w) => {
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
                                            Err(_) => {
                                            }
                                        }
                                    }
                                }
                                "toggle" => {
                                    let visible = BALL_VISIBLE.load(Ordering::SeqCst);
                                    if let Some(w) = app.webview_windows().get("main") {
                                        if visible {
                                            let _ = w.hide();
                                            // 隐藏其他所有打开的窗口
                                            let windows = app.webview_windows();
                                            for (label, win) in &windows {
                                                if label != "main" {
                                                    let _ = win.hide();
                                                }
                                            }
                                            BALL_VISIBLE.store(false, Ordering::SeqCst);
                                            sync_toggle_menu_item(&app, false);
                                        } else {
                                            let _ = w.show();
                                            BALL_VISIBLE.store(true, Ordering::SeqCst);
                                            sync_toggle_menu_item(&app, true);
                                            // 重新应用圆形遮罩，防止 WS_CAPTION 热区重现
                                            let ball_size_val = *BALL_SIZE.lock().unwrap();
                                            let full_size = ball_size_val + BALL_PADDING * 2;
                                            apply_circular_window_mask(w, full_size, "tray_init");
                                        }
                                    } else {
                                    }
                                }
                                "aigc" => {
                                    let _ = app.emit_to("main", "open-aigc", ());
                                }
                                _ => {}
                            }
                        })
                        .build(app)?;
                    Ok(())
                })();

                if let Err(_) = tray_result {
                }

                // 全局菜单事件监听（菜单重建后依然有效）
                    app.on_menu_event(|app, event| match event.id.as_ref() {
                        "login" => {
                            // 显示登录窗口
                            if let Some(w) = app.webview_windows().get("login") {
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
                                // 窗口不存在，动态创建
                                match create_login_window(app) {
                                    Ok(w) => {
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
                                    Err(_) => {
                                    }
                                }
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    });

                // 提前从 auth.json 读取登录状态
                // 使 IS_LOGGED_IN 在前端 WebView 加载之前就已正确设置
                {
                    let has_valid_token = dirs::data_local_dir()
                        .map(|d| d.join("AIDI Desktop").join("auth.json"))
                        .and_then(|p| std::fs::read_to_string(p).ok())
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                        .map(|v| !v["token"].as_str().unwrap_or("").is_empty())
                        .unwrap_or(false);

                    if has_valid_token {
                        IS_LOGGED_IN.store(true, Ordering::SeqCst);
                        rebuild_tray_menu(&app.handle(), true, false);
                    } else {
                    }
                }

                // Position main window at center
                if let Some(window) = app.webview_windows().get("main") {
                    // 禁用窗口阴影，避免灰色边框
                    #[cfg(any(target_os = "macos", target_os = "windows"))]
                    {
                        let _ = window.set_shadow(false);
                    }
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
                        // 在正确尺寸和位置设置后应用圆形遮罩（必须在 set_size 之后）
                        apply_circular_window_mask(&window, size, "init_position");
                    }
                    // show 触发 webview 初始化（App.vue 开始执行），
                    // 随即 hide 避免浮动球提前显示；
                    // App.vue 内部根据 token 再决定显示浮动球或登录窗口
                    let _ = window.show();
                    let _ = window.hide();

                    // Windows 专用：监听窗口失去焦点事件，自动刷新圆形遮罩
                    // 解决：点击其他应用后悬浮球出现灰色背景的问题
                    #[cfg(target_os = "windows")]
                    {
                        let app_handle = app.handle().clone();
                        let main_win = window.clone();
                        let _ = main_win.on_window_event(move |event| {
                            if let tauri::WindowEvent::Focused(false) = event {
                                let app_clone = app_handle.clone();
                                // 延迟刷新，等待 Windows DWM 完成状态更新
                                std::thread::spawn(move || {
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                    if let Some(w) = app_clone.webview_windows().get("main") {
                                        let ball_size_val = *BALL_SIZE.lock().unwrap();
                                        let full_size = ball_size_val + BALL_PADDING * 2;
                                        apply_circular_window_mask(&w, full_size, "on_blur");
                                    }
                                });
                            }
                        });
                    }
                }

            }
            // 注册全局快捷键：Cmd+D (macOS) / Ctrl+D (Windows)
            // 使用 Carbon RegisterEventHotKey，不触发 Apple Music 媒体库权限弹窗
            let app_handle_hotkey = app.handle().clone();
            register_hotkey_cmd_d(app_handle_hotkey);

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
            } else {
            }

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
            menu_force_repaint,
            show_submenu,
            hide_submenu,
            update_settings,
            set_report_user_info,
            trigger_report,
            update_window_size,
            expand_input_window,
            collapse_input_window,
            resize_input_window_height,
            start_drag,
            move_window_by,
            move_window_to,
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
            get_login_info,
            clear_login_state,
            diagnose_window,
            show_chat_window,
            hide_chat_window,
            close_chat_window,
            send_chat_message,
            update_chat_window_position,
            resize_chat_window,
            reset_chat_window_size,
            feishu::auth::feishu_login,
            feishu::bitable::feishu_report_device,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // RunEvent::Reopen 仅在 macOS 存在（Dock 图标点击事件）
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows, .. } = event {
                if !has_visible_windows {
                    let is_logged_in = IS_LOGGED_IN.load(Ordering::SeqCst);
                    if is_logged_in {
                        if let Some(main_window) = app.webview_windows().get("main") {
                            let _ = main_window.show();
                            BALL_VISIBLE.store(true, Ordering::SeqCst);
                            let ball_size_val = *BALL_SIZE.lock().unwrap();
                            let full_size = ball_size_val + BALL_PADDING * 2;
                            apply_circular_window_mask(&main_window, full_size, "reopen");
                            rebuild_tray_menu(app, true, true);
                        }
                    } else {
                        if let Some(w) = app.webview_windows().get("login") {
                            let login_url = build_login_url(app);
                            let _ = w.navigate(tauri::Url::parse(&login_url).unwrap());
                            let _ = w.center();
                            let _ = w.show();
                            let _ = w.set_focus();
                        } else {
                            let app_clone = app.clone();
                            tauri::async_runtime::spawn(async move {
                                match create_login_window(&app_clone) {
                                    Ok(w) => {
                                        let _ = w.center();
                                        let _ = w.show();
                                        let _ = w.set_focus();
                                    }
                                    Err(_) => {
                                    }
                                }
                            });
                        }
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
