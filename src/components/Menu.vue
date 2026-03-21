<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { ChevronRight, Palette, Zap } from 'lucide-vue-next'
import { Card } from '@/components/ui/card'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

function applyThemeFromMode(mode: string) {
  let isDark = mode === 'dark'
  if (mode === 'system') {
    isDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  }
  document.documentElement.classList.toggle('dark', isDark)
}

function applyTheme() {
  const saved = localStorage.getItem('aidi-settings')
  let mode = 'system'
  if (saved) {
    const s = JSON.parse(saved)
    mode = s.themeMode || 'system'
  }
  applyThemeFromMode(mode)
}

function onAppearanceEnter() {
  invoke('show_submenu')
}

function hideSubmenu() {
  invoke('hide_submenu')
}

function onOptimizerClick(event: MouseEvent) {
  event.stopPropagation()
  // 直接显示优化器窗口
  invoke('show_optimizer_window')
  // 隐藏菜单（使用 hide_menu 而不是 hide_menu_window，保持状态一致）
  invoke('hide_menu')
}

function onMouseEnter() {
  invoke('menu_enter')
}

function onMouseLeave() {
  // 通过后端状态机管理，而不是直接隐藏
  invoke('menu_leave')
}

let unlisten: (() => void) | null = null

onMounted(async () => {
  applyTheme()
  unlisten = await listen('settings-updated', (event: any) => {
    const settings = event.payload as any
    // Handle both camelCase and snake_case
    if (settings.theme_mode) {
      applyThemeFromMode(settings.theme_mode)
    } else if (settings.themeMode) {
      applyThemeFromMode(settings.themeMode)
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})
</script>

<template>
  <div
    class="menu-wrapper"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
  >
    <Card class="menu-card overflow-hidden border-0 !py-0 !gap-0">
      <div class="py-1">
        <button
          class="menu-item group"
          @mouseenter="onAppearanceEnter"
        >
          <span class="menu-icon">
            <Palette :size="16" class="text-violet-500" />
          </span>
          <span class="menu-label">界面外观</span>
          <ChevronRight :size="14" class="menu-arrow" />
        </button>

        <button
          class="menu-item group"
          @mouseenter="hideSubmenu"
          @click="onOptimizerClick"
        >
          <span class="menu-icon">
            <Zap :size="16" class="text-amber-500" />
          </span>
          <span class="menu-label">系统优化</span>
        </button>
      </div>
    </Card>
  </div>
</template>

<style scoped>
.menu-wrapper {
  padding: 0px;
  overflow: hidden;
  animation: menu-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.menu-card {
  width: 176px;
  border-radius: 10px;
  background: var(--card);
}

.menu-item {
  display: flex;
  align-items: center;
  width: 100%;
  padding: 8px 12px;
  cursor: pointer;
  transition: all 0.15s ease;
  border: none;
  background: transparent;
  text-align: left;
  border-radius: 6px;
  margin: 2px 4px;
  width: calc(100% - 8px);
}

.menu-item:hover {
  background: var(--accent);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
}

.menu-item:active {
  background: color-mix(in oklch, var(--accent) 80%, transparent);
  transform: scale(0.98);
}

.menu-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  margin-right: 8px;
  border-radius: 6px;
  background: var(--muted);
  transition: all 0.15s ease;
}

.menu-item:hover .menu-icon {
  background: var(--background);
}

.menu-label {
  flex: 1;
  color: var(--foreground);
  font-size: 13px;
  font-weight: 500;
}

.menu-arrow {
  color: var(--muted-foreground);
  opacity: 0.5;
  transition: all 0.15s ease;
}

.menu-item:hover .menu-arrow {
  opacity: 1;
  transform: translateX(2px);
}

@keyframes menu-enter {
  from {
    opacity: 0;
    transform: scale(0.96) translateY(-4px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}
</style>
