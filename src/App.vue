<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import FloatingBall from './components/FloatingBall.vue'

const ballSize = ref(48)
const opacity = ref(100)
const colorTheme = ref('cyan-purple')

function loadSettings() {
  const saved = localStorage.getItem('aidi-settings')
  if (saved) {
    const s = JSON.parse(saved)
    ballSize.value = s.ballSize || 48
    opacity.value = s.opacity ?? 100
    colorTheme.value = s.colorTheme || 'cyan-purple'
  }
}

onMounted(() => {
  // 临时清除旧设置，确保使用默认值
  localStorage.removeItem('aidi-settings')
  loadSettings()

  // Listen for settings updates from other windows
  listen('settings-updated', (event: any) => {
    const settings = event.payload as any
    // Handle both camelCase (from localStorage) and snake_case (from backend)
    if (settings.ball_size) ballSize.value = settings.ball_size
    if (settings.ballSize) ballSize.value = settings.ballSize
    if (settings.opacity !== undefined) opacity.value = settings.opacity
    if (settings.color_theme) colorTheme.value = settings.color_theme
    if (settings.colorTheme) colorTheme.value = settings.colorTheme
  })
})
</script>

<template>
  <div class="app">
    <FloatingBall
      :size="ballSize"
      :opacity="opacity"
      :colorTheme="colorTheme"
    />
  </div>
</template>

<style scoped>
.app {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  overflow: hidden;
}
</style>
