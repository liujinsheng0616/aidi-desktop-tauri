# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 沟通规则

**所有回复必须使用中文。**

## Project Overview

AIDI Desktop is a Tauri 2 + Vue 3 desktop application featuring a draggable floating ball with edge-docking behavior, a context menu system, and a system optimizer panel. The app supports both macOS and Windows platforms.

## Development Commands

```bash
# Install dependencies
npm install

# Start development server (frontend only)
npm run dev

# Start Tauri in development mode (full app)
npm run tauri dev

# Build for production
npm run tauri build

# Type check
vue-tsc --noEmit

# Build frontend only
npm run build
```

## Architecture

### Multi-Window System

The application uses three Tauri windows, each with its own HTML entry point and Vue app:

| Window | Entry Point | Vue Component | Purpose |
|--------|-------------|---------------|---------|
| `main` | index.html → App.vue | FloatingBall.vue | Draggable floating ball with edge-docking |
| `menu` | menu.html | MenuPanel.vue | Context menu with appearance settings and optimizer access |
| `optimizer` | optimizer.html | OptimizerPanel.vue | System optimizer with disk/memory/startup tools |

### Frontend Stack

- **Vue 3** with Composition API (`<script setup>`)
- **TypeScript** with strict mode enabled
- **Tailwind CSS v4** via `@tailwindcss/vite` plugin
- **reka-ui** and **radix-vue** for UI components
- **lucide-vue-next** for icons
- **@vueuse/core** for Vue utilities

### Backend (Rust/Tauri)

The Rust backend in `src-tauri/src/lib.rs` handles:

1. **Window Management**: Position, visibility, and focus for all windows
2. **Ball Interaction State Machine**: `InteractionState` enum (`Idle`, `Hovering`, `MenuShowing`, `HideDelaying`, `Dragging`, `Animating`)
3. **Edge Docking System**: `DockState` struct tracks docking status, position, and hover states
4. **Smart Menu Positioning**: `detect_ball_position()` and `calculate_menu_strategy()` determine optimal menu placement based on ball position
5. **Animation System**: `animate_to_position()` with ease-out cubic easing

### System Optimizer Scripts

Platform-specific shell scripts in `src-tauri/scripts/`:

| Script | macOS (.sh) | Windows (.ps1) | Purpose |
|--------|-------------|----------------|---------|
| disk-scan | ✓ | ✓ | Scan for cleanable files |
| disk-clean | ✓ | ✓ | Clean selected file categories |
| disk-health | ✓ | ✓ | Check disk health status |
| memory-status | ✓ | ✓ | Get memory usage info |
| memory-optimize | ✓ | ✓ | Free up memory |
| startup-list | ✓ | ✓ | List startup items |
| startup-toggle | ✓ | ✓ | Enable/disable startup items |
| system-info | ✓ | ✓ | Get system information |

Scripts are executed via `run_script()` / `run_script_with_args()` functions that parse JSON output.

### Key Tauri Commands

Backend commands callable from frontend via `invoke()`:

- **Window Control**: `show_menu`, `hide_menu`, `show_optimizer_window`, `hide_optimizer_window`
- **Ball Interaction**: `ball_enter`, `ball_leave`, `menu_enter`, `menu_leave`
- **Dragging**: `start_drag`, `move_window_by`, `drag_end`
- **Settings**: `update_settings`, `update_window_size`
- **Optimizer**: `optimizer_scan_all`, `optimizer_disk_scan`, `optimizer_disk_clean`, `optimizer_memory_status`, `optimizer_memory_optimize`, `optimizer_startup_list`, `optimizer_startup_toggle`, `optimizer_system_info`

### State Management

- **Optimizer Store**: `src/stores/optimizer.ts` exports `useOptimizer()` composable with reactive state for scan results and optimization selections
- **Local Settings**: Settings persist in `localStorage` under `aidi-settings` key
- **Backend State**: Global `DockState` in Rust manages docking and interaction state

## Platform Considerations

- **macOS**: Uses `macos-private-api` feature for transparent windows; MENUBAR_HEIGHT = 25px offset
- **Windows**: Slower animation frame rate (30fps vs 60fps), longer hide delays
- **Path Alias**: `@/*` maps to `./src/*` in TypeScript

## File Structure

```
src/
├── components/
│   ├── FloatingBall.vue      # Main ball component with drag/dock logic
│   ├── MenuPanel.vue         # Menu with settings submenu
│   ├── OptimizerPanel.vue    # Main optimizer panel
│   ├── optimizer/            # Optimizer sub-components
│   └── ui/                   # Reusable UI components (button, card, etc.)
├── stores/
│   └── optimizer.ts          # Optimizer state composable
├── lib/
│   └── utils.ts              # Utility functions (cn helper)
├── App.vue                   # Main window root component
├── main.ts                   # Main window entry
├── menu.ts                   # Menu window entry
└── optimizer.ts              # Optimizer window entry

src-tauri/
├── src/
│   └── lib.rs                # All Rust backend logic
├── scripts/                  # Platform-specific optimizer scripts
├── Cargo.toml                # Rust dependencies
└── tauri.conf.json           # Tauri configuration
```
