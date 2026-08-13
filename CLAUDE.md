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
- `quick-screenshot-preview` — `Cmd+E`/`Ctrl+E` 截图完成后发给 `main` 窗口，`QuickInputBox` 收到后自动展开输入框并挂上缩略图

关键 invoke 命令（定义于 `src-tauri/src/lib.rs`）：
- **浮动球交互**：`prepare_drag`、`start_drag`、`move_window_by`、`drag_end`、`ball_enter`、`ball_leave`、`hide_docked_ball`
- **菜单**：`show_menu`、`hide_menu`、`menu_enter`、`menu_leave`
- **窗口管理**：`show_main_window`、`hide_main_window`、`show_optimizer_window`、`hide_optimizer_window`、`open_panel`、`update_settings`、`update_window_size`
- **系统优化**：`optimizer_disk_scan`、`optimizer_disk_clean`、`optimizer_disk_health`、`optimizer_memory_status`、`optimizer_memory_optimize`、`optimizer_startup_list`、`optimizer_startup_toggle`、`optimizer_system_info`
- **截图提问**：`trigger_screenshot`（手动触发）、`get_pending_screenshot`（取走待发截图，take 语义）、`clear_pending_screenshot`（删图/收起时清理）、`open_screenshot_preview`（点缩略图用系统看图工具打开大图）

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

### macOS 截图：缺少「屏幕录制」权限时静默返回桌面壁纸

**现象**：`Cmd+E` 截图"成功"、无任何报错，但画面里只有桌面壁纸和 AIDI 自己的浮动球，其他应用的窗口全部消失。

**根因**：缺少「屏幕录制」权限时，macOS 的截屏 API（`xcap` 底层的 `CGWindowListCreateImage`）**不报错**，而是返回一张抹掉其他进程窗口的图。代码层面完全看不出失败，日志里也是"截图成功"。

**解决方案**：截图前用 `CGPreflightScreenCaptureAccess()` 显式预检，未授权则不截图。

```rust
// src-tauri/src/lib.rs - space_screenshot
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;   // 查询是否已授权
    fn CGRequestScreenCaptureAccess() -> bool;     // 弹出系统授权确认框
}
```

**授权交互的顺序很重要**：`CGRequestScreenCaptureAccess()` 对同一 App 身份**只弹一次**弹窗，之后调用只静默返回 false。所以首次未授权时只弹确认框；第二次仍未授权，才用 `open x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture` 引导到设置面板。两个一起调会让刚弹出的确认框被设置面板打断。

**但「只提示一次」的闩锁不能管住用户主动操作**（踩过）：上面这套用两个 `static AtomicBool`（`PERMISSION_REQUESTED` / `SETTINGS_OPENED`）实现，一个进程内各只生效一次。第 3 次起两个分支都跳过，**用户再按快捷键什么都不会发生，也没有任何途径进入授权界面**。原本是为了不骚扰（当时的反馈是「一直跳系统授权界面」），结果矫正过度把唯一入口堵死。

正确做法：自动弹窗保持只弹一次，同时给前端提示行一个**独立的、不受闩锁限制的**命令入口。

```rust
/// 由前端提示行点击触发，必须每次都响应
#[tauri::command]
fn open_screen_recording_settings() {
    space_screenshot::open_screen_permission_settings();
}
```

**后端 emit 的事件必须有人监听，否则就是静默失败**（踩过）：后端在权限缺失时 emit `screenshot-permission-needed`，但 `QuickInputBox.vue` 只监听了 `quick-screenshot-preview`，这个事件没有任何接收方。表现为「按了快捷键完全没反应」——实际是触发了、被权限挡住、然后静默 `return`。加事件时同步确认前端有 `listen`。

**授权后必须重启应用**：TCC 授权对已运行进程不生效。

---

### macOS 开发调试：每次重新编译都会让「屏幕录制」权限失效

**现象**：授权过一次，改代码重编译后权限又失效；系统设置列表里还出现名为 `old`、图标是 `exec` 的陌生条目。

**根因**：`tauri dev` 跑的是 adhoc 签名的裸二进制（`Signature=adhoc, linker-signed`、`TeamIdentifier=not set`）。TCC 对 adhoc 签名按 **cdhash** 记账，每次重新链接 cdhash 都变，系统就当成另一个程序。裸二进制没有 Bundle 信息，列表里自然显示不出正常应用名。

**开发期做法**：用 `npx tauri build --bundles app` 打成 `.app` 测——有 `CFBundleName`（列表显示 `AIDI Desktop`）和固定 `CFBundleIdentifier`，同一份包反复启动权限有效。但重新打包 cdhash 仍会变，所以**把改 Rust 代码的验证攒到一起，只重打一次包**；纯前端改动走 Vite 热更新，不受影响。

**彻底解决**：用 Apple Developer ID 证书正式签名，TCC 按团队标识记账，重新打包不失效。

**清掉陈旧记录**：`tccutil reset ScreenCapture com.aidi.desktop`（只影响本 app，并让授权弹窗恢复「首次」资格）。

**注意**：不要为了"重置干净"反复 `tccutil reset` + 重打包——每次都换 cdhash，用户每次都要重授，很容易把自己和用户都绕晕。

---

### `tauri dev` 自动重启会让你测到旧二进制（判断方法也容易错）

**现象**：改完 Rust 代码，`tauri dev` 显示 `Finished` + `Running`，日志里却还是旧版的输出文案，行为也是旧的。

**根因**：`tauri dev` 的文件监听与 `cargo` 的重新链接是两个异步过程。连续编辑（尤其编辑器写 `lib.rs.tmp.*` 临时文件）会触发多轮 `Rebuilding`，结果**进程启动时刻早于二进制链接时刻**，跑的是上一版。

**判断方法**：比对进程启动时间与二进制 mtime，前者必须 ≥ 后者。

```bash
B=src-tauri/target/debug/aidi-desktop-tauri
stat -f "%Sm" -t "%H:%M:%S" $B                                  # 二进制链接时间
ps -o lstart=,pid= -p $(pgrep -f "target/debug/aidi-desktop-tauri" | head -1)
```

不一致就手动重启（`TaskStop` 后重新 `npm run tauri:dev:local`），别指望它自己纠正。

**⚠️ 别用 `strings` 校验二进制里的中文字符串**：`strings` 默认按可打印 ASCII 提取，**中文文案一个都读不出来**，结果恒为 0，看起来像"新代码没进去"。用 `grep -a` 直接搜字节：

```bash
strings $B | grep -c '空格已按住'    # ❌ 恒为 0，得出错误结论
grep -ac '空格已按住' $B             # ✅ 有效
```

**自查**：拿一个**确定存在**的旧文案做对照实验。若连它也是 0，说明方法本身不可靠，而不是代码没编进去。

---

### 长按空格触发截图：只能观察、不能拦截

**触发键的演变**：长按空格 → `Cmd+E`/`Ctrl+E`（因空格在输入框内冲突）→ 又改回长按空格 3s（并保留 `Cmd+E`）。

**实现选择**：轮询单个按键状态，而不是 `CGEventTap`。

```rust
// macOS：只读某一个键的状态，不建事件监听链
fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;   // state_id=0, kVK_Space=0x31
// Windows：GetAsyncKeyState(VK_SPACE) & 0x8000
```

`CGEventTap` 会监听全部键盘事件，需要「输入监控」权限，且历史上触发过 Apple Music 媒体库权限弹窗（当初换掉 `tauri-plugin-global-shortcut` 就是这个原因）。轮询只查单键状态，两个问题都不存在。

**固有缺陷（无法消除）**：轮询只能观察按键，**不能吞掉它**。按住空格这 3 秒，当前应用照样在收连续空格——编辑器里连打空格、浏览器里连续翻页，然后截图弹出来。3 秒阈值只是把误触概率压低，不是解决。要真正拦截必须上 `CGEventTap`，代价是回到权限弹窗那个坑。

**⚠️ 计时别用累加计数**（踩过）：

```rust
// ❌ sleep(100ms) 实际间隔略大于 100ms，累加到「3000」时真实已过去 3.2~3.3 秒
held_ms += POLL_INTERVAL_MS;
if held_ms >= 3000 { ... }

// ✅ 用真实时间戳
let start = *press_start.get_or_insert_with(Instant::now);
if start.elapsed() >= Duration::from_millis(LONG_PRESS_MS) { ... }
```

偷偷抬高阈值的后果：用户按了 3 秒松手、没反应，双方都以为是功能没生效。

**调试日志要能区分触发源**：`do_screenshot_and_emit(app, source)` 带上来源（`长按空格 3s` / `Cmd+E` / `Ctrl+E`），并在松开时记录实际按住毫秒数。否则"没反应"分不清是触发没到、还是到了却被权限挡住。

---

### 发布打包：DMG 失败与签名缺失（每次发版都会遇到）

**现象 1：`.app` 打包成功，DMG 报错中止**

```
Bundling AIDI Desktop_1.0.0_aarch64.dmg
   Running bundle_dmg.sh
failed to bundle project error running bundle_dmg.sh: `failed to run .../bundle_dmg.sh`
```

**根因**：`bundle_dmg.sh` 用 `osascript` 指挥 Finder 摆图标位置，这需要「自动化」权限。非交互终端（CI、后台任务、agent）弹不出授权框，直接失败：

```
execution error: 未获得授权将Apple事件发送给Finder。 (-1743)
```

脚本第 10 行是 `set -e`，一失败即整体中止。Tauri 只转述 "failed to run"，不输出 stderr，所以要单独跑 `osascript -e 'tell application "Finder" to get name of startup disk'` 才能看到真实原因。

**解决**：手动调用脚本并加 `--skip-jenkins`（跳过 Finder 美化，不影响拖拽安装功能）：

```bash
cd src-tauri/target/release/bundle/dmg
./bundle_dmg.sh --volname "AIDI Desktop" \
  --icon "AIDI Desktop.app" 180 170 --app-drop-link 480 170 \
  --window-size 660 400 --hide-extension "AIDI Desktop.app" \
  --skip-jenkins \
  "AIDI Desktop_1.0.0_aarch64.dmg" "../macos/AIDI Desktop.app"
```

**重试前必须清理残留**，否则 `hdiutil` 会因文件占用再次失败：

```bash
rm -f src-tauri/target/release/bundle/macos/rw.*.dmg
rm -f src-tauri/target/release/bundle/dmg/*.dmg
```

**现象 2：`spctl` 报 `code has no resources but signature indicates they must be present`**

**根因**：Tauri 产出的 `.app` 缺少 `Contents/_CodeSignature` 目录，签名校验无法通过。

**解决**：补签名后再打 DMG（顺序不能颠倒，否则 DMG 里装的还是无签名版本）：

```bash
codesign --force --deep --sign - "src-tauri/target/release/bundle/macos/AIDI Desktop.app"
codesign --verify --deep --strict --verbose=2 "...AIDI Desktop.app"   # 应输出 valid on disk
```

**adhoc 签名的固有限制**：`spctl` 评估仍是 `rejected`（无 Developer ID、未公证）。他人下载后首次打开需右键 → 打开，或 `xattr -dr com.apple.quarantine "/Applications/AIDI Desktop.app"`。要免除此步骤，必须用 Apple Developer ID 证书签名 + `notarytool` 公证。

**通用包（Intel + Apple Silicon）**：默认构建只产出当前架构（文件名带 `aarch64`），Intel Mac 无法运行（Rosetta 不能反向转译）。要覆盖两种机型：

```bash
rustup target add x86_64-apple-darwin        # 仅首次
AIDI_ENV=prod npx tauri build --target universal-apple-darwin --bundles app
lipo -info ".../universal-apple-darwin/release/bundle/macos/AIDI Desktop.app/Contents/MacOS/aidi-desktop-tauri"
# 应输出：x86_64 arm64
```

**坑**：`--bundles app` 不生成 DMG 所需的辅助文件，直接调 `bundle_dmg.sh` 会报
`Cannot find support/ directory`。需从 `release/bundle` 复制两样东西过去：

```bash
R=src-tauri/target/release/bundle
U=src-tauri/target/universal-apple-darwin/release/bundle
mkdir -p "$U/dmg" "$U/share"
cp "$R/dmg/bundle_dmg.sh" "$R/dmg/icon.icns" "$U/dmg/"   # 打包脚本
cp -R "$R/share/create-dmg" "$U/share/"                   # AppleScript 模板（support/ 在此）
chmod +x "$U/dmg/bundle_dmg.sh"
```

另外 universal 包内含两个架构切片，用 `grep -c` 查字符串时**每个计数都会翻倍**（每架构各一份），这是正常现象，不是重复代码。

**Windows 包无法在 macOS 产出**：`tauri.conf.json` 的 `targets` 含 `nsis`，但 NSIS 不支持交叉编译，必须在 Windows 机器上执行 `npm run tauri:build:prod`。

**发布顺序（有依赖，不可颠倒）**：prod 包的聊天/菜单/登录窗口加载远程 URL `https://aidi.yadea.com.cn/aidi-desktop`，因此 **`aidi-desktop-web` 必须先部署上线**，否则新 app 会加载到旧版页面。

---

## 关键技术细节

- **端口 1420** 在 `vite.config.ts` 中硬编码，Tauri 的 `devUrl` 必须使用此端口
- `tauri.conf.json` 中启用了 **`macOSPrivateApi: true`**，用于 macOS 半透明窗口效果
- **边缘吸附**：已移除。浮动球不再自动吸附到屏幕边缘
- `tauri.conf.json` 中 **CSP 已禁用**（`"csp": null`）
- `panel` 窗口（内嵌 `aidi.yadea.com.cn/aigc/` 的 iframe）通过 `open_panel` 命令动态创建，未在 `tauri.conf.json` 中预先声明
