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

## 关键技术细节

- **端口 1420** 在 `vite.config.ts` 中硬编码，Tauri 的 `devUrl` 必须使用此端口
- `tauri.conf.json` 中启用了 **`macOSPrivateApi: true`**，用于 macOS 半透明窗口效果
- **边缘吸附**：浮动球靠近屏幕边缘时会部分隐藏（仅露出约 35px），悬停时触发"弹出"动画
- `tauri.conf.json` 中 **CSP 已禁用**（`"csp": null`）
- `panel` 窗口（内嵌 `aidi.yadea.com.cn/aigc/` 的 iframe）通过 `open_panel` 命令动态创建，未在 `tauri.conf.json` 中预先声明
