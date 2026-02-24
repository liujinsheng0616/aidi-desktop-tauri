<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { ChevronRight, Palette, Zap, Sun, Moon, Monitor } from 'lucide-vue-next'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { Label } from '@/components/ui/label'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { LogicalSize } from '@tauri-apps/api/dpi'
import { cn } from '@/lib/utils'

const appWindow = getCurrentWebviewWindow()

// Submenu state
const submenuVisible = ref(false)
const submenuDirection = ref<'right' | 'left'>('right') // 子菜单展开方向
const themeMode = ref('system')
const opacity = ref([100])
const ballSize = ref([60])
const colorTheme = ref('cyan-purple')

const colorThemes = [
  { id: 'cyan-purple', name: '青紫', gradient: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)' },
  { id: 'ocean', name: '海洋', gradient: 'linear-gradient(135deg, #0052d4 0%, #6fb1fc 100%)' },
  { id: 'forest', name: '森林', gradient: 'linear-gradient(135deg, #11998e 0%, #38ef7d 100%)' },
  { id: 'fire', name: '火焰', gradient: 'linear-gradient(135deg, #f12711 0%, #f5af19 100%)' },
  { id: 'midnight', name: '午夜', gradient: 'linear-gradient(135deg, #000000 0%, #1a1a1a 50%, #333333 100%)' },
]

const themeModes = [
  { id: 'light', name: '浅色', icon: Sun },
  { id: 'dark', name: '深色', icon: Moon },
  { id: 'system', name: '系统', icon: Monitor },
]

// Theme functions
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

function loadSettings() {
  const saved = localStorage.getItem('aidi-settings')
  if (saved) {
    const s = JSON.parse(saved)
    themeMode.value = s.themeMode || 'system'
    opacity.value = [s.opacity ?? 100]
    ballSize.value = [s.ballSize || 60]
    colorTheme.value = s.colorTheme || 'cyan-purple'
  }
}

function saveSettings() {
  const settings = {
    themeMode: themeMode.value,
    opacity: opacity.value[0],
    ballSize: ballSize.value[0],
    colorTheme: colorTheme.value,
  }
  localStorage.setItem('aidi-settings', JSON.stringify(settings))

  // Call backend to update settings
  invoke('update_settings', {
    settings: {
      theme_mode: themeMode.value,
      opacity: opacity.value[0],
      ball_size: ballSize.value[0],
      color_theme: colorTheme.value,
    }
  })

  // Update window size
  invoke('update_window_size', { size: ballSize.value[0] })
}

function setThemeMode(mode: string) {
  themeMode.value = mode
  applyThemeFromMode(mode)
  saveSettings()
}

function setColorTheme(theme: string) {
  colorTheme.value = theme
  saveSettings()
}

// Menu interaction functions
async function showSubmenu() {
  // 先调整窗口大小，再显示子菜单
  await appWindow.setSize(new LogicalSize(436, 368))
  submenuVisible.value = true
}

async function hideSubmenu() {
  // 先隐藏子菜单，再调整窗口大小
  submenuVisible.value = false
  await appWindow.setSize(new LogicalSize(436, 116))
}

function onOptimizerClick(event: MouseEvent) {
  event.stopPropagation()
  console.log('Optimizer clicked!')
  // Show optimizer window
  invoke('show_optimizer_window')
  // Hide menu
  invoke('hide_menu')
}

function onMouseEnter() {
  invoke('menu_enter')
}

function onMouseLeave() {
  invoke('menu_leave')
}

// Watch for settings changes
watch(opacity, saveSettings)
watch(ballSize, saveSettings)

let unlisten: (() => void) | null = null
let unlistenDirection: (() => void) | null = null
let unlistenHide: (() => void) | null = null

onMounted(async () => {
  applyTheme()
  loadSettings()
  unlisten = await listen('settings-updated', (event: any) => {
    const settings = event.payload as any
    if (settings.theme_mode) {
      applyThemeFromMode(settings.theme_mode)
    } else if (settings.themeMode) {
      applyThemeFromMode(settings.themeMode)
    }
  })

  // 监听子菜单展开方向
  unlistenDirection = await listen<{ direction: string }>('submenu-direction', (event) => {
    submenuDirection.value = event.payload.direction as 'right' | 'left'
  })

  // 监听菜单隐藏事件，重置子菜单状态
  unlistenHide = await listen('menu-hidden', () => {
    submenuVisible.value = false
    appWindow.setSize(new LogicalSize(436, 116))
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
  if (unlistenDirection) unlistenDirection()
  if (unlistenHide) unlistenHide()
})
</script>

<template>
  <div
    :class="['menu-panel', submenuDirection === 'left' ? 'direction-left' : 'direction-right']"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
  >
    <!-- Main Menu -->
    <div class="menu-container">
      <Card class="menu-card overflow-hidden border-0 !py-0 !gap-0">
        <div class="py-1">
          <button
            class="menu-item group"
            @mouseenter="showSubmenu"
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

    <!-- Submenu -->
    <div
      v-if="submenuVisible"
      class="submenu-container"
      @mouseenter="showSubmenu"
      @mouseleave="hideSubmenu"
    >
      <Card class="submenu-card border-0 !py-0 !gap-0">
        <CardContent class="p-4 space-y-4">
          <!-- Theme Mode -->
          <div class="space-y-2">
            <Label class="text-xs uppercase text-muted-foreground font-medium">主题模式</Label>
            <div class="flex gap-1.5">
              <Button
                v-for="mode in themeModes"
                :key="mode.id"
                :variant="themeMode === mode.id ? 'default' : 'outline'"
                size="sm"
                :class="cn(
                  'flex-1 gap-1.5 text-xs',
                  mode.id === 'dark' && themeMode === mode.id && '!bg-black hover:!bg-zinc-900 !text-white !border-black'
                )"
                @click="setThemeMode(mode.id)"
              >
                <component :is="mode.icon" :size="14" />
                {{ mode.name }}
              </Button>
            </div>
          </div>

          <!-- Opacity -->
          <div class="space-y-2">
            <div class="flex justify-between items-center">
              <Label class="text-sm font-medium">透明度</Label>
              <span class="text-xs text-muted-foreground tabular-nums">{{ opacity[0] }}%</span>
            </div>
            <Slider
              v-model="opacity"
              :min="30"
              :max="100"
              :step="1"
            />
          </div>

          <!-- Ball Size -->
          <div class="space-y-2">
            <div class="flex justify-between items-center">
              <Label class="text-sm font-medium">浮动球大小</Label>
              <span class="text-xs text-muted-foreground tabular-nums">{{ ballSize[0] }}px</span>
            </div>
            <Slider
              v-model="ballSize"
              :min="60"
              :max="100"
              :step="1"
            />
          </div>

          <!-- Color Theme -->
          <div class="space-y-2">
            <Label class="text-xs uppercase text-muted-foreground font-medium">颜色主题</Label>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="theme in colorThemes"
                :key="theme.id"
                :class="cn(
                  'flex flex-col items-center gap-1 p-1 rounded-lg transition-all',
                  colorTheme === theme.id ? 'bg-accent' : 'hover:bg-accent/50'
                )"
                @click="setColorTheme(theme.id)"
              >
                <span
                  :class="cn(
                    'w-7 h-7 rounded-full shadow-md transition-all',
                    colorTheme === theme.id && 'ring-2 ring-primary ring-offset-1 ring-offset-background'
                  )"
                  :style="{ background: theme.gradient }"
                />
                <span class="text-[10px] text-foreground font-medium">{{ theme.name }}</span>
              </button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>

<style scoped>
.menu-panel {
  display: flex;
  padding: 4px;
  gap: 8px;
}

/* 默认方向：子菜单在右边 */
.direction-right {
  flex-direction: row;
}

/* 反向：子菜单在左边 */
.direction-left {
  flex-direction: row-reverse;
}

.menu-container {
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

.submenu-container {
  /* padding 由 menu-panel 统一控制，这里不需要额外 padding */
}

/* 子菜单动画 - 根据方向不同 */
.direction-right .submenu-container {
  animation: submenu-enter-right 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.direction-left .submenu-container {
  animation: submenu-enter-left 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.submenu-card {
  width: 236px;
  border-radius: 12px;
  background: var(--card);
  overflow: hidden;
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

@keyframes submenu-enter-right {
  from {
    opacity: 0;
    transform: scale(0.96) translateX(-4px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateX(0);
  }
}

@keyframes submenu-enter-left {
  from {
    opacity: 0;
    transform: scale(0.96) translateX(4px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateX(0);
  }
}
</style>
