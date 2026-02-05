// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager, PhysicalPosition, LogicalPosition, Position, Size};

// ==================== DATA STRUCTURES ====================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub ball_size: u32,
    pub opacity: u32,
    pub color_theme: String,
    pub theme_mode: String,
}

// ==================== POSITION DETECTION SYSTEM ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BallPosition {
    LeftTop,     // 屏幕左上1/3区域
    LeftMiddle,  // 屏幕左中1/3区域
    LeftBottom,  // 屏幕左下1/3区域
    RightTop,    // 屏幕右上1/3区域
    RightMiddle, // 屏幕右中1/3区域
    RightBottom, // 屏幕右下1/3区域
    TopCenter,   // 屏幕上中1/3区域
    BottomCenter,// 屏幕下中1/3区域
    Center,      // 屏幕中心区域
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuPosition {
    Below,  // 菜单在球下方
    Above,  // 菜单在球上方
    Left,   // 菜单在球左侧
    Right,  // 菜单在球右侧
}

impl Default for MenuPosition {
    fn default() -> Self {
        MenuPosition::Below
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // 为未来功能扩展预留
enum MenuAlignment {
    LeftAlign,   // 左对齐
    RightAlign,  // 右对齐
    CenterAlign, // 居中对齐
    TopAlign,    // 上对齐
    BottomAlign, // 下对齐
}

impl Default for MenuAlignment {
    fn default() -> Self {
        MenuAlignment::LeftAlign
    }
}

// ==================== INTERACTION STATE MACHINE ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]  // Dragging variant reserved for future use
enum InteractionState {
    Idle,           // 空闲
    Hovering,       // 悬浮球 hover
    MenuShowing,    // 菜单显示中
    SubmenuShowing, // 子菜单显示中
    HideDelaying,   // 等待隐藏
    Dragging,       // 拖拽中
    Animating,      // 动画中
}

impl Default for InteractionState {
    fn default() -> Self {
        InteractionState::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuDirection {
    Right,   // 菜单向右展开（球在左边）
    Left,    // 菜单向左展开（球在右边）
    Bottom,  // 菜单向下展开（球在上边）
    Top,     // 菜单向上展开（球在下边）
}

impl Default for MenuDirection {
    fn default() -> Self {
        MenuDirection::Right
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmenuDirection {
    Right,  // 子菜单向右展开
    Left,   // 子菜单向左展开
}

impl Default for SubmenuDirection {
    fn default() -> Self {
        SubmenuDirection::Right
    }
}

// ==================== DOCK STATE ====================

#[derive(Debug, Clone, Default)]
struct DockState {
    is_docked: bool,
    dock_edge: Option<String>, // "left", "right", "top", "bottom"
    is_popped_out: bool,
    menu_dir: MenuDirection,         // Menu expansion direction (4 directions)
    submenu_dir: SubmenuDirection,   // Submenu expansion direction (left or right)
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
    submenu_hover: bool,
    // 新增：智能定位系统状态
    ball_position: Option<BallPosition>,    // 悬浮球位置分类
    menu_position: MenuPosition,            // 菜单位置策略
    menu_alignment: MenuAlignment,          // 菜单对齐方式
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
    menu_dir: MenuDirection::Right,
    submenu_dir: SubmenuDirection::Right,
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
    submenu_hover: false,
    ball_position: None,
    menu_position: MenuPosition::Below,
    menu_alignment: MenuAlignment::LeftAlign,
});

// 定时器句柄（使用 Arc<Mutex<Option<...>>> 存储跨线程可访问的句柄）
static HIDE_DOCK_TIMER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
static POP_PROTECTION_TIMER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

static BALL_SIZE: Mutex<u32> = Mutex::new(48);
const BALL_PADDING: u32 = 16; // 外环需要 ballSize + 8，所以需要额外 8 + 边距
const EDGE_THRESHOLD: i32 = 20;  // Edge detection threshold
const DOCK_VISIBLE_RATIO: f32 = 0.5;  // 50% of ball visible when docked

// Animation constants
const ANIMATION_FRAMES: u32 = 12;
#[cfg(target_os = "windows")]
const ANIMATION_FRAME_MS: u64 = 33;  // ~30fps on Windows
#[cfg(not(target_os = "windows"))]
const ANIMATION_FRAME_MS: u64 = 16;  // ~60fps on other platforms

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

/// 检测悬浮球在屏幕上的位置分类
/// 将屏幕按1/3划分为9个区域进行精确检测
fn detect_ball_position(
    ball_x: i32,
    ball_y: i32,
    ball_width: i32,
    ball_height: i32,
    screen_width: i32,
    screen_height: i32,
) -> BallPosition {
    // 计算悬浮球中心点位置
    let center_x = ball_x + ball_width / 2;
    let center_y = ball_y + ball_height / 2;

    // 边界检查：确保参数合法
    if screen_width <= 0 || screen_height <= 0 {
        return BallPosition::Center;
    }


    // 水平位置判断 - 调整阈值使边缘检测更敏感
    let horizontal_zone = if center_x < screen_width / 4 {
        0 // 左侧 (左1/4)
    } else if center_x > screen_width * 3 / 4 {
        2 // 右侧 (右1/4)
    } else {
        1 // 中间
    };


    // 垂直位置判断（考虑macOS菜单栏）
    let effective_center_y = center_y - MENUBAR_HEIGHT;
    let effective_screen_height = screen_height - MENUBAR_HEIGHT;

    // 防止除零错误
    if effective_screen_height <= 0 {
        return BallPosition::Center;
    }

    let effective_third_height = effective_screen_height / 3;

    let vertical_zone = if effective_center_y < effective_third_height {
        0 // 上方
    } else if effective_center_y < effective_third_height * 2 {
        1 // 中间
    } else {
        2 // 下方
    };

    // 根据位置组合确定分类
    match (horizontal_zone, vertical_zone) {
        (0, 0) => BallPosition::LeftTop,
        (0, 1) => BallPosition::LeftMiddle,
        (0, 2) => BallPosition::LeftBottom,
        (1, 0) => BallPosition::TopCenter,
        (1, 1) => BallPosition::Center,
        (1, 2) => BallPosition::BottomCenter,
        (2, 0) => BallPosition::RightTop,
        (2, 1) => BallPosition::RightMiddle,
        (2, 2) => BallPosition::RightBottom,
        _ => BallPosition::Center, // 默认情况
    }
}

/// 根据子菜单空间需求计算最佳菜单定位策略（参考Electron版本逻辑）
/// 返回 (菜单位置, 菜单对齐方式, 子菜单方向)
fn calculate_menu_strategy(
    ball_x: i32,
    ball_y: i32,
    ball_width: i32,
    ball_height: i32,
    screen_width: i32,
    screen_height: i32,
    menu_height: i32,
    menu_gap: i32,
) -> (MenuPosition, MenuAlignment, SubmenuDirection) {
    let submenu_width = 250; // 子菜单宽度

    // 计算子菜单展示空间需求
    let right_available = screen_width - (ball_x + ball_width + menu_gap);
    let _left_available = ball_x + menu_gap;

    // 根据子菜单空间决定展示方向
    let submenu_direction = if right_available >= submenu_width {
        SubmenuDirection::Right
    } else {
        SubmenuDirection::Left
    };

    // 根据子菜单方向决定菜单对齐方式
    let menu_alignment = if submenu_direction == SubmenuDirection::Right {
        // 右边空间够，菜单左对齐球体（子菜单向右展开）
        MenuAlignment::LeftAlign
    } else {
        // 右边空间不够，菜单右对齐球体（子菜单向左展开）
        MenuAlignment::RightAlign
    };

    // 垂直方向：优先在球体下方显示，空间不足时向上
    let bottom_space = screen_height - (ball_y + ball_height + menu_gap);
    let top_space = ball_y - menu_gap;

    let menu_position = if bottom_space >= menu_height {
        MenuPosition::Below
    } else if top_space >= menu_height {
        MenuPosition::Above
    } else {
        MenuPosition::Below // 默认下方，即使空间不足
    };

    (menu_position, menu_alignment, submenu_direction)
}

/// 计算菜单的具体位置坐标
/// 根据菜单位置策略和对齐方式计算最终坐标
fn calculate_menu_position(
    ball_x: i32,
    ball_y: i32,
    ball_width: i32,
    ball_height: i32,
    menu_width: i32,
    menu_height: i32,
    screen_width: i32,
    screen_height: i32,
    position: MenuPosition,
    alignment: MenuAlignment,
    gap: i32,
) -> (i32, i32) {
    // 边界检查：确保参数合法
    if screen_width <= 0 || screen_height <= 0 || menu_width <= 0 || menu_height <= 0 {
        return (0, MENUBAR_HEIGHT); // 返回安全的默认位置
    }

    let mut menu_x = ball_x;
    let mut menu_y = ball_y;

    // 根据菜单位置计算基础位置
    match position {
        MenuPosition::Below => {
            menu_y = ball_y + ball_height + gap;
        },
        MenuPosition::Above => {
            menu_y = ball_y - menu_height - gap;
        },
        MenuPosition::Right => {
            menu_x = ball_x + ball_width + gap;
        },
        MenuPosition::Left => {
            menu_x = ball_x - menu_width - gap;
        },
    }

    // 根据对齐方式调整位置
    match alignment {
        MenuAlignment::LeftAlign => {
            // 菜单左边缘与球左边缘对齐 - 已经是默认行为
        },
        MenuAlignment::RightAlign => {
            // 菜单右边缘与球右边缘对齐
            if position == MenuPosition::Below || position == MenuPosition::Above {
                menu_x = ball_x + ball_width - menu_width;
            }
        },
        MenuAlignment::CenterAlign => {
            // 菜单中心与球中心对齐
            if position == MenuPosition::Below || position == MenuPosition::Above {
                menu_x = ball_x + (ball_width - menu_width) / 2;
            } else {
                menu_y = ball_y + (ball_height - menu_height) / 2;
            }
        },
        MenuAlignment::TopAlign => {
            // 菜单顶部与球顶部对齐
            if position == MenuPosition::Right || position == MenuPosition::Left {
                menu_y = ball_y;
            }
        },
        MenuAlignment::BottomAlign => {
            // 菜单底部与球底部对齐
            if position == MenuPosition::Right || position == MenuPosition::Left {
                menu_y = ball_y + ball_height - menu_height;
            }
        },
    }

    // 边界检查：确保菜单不超出屏幕边界
    // 对于右对齐，优先保持右对齐效果
    if alignment == MenuAlignment::RightAlign && (position == MenuPosition::Below || position == MenuPosition::Above) {
        // 右对齐时，确保菜单右边缘不超出屏幕
        if menu_x + menu_width > screen_width {
            menu_x = screen_width - menu_width;
        }
        // 确保菜单左边缘不超出屏幕
        menu_x = menu_x.max(0);
    } else {
        // 其他情况的常规边界检查
        let max_x = screen_width.saturating_sub(menu_width);
        menu_x = menu_x.max(0).min(max_x);
    }

    let max_y = screen_height.saturating_sub(menu_height);
    menu_y = menu_y.max(MENUBAR_HEIGHT).min(max_y);

    // 如果菜单仍然无法完全显示，启动降级策略
    if menu_x + menu_width > screen_width {
        menu_x = screen_width.saturating_sub(menu_width);
    }
    if menu_y + menu_height > screen_height {
        menu_y = screen_height.saturating_sub(menu_height);
    }

    (menu_x, menu_y)
}

/// 计算子菜单的具体位置坐标
/// 根据一级菜单位置和子菜单展开方向计算最终坐标
fn calculate_submenu_position(
    menu_x: i32,
    menu_y: i32,
    menu_width: i32,
    submenu_width: i32,
    submenu_height: i32,
    screen_width: i32,
    screen_height: i32,
    submenu_direction: SubmenuDirection,
    padding_overlap: i32,
) -> (i32, i32) {
    let (sub_x, sub_y) = match submenu_direction {
        SubmenuDirection::Right => {
            let x = menu_x + menu_width - padding_overlap;
            let y = menu_y;
            (x, y)
        },
        SubmenuDirection::Left => {
            let x = menu_x - submenu_width + padding_overlap;
            let y = menu_y;
            (x, y)
        },
    };

    // 确保子菜单在屏幕边界内
    let final_x = sub_x.max(0).min(screen_width - submenu_width);
    let final_y = sub_y.max(MENUBAR_HEIGHT).min(screen_height - submenu_height);

    (final_x, final_y)
}

// ==================== WINDOW MANAGEMENT ====================

#[tauri::command]
fn show_main_window(window: tauri::Window) {
    let _ = window.show();
}

#[tauri::command]
fn hide_main_window(window: tauri::Window) {
    let _ = window.hide();
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
fn show_submenu_window(app: tauri::AppHandle) {
    if let Some(submenu_window) = app.webview_windows().get("submenu") {
        let _ = submenu_window.show();
    }
}

#[tauri::command]
fn hide_submenu_window(app: tauri::AppHandle) {
    if let Some(submenu_window) = app.webview_windows().get("submenu") {
        let _ = submenu_window.hide();
    }
}

#[tauri::command]
fn show_optimizer_window(app: tauri::AppHandle) {
    if let Some(optimizer_window) = app.webview_windows().get("optimizer") {
        let _ = optimizer_window.show();
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
            (true, state.hidden_x, state.hidden_y, state.pop_out_x, state.pop_out_y)
        } else {
            (false, 0, 0, 0, 0)
        }
    };

    if should_pop {
        // Animate pop out
        let app_handle = app.clone();
        std::thread::spawn(move || {
            if let Some(main_window) = app_handle.webview_windows().get("main") {
                animate_to_position(&main_window, hidden_x, hidden_y, pop_out_x, pop_out_y, version);
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

                if state.is_docked && !state.ball_hover && !state.menu_hover && !state.submenu_hover {
                    (true, state.hidden_x, state.hidden_y, state.pop_out_x, state.pop_out_y)
                } else {
                    (false, 0, 0, 0, 0)
                }
            };

            if should_hide {
                let hide_version = next_state_version();
                if let Some(main_window) = app_handle.webview_windows().get("main") {
                    animate_to_position(&main_window, pop_x, pop_y, hidden_x, hidden_y, hide_version);
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
            // 如果鼠标不在球、菜单或子菜单上，则隐藏菜单
            !state.ball_hover && !state.menu_hover && !state.submenu_hover
        };

        if should_hide_menu {
            // 隐藏所有菜单窗口
            let windows = app_handle.webview_windows();
            if let Some(menu_window) = windows.get("menu") {
                let _ = menu_window.hide();
            }
            if let Some(submenu_window) = windows.get("submenu") {
                let _ = submenu_window.hide();
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
                if state.is_in_pop_protection || state.ball_hover || state.menu_hover || state.submenu_hover {
                    (false, 0, 0, 0, 0)
                } else if state.is_docked {
                    (true, state.hidden_x, state.hidden_y, state.pop_out_x, state.pop_out_y)
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

    // 延迟重置 menu_hover，给子菜单进入的时间
    let app_handle = app.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));

        // 检查是否在子菜单中
        let should_reset_menu_hover = {
            let state = DOCK_STATE.lock().unwrap();
            !state.submenu_hover
        };

        if should_reset_menu_hover {
            let mut state = DOCK_STATE.lock().unwrap();
            state.menu_hover = false;
        }

        // 再等待一段时间后检查是否需要隐藏
        std::thread::sleep(Duration::from_millis(MENU_HIDE_DELAY_MS - 50));

        let should_hide = {
            let state = DOCK_STATE.lock().unwrap();
            // 只有当没有任何 hover 状态时才隐藏
            !state.menu_hover && !state.submenu_hover && !state.ball_hover
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
                let _ = menu_window.hide();
            }
            if let Some(submenu_window) = windows.get("submenu") {
                let _ = submenu_window.hide();
            }
        }
    });

    let mut timer = MENU_HIDE_TIMER.lock().unwrap();
    *timer = Some(handle);
}

#[tauri::command]
fn submenu_enter(app: tauri::AppHandle) {
    let _ = app.emit("submenu-enter", ());

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
        state.submenu_hover = true;
        state.menu_hover = true; // Keep menu hover state
        state.interaction_state = InteractionState::SubmenuShowing;
    }
}

#[tauri::command]
fn submenu_leave(app: tauri::AppHandle) {
    let _ = app.emit("submenu-leave", ());

    let version = current_state_version();

    // 立即更新 submenu_hover 状态
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.submenu_hover = false;
    }

    // Delayed check to prevent flicker when moving back to menu
    let app_handle = app.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(MENU_HIDE_DELAY_MS));

        // Check if state version changed
        if current_state_version() != version {
            return;
        }

        let should_hide_all = {
            let state = DOCK_STATE.lock().unwrap();
            // Hide all menus if not hovering menu or ball
            !state.menu_hover && !state.ball_hover
        };

        {
            let mut state = DOCK_STATE.lock().unwrap();
            state.submenu_hover = false;
            if state.interaction_state == InteractionState::SubmenuShowing {
                state.interaction_state = InteractionState::MenuShowing;
            }
        }

        if should_hide_all {
            // Hide all menu windows
            let windows = app_handle.webview_windows();
            if let Some(menu_window) = windows.get("menu") {
                let _ = menu_window.hide();
            }
            if let Some(submenu_window) = windows.get("submenu") {
                let _ = submenu_window.hide();
            }

            let mut state = DOCK_STATE.lock().unwrap();
            state.menu_hover = false;
            state.interaction_state = InteractionState::HideDelaying;
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
    let Some(main_window) = windows.get("main") else { return };
    let Ok(pos) = main_window.outer_position() else { return };
    let Ok(size) = main_window.outer_size() else { return };

    // Get screen info
    let Some(monitor) = main_window.current_monitor().ok().flatten() else { return };
    let screen_size = monitor.size();
    let screen_width = screen_size.width as i32;
    let screen_height = screen_size.height as i32;

    let window_width = size.width as i32;
    let window_height = size.height as i32;

    // Calculate actual ball center position (considering BALL_PADDING)
    let ball_center_x = pos.x + window_width / 2;

    // Calculate ball size (window size minus padding)
    let ball_size = *BALL_SIZE.lock().unwrap() as i32;

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

        // Calculate how much to hide based on DOCK_VISIBLE_RATIO
        // We want DOCK_VISIBLE_RATIO of the ball to remain visible
        let visible_amount = (ball_size as f32 * DOCK_VISIBLE_RATIO) as i32;
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
                // 球在左边 → 菜单向右展开
                state.menu_dir = MenuDirection::Right;
                state.submenu_dir = SubmenuDirection::Right;
            }
            "right" => {
                // Hide to right, show DOCK_VISIBLE_RATIO of ball
                state.hidden_x = screen_width - window_width + hide_amount;
                state.hidden_y = clamped_y;
                state.pop_out_x = screen_width - window_width - pop_offset;
                state.pop_out_y = clamped_y;
                // 球在右边 → 菜单向左展开
                state.menu_dir = MenuDirection::Left;
                state.submenu_dir = SubmenuDirection::Left;
            }
            "top" => {
                // Hide to top, show DOCK_VISIBLE_RATIO of ball
                let top_hide_amount = window_height / 2 - visible_amount / 2;
                state.hidden_x = clamped_x;
                state.hidden_y = MENUBAR_HEIGHT - top_hide_amount;
                state.pop_out_x = clamped_x;
                state.pop_out_y = MENUBAR_HEIGHT + pop_offset;
                // 球在上边 → 菜单向下展开
                state.menu_dir = MenuDirection::Bottom;
                // 子菜单方向根据水平位置决定
                state.submenu_dir = if ball_center_x > screen_width / 2 {
                    SubmenuDirection::Left
                } else {
                    SubmenuDirection::Right
                };
            }
            "bottom" => {
                // Hide to bottom, show DOCK_VISIBLE_RATIO of ball
                let bottom_hide_amount = window_height / 2 - visible_amount / 2;
                state.hidden_x = clamped_x;
                state.hidden_y = screen_height - window_height + bottom_hide_amount;
                state.pop_out_x = clamped_x;
                state.pop_out_y = screen_height - window_height - pop_offset;
                // 球在下边 → 菜单向上展开
                state.menu_dir = MenuDirection::Top;
                // 子菜单方向根据水平位置决定
                state.submenu_dir = if ball_center_x > screen_width / 2 {
                    SubmenuDirection::Left
                } else {
                    SubmenuDirection::Right
                };
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
            animate_to_position(&main_window_clone, pos.x, pos.y, hidden_x, hidden_y, version);

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
        if state.ball_hover || state.menu_hover || state.submenu_hover || state.is_in_pop_protection {
            return;
        }
        (true, state.hidden_x, state.hidden_y, state.pop_out_x, state.pop_out_y)
    };

    if should_hide {
        let version = next_state_version();
        if let Some(main_window) = app.webview_windows().get("main") {
            // Use animation in a separate thread to avoid blocking
            let main_window_clone = main_window.clone();
            std::thread::spawn(move || {
                animate_to_position(&main_window_clone, pop_x, pop_y, hidden_x, hidden_y, version);

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
fn set_window_position(window: tauri::Window, x: i32, y: i32) {
    let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
}

#[tauri::command]
fn show_menu(app: tauri::AppHandle) {
    let windows = app.webview_windows();
    let Some(main_window) = windows.get("main") else { return };
    let Some(menu_window) = windows.get("menu") else { return };

    let Ok(ball_pos) = main_window.outer_position() else { return };
    let Ok(ball_size) = main_window.outer_size() else { return };
    let Some(monitor) = main_window.current_monitor().ok().flatten() else { return };

    let screen_size = monitor.size();
    let scale_factor = monitor.scale_factor();

    // Convert to logical pixels for all calculations
    let screen_width = (screen_size.width as f64 / scale_factor) as i32;
    let screen_height = (screen_size.height as f64 / scale_factor) as i32;

    // DEBUG: 打印屏幕和监视器信息
    println!("DEBUG: Physical monitor size: {}x{}", screen_size.width, screen_size.height);
    println!("DEBUG: Logical screen size: {}x{}", screen_width, screen_height);
    println!("DEBUG: Monitor scale factor: {}", scale_factor);
    let monitor_pos = monitor.position();
    println!("DEBUG: Monitor position: ({}, {})", monitor_pos.x, monitor_pos.y);

    // Menu dimensions in logical pixels (these are CSS pixels, already logical)
    let menu_width: i32 = 185;
    let menu_height: i32 = 110;
    let menu_gap: i32 = 0;

    // 1. 获取实际的悬浮球位置（如果吸附弹出了，用 pop_out 位置）
    let (ball_x_physical, ball_y_physical) = {
        let state = DOCK_STATE.lock().unwrap();
        if state.is_docked && state.is_popped_out {
            println!("DEBUG: Using pop_out position (physical): ({}, {})", state.pop_out_x, state.pop_out_y);
            (state.pop_out_x, state.pop_out_y)
        } else {
            println!("DEBUG: Using ball window position (physical): ({}, {})", ball_pos.x, ball_pos.y);
            (ball_pos.x, ball_pos.y)
        }
    };

    // Convert ball position to logical pixels
    let ball_x = (ball_x_physical as f64 / scale_factor) as i32;
    let ball_y = (ball_y_physical as f64 / scale_factor) as i32;
    let ball_full_width = (ball_size.width as f64 / scale_factor) as i32;
    let ball_full_height = (ball_size.height as f64 / scale_factor) as i32;

    println!("DEBUG: Ball logical position: ({}, {})", ball_x, ball_y);
    println!("DEBUG: Ball logical size: {}x{}", ball_full_width, ball_full_height);

    // 2. 检测悬浮球位置分类
    let ball_position = detect_ball_position(
        ball_x,
        ball_y,
        ball_full_width,
        ball_full_height,
        screen_width,
        screen_height,
    );

    // 3. 计算菜单定位策略（采用Electron版本的智能逻辑）
    let (menu_position, menu_alignment, submenu_direction) = calculate_menu_strategy(
        ball_x,
        ball_y,
        ball_full_width,
        ball_full_height,
        screen_width,
        screen_height,
        menu_height,
        menu_gap,
    );

    println!("DEBUG: Ball position classification: {:?}", ball_position);
    println!("DEBUG: Menu strategy - position: {:?}, alignment: {:?}, submenu: {:?}",
             menu_position, menu_alignment, submenu_direction);

    // 4. 计算菜单具体位置
    let (final_x, final_y) = calculate_menu_position(
        ball_x,
        ball_y,
        ball_full_width,
        ball_full_height,
        menu_width,
        menu_height,
        screen_width,
        screen_height,
        menu_position,
        menu_alignment,
        menu_gap,
    );

    println!("DEBUG: Calculated final position (logical pixels): ({}, {})", final_x, final_y);


    // 5. 保存策略信息到状态，供子菜单使用
    {
        let mut state = DOCK_STATE.lock().unwrap();
        state.ball_position = Some(ball_position);
        state.menu_position = menu_position;
        state.menu_alignment = menu_alignment;
        state.submenu_dir = submenu_direction;
    }

    // 6. 设置菜单位置并显示
    // 先隐藏窗口确保重置状态
    let _ = menu_window.hide();

    // 小延迟确保隐藏完成
    std::thread::sleep(std::time::Duration::from_millis(1));

    // 设置位置（使用逻辑像素坐标）
    let position_result = menu_window.set_position(Position::Logical(LogicalPosition { x: final_x as f64, y: final_y as f64 }));
    println!("DEBUG: set_position result: {:?}", position_result);

    // 再次小延迟确保位置设置生效
    std::thread::sleep(std::time::Duration::from_millis(5));

    // DEBUG: 验证窗口位置是否真的被设置了
    if let Ok(actual_pos) = menu_window.outer_position() {
        println!("DEBUG: Menu window position after set_position: ({}, {})", actual_pos.x, actual_pos.y);
        println!("DEBUG: Expected vs Actual: ({}, {}) vs ({}, {})", final_x, final_y, actual_pos.x, actual_pos.y);
    } else {
        println!("DEBUG: Failed to get window position");
    }

    // 显示窗口
    let show_result = menu_window.show();
    println!("DEBUG: show result: {:?}", show_result);

    // 最后再次验证位置
    if let Ok(final_pos) = menu_window.outer_position() {
        println!("DEBUG: Menu window position after show: ({}, {})", final_pos.x, final_pos.y);
    }
}

#[tauri::command]
fn hide_menu(app: tauri::AppHandle) {
    let windows = app.webview_windows();
    if let Some(menu_window) = windows.get("menu") {
        let _ = menu_window.hide();
    }
    if let Some(submenu_window) = windows.get("submenu") {
        let _ = submenu_window.hide();
    }
}

#[tauri::command]
fn show_submenu(app: tauri::AppHandle) {
    let windows = app.webview_windows();
    let Some(menu_window) = windows.get("menu") else { return };
    let Some(submenu_window) = windows.get("submenu") else { return };
    let Some(main_window) = windows.get("main") else { return };

    let Ok(menu_pos_physical) = menu_window.outer_position() else { return };
    let Ok(menu_size_physical) = menu_window.outer_size() else { return };
    let Some(monitor) = main_window.current_monitor().ok().flatten() else { return };

    let screen_size = monitor.size();
    let scale_factor = monitor.scale_factor();

    // Convert to logical pixels for all calculations
    let screen_width = (screen_size.width as f64 / scale_factor) as i32;
    let screen_height = (screen_size.height as f64 / scale_factor) as i32;

    // Convert menu position and size from physical to logical pixels
    let menu_pos_x = (menu_pos_physical.x as f64 / scale_factor) as i32;
    let menu_pos_y = (menu_pos_physical.y as f64 / scale_factor) as i32;
    let menu_width = (menu_size_physical.width as f64 / scale_factor) as i32;
    let menu_height = (menu_size_physical.height as f64 / scale_factor) as i32;

    // Submenu dimensions in logical pixels (CSS pixels)
    let submenu_width: i32 = 250;
    let submenu_height: i32 = 310;

    println!("DEBUG SUBMENU: Menu position (physical): ({}, {})", menu_pos_physical.x, menu_pos_physical.y);
    println!("DEBUG SUBMENU: Menu position (logical): ({}, {})", menu_pos_x, menu_pos_y);
    println!("DEBUG SUBMENU: Menu size (physical): {}x{}", menu_size_physical.width, menu_size_physical.height);
    println!("DEBUG SUBMENU: Menu size (logical): {}x{}", menu_width, menu_height);
    println!("DEBUG SUBMENU: Screen size (logical): {}x{}", screen_width, screen_height);

    // 1. 读取保存的策略信息
    let submenu_dir = {
        let state = DOCK_STATE.lock().unwrap();
        state.submenu_dir
    };

    println!("DEBUG SUBMENU: Submenu direction strategy: {:?}", submenu_dir);

    // 菜单和子菜单都有 4px 的 wrapper padding
    // 重叠 padding 实现无缝衔接
    let padding_overlap = 8;

    // 2. 使用精确的子菜单定位算法
    let (final_x, final_y) = calculate_submenu_position(
        menu_pos_x,
        menu_pos_y,
        menu_width,
        submenu_width,
        submenu_height,
        screen_width,
        screen_height,
        submenu_dir,
        padding_overlap,
    );

    println!("DEBUG SUBMENU: Calculated final position (logical): ({}, {})", final_x, final_y);

    // 3. 设置子菜单位置并显示（使用逻辑像素坐标）
    let position_result = submenu_window.set_position(Position::Logical(LogicalPosition { x: final_x as f64, y: final_y as f64 }));
    println!("DEBUG SUBMENU: set_position result: {:?}", position_result);

    let show_result = submenu_window.show();
    println!("DEBUG SUBMENU: show result: {:?}", show_result);

    // 验证最终位置
    if let Ok(actual_pos) = submenu_window.outer_position() {
        println!("DEBUG SUBMENU: Actual window position: ({}, {})", actual_pos.x, actual_pos.y);
        println!("DEBUG SUBMENU: Expected vs Actual: ({}, {}) vs ({}, {})", final_x, final_y, actual_pos.x, actual_pos.y);
    }
}

#[tauri::command]
fn hide_submenu(app: tauri::AppHandle) {
    let windows = app.webview_windows();
    if let Some(submenu_window) = windows.get("submenu") {
        let _ = submenu_window.hide();
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

#[tauri::command]
fn update_window_size(app: tauri::AppHandle, size: u32) {
    if let Some(main_window) = app.webview_windows().get("main") {
        // 确保最小尺寸，外环需要 ballSize + 8，再加两边 padding
        let actual_size = size.max(30);
        let full_size = actual_size + BALL_PADDING * 2;
        // 使用 LogicalSize 以正确支持高 DPI 屏幕
        let _ = main_window.set_size(Size::Logical(tauri::LogicalSize {
            width: full_size as f64,
            height: full_size as f64,
        }));

        // 同步更新内部状态
        let mut ball_size = BALL_SIZE.lock().unwrap();
        *ball_size = actual_size;
    }
}

// ==================== OPTIMIZER COMMANDS (TODO) ====================

#[tauri::command]
fn optimizer_scan_all(_app: tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}

#[tauri::command]
fn optimizer_disk_scan(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "dimension": "disk",
        "status": "success",
        "summary": "TODO",
        "details": null
    }))
}

#[tauri::command]
fn optimizer_disk_health(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "dimension": "health",
        "status": "success",
        "summary": "TODO",
        "details": null
    }))
}

#[tauri::command]
fn optimizer_disk_clean(_app: tauri::AppHandle, _categories_json: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "cleaned": 0,
        "errors": 0,
        "details": []
    }))
}

#[tauri::command]
fn optimizer_memory_status(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "dimension": "memory",
        "status": "success",
        "summary": "TODO",
        "details": null
    }))
}

#[tauri::command]
fn optimizer_memory_optimize(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "freedBytes": 0,
        "freedMB": 0
    }))
}

#[tauri::command]
fn optimizer_startup_list(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "dimension": "startup",
        "status": "success",
        "summary": "TODO",
        "details": null
    }))
}

#[tauri::command]
fn optimizer_startup_toggle(_app: tauri::AppHandle, _item_json: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "success": true,
        "message": "TODO"
    }))
}

#[tauri::command]
fn optimizer_system_info(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "dimension": "system",
        "status": "success",
        "summary": "TODO",
        "details": null
    }))
}

// ==================== MAIN ENTRY POINT ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(desktop)]
            {
                // Position main window at right-center-bottom
                if let Some(window) = app.webview_windows().get("main") {
                    if let Some(monitor) = window.current_monitor().ok().flatten() {
                        let screen_size = monitor.size();
                        let ball_size = *BALL_SIZE.lock().unwrap();
                        let size = ball_size + BALL_PADDING * 2;
                        let initial_x = screen_size.width as i32 - size as i32 - 50;
                        let initial_y = (screen_size.height as f32 * 0.65) as i32;
                        let _ = window.set_position(Position::Physical(PhysicalPosition { x: initial_x, y: initial_y }));
                    }
                    let _ = window.show();
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
            start_drag,
            move_window_by,
            drag_end,
            hide_docked_ball,
            set_window_position,
            show_menu,
            hide_menu,
            show_submenu,
            hide_submenu,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
