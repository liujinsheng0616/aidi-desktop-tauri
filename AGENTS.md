# AGENTS.md - Development Guide for aidi-desktop-tauri

## Project Overview

A Tauri-based desktop application with a floating ball UI and system optimizer features. Built with Vue 3 + TypeScript frontend and Rust backend.

## Tech Stack

- **Frontend**: Vue 3, TypeScript, Vite, Tailwind CSS v4, Reka UI (radix-vue)
- **Backend**: Rust, Tauri v2
- **Build Tools**: Vite, Cargo

## Build Commands

```bash
# Development (runs Vite dev server on port 1420)
npm run dev

# Production build (TypeScript check + Vite build)
npm run build

# Tauri development (hot reload)
npm run tauri:dev

# Tauri production build
npm run tauri:build

# Preview production build
npm run preview

# Type checking only
npx vue-tsc --noEmit
```

## Rust Commands

```bash
# Build Rust library
cd src-tauri && cargo build

# Build release
cd src-tauri && cargo build --release

# Check Rust code
cd src-tauri && cargo check

# Format Rust code
cd src-tauri && cargo fmt

# Lint Rust code
cd src-tauri && cargo clippy
```

## Testing

**No test framework configured yet.** The project currently has no test files. To add testing:

- Frontend: Consider Vitest for Vue/TypeScript testing
- Backend: Use `cargo test` for Rust unit tests

## Code Style Guidelines

### Vue/TypeScript (Frontend)

#### File Structure
- Components: `src/components/**/*.vue`
- UI Components: `src/components/ui/<component>/ComponentName.vue`
- Composables: `src/composables/**/*.ts`
- Utils: `src/lib/utils.ts`
- Assets: `src/assets/`

#### Component Conventions
- Use `<script setup lang="ts">` syntax
- Props: Use TypeScript interfaces with `defineProps<{}>()`
- Events: Use `defineEmits<{}>()` with type definitions
- Component names: PascalCase (e.g., `FloatingBall.vue`)
- Props naming: camelCase in script, kebab-case in template

#### Imports
```typescript
// Order: Vue/core -> Tauri -> Third-party -> Local
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { cn } from '@/lib/utils'
import FloatingBall from '@/components/FloatingBall.vue'

// Use @/ alias for src/ directory
```

#### TypeScript
- Strict mode enabled
- Target: ES2020
- Use `type` keyword for type imports when possible
- Prefer interfaces over type aliases for objects
- Use explicit return types on public functions

#### Styling (Tailwind CSS v4)
- Use Tailwind CSS utility classes
- Custom CSS in `<style scoped>` for component-specific styles
- CSS variables for theming in `style.css`
- Use `cn()` utility for conditional class merging

#### Naming Conventions
- Variables/functions: camelCase
- Components: PascalCase
- Files: PascalCase for components, camelCase for utilities
- Constants: UPPER_SNAKE_CASE

### Rust (Backend)

#### File Structure
- Main entry: `src-tauri/src/main.rs`
- Library: `src-tauri/src/lib.rs`
- All Rust code lives in `src-tauri/src/`

#### Code Organization
```rust
// Use modules for organization
mod window_manager;
mod system_optimizer;

// Re-export commonly used items
pub use window_manager::*;
```

#### Naming Conventions
- Types (structs/enums): PascalCase
- Functions/variables: snake_case
- Constants: SCREAMING_SNAKE_CASE
- Modules: snake_case

#### Error Handling
- Use `Result<T, E>` for fallible operations
- Custom error types with `thiserror` or `anyhow`
- Log errors with appropriate context

#### Safety
- Mark unsafe code with `// SAFETY: <explanation>` comments
- Minimize unsafe blocks
- Use Tauri's safe APIs when possible

## Tauri Commands

Frontend invokes backend via:
```typescript
import { invoke } from '@tauri-apps/api/core'

// Call Rust function
await invoke('command_name', { arg1: value1 })
```

Backend exposes commands:
```rust
#[tauri::command]
fn command_name(arg1: Type) -> ResultType {
    // Implementation
}
```

## State Management

- Local state: Vue `ref()` and `reactive()`
- Cross-window state: Tauri events with `listen()` and `emit()`
- Persistent state: localStorage for frontend, filesystem for backend

## Multi-Window Architecture

The app uses multiple windows:
- `main`: Floating ball window
- `menu`: Main menu window
- `submenu`: Submenu window
- `optimizer`: System optimizer panel

Each has its own HTML entry point and Vue app instance.

## Communication Patterns

1. **Frontend → Backend**: `invoke('command_name', payload)`
2. **Backend → Frontend**: `window.emit('event_name', payload)`
3. **Window → Window**: Use Tauri events with unique event names

## Performance Guidelines

- Use `v-memo` for expensive renders
- Debounce rapid events (drag, resize)
- Clean up event listeners in `onUnmounted`
- Use `computed()` for derived state
- Minimize reactive dependencies

## Git Workflow

- Main branch: `main`
- Feature branches: `feature/description`
- Commit messages: Clear, descriptive, present tense
- No commits directly to main

## Common Issues

1. **Port 1420 in use**: Kill other Vite processes or change port in `vite.config.ts`
2. **Rust compilation errors**: Run `cargo clean` and rebuild
3. **Type errors**: Run `npx vue-tsc --noEmit` to check
4. **Tauri dev not reloading**: Check that `src-tauri/` is in watch ignore list

## Resources

- [Tauri v2 Docs](https://v2.tauri.app/)
- [Vue 3 Docs](https://vuejs.org/)
- [Tailwind CSS v4](https://tailwindcss.com/)
- [Reka UI](https://reka-ui.com/)
