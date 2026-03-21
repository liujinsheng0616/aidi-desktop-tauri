# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 语言要求

始终使用中文回复用户。

## 项目概述

AIDI Desktop 是基于 Tauri v2 + Vue 3 + TypeScript 构建的 macOS/Windows 浮动球助手。提供可拖拽、始终置顶的浮动球，支持吸附到屏幕边缘，并包含系统优化面板。

## 常用命令

```bash
# 仅启动前端开发服务器（端口 1420，Tauri 必需）
npm run dev

# 启动完整桌面应用开发模式（同时启动 Rust 后端 + Vite）
npx tauri dev

# TypeScript 类型检查
vue-tsc --noEmit

# 生产构建（类型检查 + Vite + Tauri 打包）
npm run tauri:build
# 等价于：
npx tauri build
```

项目未配置测试命令，`vue-tsc --noEmit` 是主要的静态验证手段。

## 架构说明

### 多窗口结构

三个独立的 Tauri 窗口，各有对应的 Vue 入口：

| 窗口 | 入口文件 | 用途 |
|------|---------|------|
| `main` | `index.html` → `src/main.ts` → `App.vue` → `FloatingBall.vue` | 可拖拽浮动球 |
| `menu` | `menu.html` → `src/menu.ts` → `MenuPanel.vue` | 右键菜单弹窗 |
| `chat` | 远程 URL → `aidi-desktop-web/#/chat` | AI 聊天面板 |
| `optimizer` | `optimizer.html` → `src/optimizer.ts` → `OptimizerPanel.vue` | 系统优化面板 |

所有窗口均无边框且透明。`main` 窗口初始为 120x120 的浮动球。Vite 配置为多页应用，包含上述三个 HTML 入口。

### 前后端通信

前端通过 Tauri 的 `invoke()` 调用命令，通过 `listen()`/`emit()` 收发事件。

关键事件：
- `settings-updated` — 外观设置变更时广播，所有窗口均监听此事件

关键 invoke 命令（定义于 `src-tauri/src/lib.rs`）：
- **浮动球交互**：`prepare_drag`、`start_drag`、`move_window_by`、`drag_end`、`ball_enter`、`ball_leave`、`hide_docked_ball`
- **菜单**：`show_menu`、`hide_menu`、`menu_enter`、`menu_leave`
- **窗口管理**：`show_main_window`、`hide_main_window`、`show_optimizer_window`、`hide_optimizer_window`、`open_panel`、`update_settings`、`update_window_size`
- **系统优化**：`optimizer_disk_scan`、`optimizer_disk_clean`、`optimizer_disk_health`、`optimizer_memory_status`、`optimizer_memory_optimize`、`optimizer_startup_list`、`optimizer_startup_toggle`、`optimizer_system_info`

### Rust 后端（`src-tauri/src/lib.rs`）

单文件约 1450 行，管理以下状态：

- **`DockState`**（`Mutex` 包装）— 跟踪浮动球位置、吸附/弹出状态、菜单可见性
- **`InteractionState`** 枚举 — `Idle`、`Hovering`、`MenuShowing`、`Dragging`
- **`BallPosition`** 枚举 — 9 个屏幕区域，用于决定菜单弹出方向
- **`MenuPosition`** 枚举 — `Below`、`Above`、`Left`、`Right`

优化器命令通过执行平台专属脚本实现，而非原生 Rust 代码。脚本位于 `src-tauri/scripts/`，macOS 为 `.sh`，Windows 为 `.ps1`，均作为 Tauri `resources` 打包。

### 设置与状态同步

- 外观设置（透明度、球大小、颜色主题、主题模式）存储在前端 `localStorage`
- 设置变更时，前端调用 `update_settings` 同步到 Rust `DockState`
- 通过广播 `settings-updated` 事件保持所有窗口同步

### 优化器状态（`src/stores/optimizer.ts`）

`useOptimizer()` composable 管理所有优化器 UI 状态并调用后端命令。`src/components/optimizer/` 下的子组件各自负责一项功能（磁盘清理、内存、启动项管理、磁盘健康、系统信息）。

### UI 组件

`src/components/ui/` 下的基础 UI 组件遵循 shadcn-vue 模式，基于 `reka-ui`（Radix Vue 的分支）。`src/lib/utils.ts` 中的 `cn()` 工具函数组合了 `clsx` + `tailwind-merge`。

## 目录结构

```
aidi-desktop-tauri/
├── index.html                    # 浮动球窗口
├── menu.html                     # 右键菜单窗口
├── optimizer.html                # 系统优化窗口
├── login.html                    # 飞书登录窗口
├── panel.html                    # AI 面板窗口（动态创建）
├── chat.html                     # 聊天窗口（加载远程 URL）
├── vite.config.ts                # 多页应用配置，端口 1420
│
├── src/
│   ├── main.ts                   # 浮动球 - 入口脚本
│   ├── menu.ts                   # 右键菜单 - 入口脚本
│   ├── optimizer.ts              # 系统优化 - 入口脚本
│   ├── login.ts                  # 飞书登录 - 入口脚本
│   ├── panel.ts                  # AI 面板 - 入口脚本
│   ├── chat.ts                   # 聊天 - 入口脚本
│   ├── App.vue                   # 浮动球 - 根组件
│   ├── components/
│   │   ├── FloatingBall.vue      # 浮动球 - 可拖拽球体
│   │   ├── MenuPanel.vue         # 右键菜单 - 菜单面板
│   │   ├── LoginPage.vue         # 飞书登录 - 扫码登录页
│   │   └── optimizer/
│   │       ├── OptimizerPanel.vue   # 系统优化 - 主面板
│   │       ├── DiskClean.vue        # 系统优化 - 磁盘清理
│   │       ├── DiskHealth.vue       # 系统优化 - 磁盘健康
│   │       ├── MemoryStatus.vue     # 系统优化 - 内存状态
│   │       ├── StartupManager.vue   # 系统优化 - 启动项管理
│   │       └── SystemInfo.vue       # 系统优化 - 系统信息
│   └── stores/
│       ├── auth.ts               # 飞书登录 - OAuth 认证逻辑
│       └── optimizer.ts          # 系统优化 - 状态管理
│
└── src-tauri/
    ├── tauri.conf.json           # 窗口配置、权限声明
    ├── src/
    │   └── lib.rs                # 全部后端逻辑（DockState、所有 invoke 命令）
    └── scripts/                  # 系统优化 - 平台脚本（macOS .sh / Windows .ps1）
        ├── disk-{scan,clean,health}.sh
        ├── memory-{status,optimize}.sh
        ├── startup-{list,toggle}.sh
        └── system-info.sh
```

## 踩坑记录

### Windows 浮动球：失焦后顶部出现灰色半圆弧

**现象**：点击浮动球外部区域（窗口失焦）后，球的顶部出现一块灰色半圆弧残影；拖动后消失。

**根因**：DWM（桌面窗口管理器）在窗口焦点切换时会触发 NC（非客户区）重绘：
- `WM_NCACTIVATE`：窗口激活状态变化时，DWM 重绘 NC 标题区 → 出现灰色
- `WM_NCPAINT`：NC 区域脏标记触发重绘 → 灰色残留
- `ball_window_proc` 只拦截了 `WM_NCCALCSIZE`，这两个消息未处理，走 `DefSubclassProc` 会触发默认 NC 绘制

**解决方案**：在 `ball_window_proc` 中追加对这两条消息的拦截，阻止 DWM 绘制任何 NC 内容。

```rust
// src-tauri/src/lib.rs - ball_window_proc()
use windows::Win32::UI::WindowsAndMessaging::{WM_NCCALCSIZE, WM_NCACTIVATE, WM_NCPAINT};

// ... WM_NCCALCSIZE 处理保持不变 ...

// 拦截 NC 激活重绘：返回 TRUE(1) 阻止 DWM 绘制灰色标题栏
if msg == WM_NCACTIVATE {
    return windows::Win32::Foundation::LRESULT(1);
}

// 拦截 NC 绘制：直接吞掉，不绘制任何 NC 内容
if msg == WM_NCPAINT {
    return windows::Win32::Foundation::LRESULT(0);
}
```

**原理**：
- `WM_NCACTIVATE` 返回 `1`（TRUE）= 告诉系统"已处理激活状态变化"，DWM 不再重绘 NC 区域
- `WM_NCPAINT` 返回 `0` = 告诉系统"NC 区域无需绘制"，跳过整个 NC 绘制流程

**注意**：无需修改 `Cargo.toml`，`Win32_UI_WindowsAndMessaging` feature 已包含这两个消息常量。

---

## 关键技术细节

- **端口 1420** 在 `vite.config.ts` 中硬编码，Tauri 的 `devUrl` 必须使用此端口
- `tauri.conf.json` 中启用了 **`macOSPrivateApi: true`**，用于 macOS 半透明窗口效果
- **边缘吸附**：已移除。浮动球不再自动吸附到屏幕边缘
- `tauri.conf.json` 中 **CSP 已禁用**（`"csp": null`）
- `panel` 窗口（内嵌 `aidi.yadea.com.cn/aigc/` 的 iframe）通过 `open_panel` 命令动态创建，未在 `tauri.conf.json` 中预先声明
