<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Sun, Moon, Monitor } from 'lucide-vue-next'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { Label } from '@/components/ui/label'
import { cn } from '@/lib/utils'

const themeMode = ref('system')
const opacity = ref([100])
const ballSize = ref([48])
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

function loadSettings() {
  const saved = localStorage.getItem('aidi-settings')
  if (saved) {
    const s = JSON.parse(saved)
    themeMode.value = s.themeMode || 'system'
    opacity.value = [s.opacity ?? 100]
    ballSize.value = [s.ballSize || 48]
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

  // 调用后端更新设置
  invoke('update_settings', {
    settings: {
      theme_mode: themeMode.value,
      opacity: opacity.value[0],
      ball_size: ballSize.value[0],
      color_theme: colorTheme.value,
    }
  })

  // 同时更新窗口大小
  invoke('update_window_size', { size: ballSize.value[0] })
}

function applyTheme(mode: string) {
  let isDark = mode === 'dark'
  if (mode === 'system') {
    isDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  }
  document.documentElement.classList.toggle('dark', isDark)
}

function setThemeMode(mode: string) {
  themeMode.value = mode
  applyTheme(mode)
  saveSettings()
}

function setColorTheme(theme: string) {
  colorTheme.value = theme
  saveSettings()
}

function onMouseEnter() {
  invoke('submenu_enter')
}

function onMouseLeave() {
  console.log('Submenu mouse leave triggered!')
  invoke('hide_menu_window')
  invoke('hide_submenu_window')
}

watch(opacity, saveSettings)
watch(ballSize, saveSettings)

onMounted(() => {
  loadSettings()
  applyTheme(themeMode.value)
})
</script>

<template>
  <div
    class="submenu-wrapper"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
  >
    <Card class="submenu-card border-0 !py-0 !gap-0">
      <CardContent class="p-4 space-y-4">
        <!-- 主题模式 -->
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

        <!-- 透明度 -->
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

        <!-- 浮动球大小 -->
        <div class="space-y-2">
          <div class="flex justify-between items-center">
            <Label class="text-sm font-medium">浮动球大小</Label>
            <span class="text-xs text-muted-foreground tabular-nums">{{ ballSize[0] }}px</span>
          </div>
          <Slider
            v-model="ballSize"
            :min="30"
            :max="60"
            :step="1"
          />
        </div>

        <!-- 颜色主题 -->
        <div class="space-y-2">
          <Label class="text-xs uppercase text-muted-foreground font-medium">颜色主题</Label>
          <div class="flex gap-3 justify-between">
            <button
              v-for="theme in colorThemes"
              :key="theme.id"
              :class="cn(
                'flex flex-col items-center gap-1.5 p-1.5 rounded-lg transition-all',
                colorTheme === theme.id ? 'bg-accent' : 'hover:bg-accent/50'
              )"
              @click="setColorTheme(theme.id)"
            >
              <span
                :class="cn(
                  'w-8 h-8 rounded-full shadow-md transition-all',
                  colorTheme === theme.id && 'ring-2 ring-primary ring-offset-2 ring-offset-background'
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
</template>

<style scoped>
.submenu-wrapper {
  padding: 4px;
  overflow: hidden;
  animation: submenu-enter 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.submenu-card {
  width: 236px;
  border-radius: 12px;
  background: var(--card);
}

@keyframes submenu-enter {
  from {
    opacity: 0;
    transform: scale(0.96) translateX(-4px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateX(0);
  }
}
</style>
