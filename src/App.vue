<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import FloatingBall from './components/FloatingBall.vue'
import { isLoggedIn, getToken, fetchCurrentUser, setUser, getUser } from './stores/auth'
import { WebviewWindow, getAllWebviewWindows } from '@tauri-apps/api/webviewWindow'

const ballSize = ref(60)
const opacity = ref(100)
const colorTheme = ref('cyan-purple')
const initialized = ref(false)

function loadSettings() {
  const saved = localStorage.getItem('aidi-settings')
  if (saved) {
    const s = JSON.parse(saved)
    ballSize.value = s.ballSize || 60
    opacity.value = s.opacity ?? 100
    colorTheme.value = s.colorTheme || 'cyan-purple'
  }
}

async function initApp() {
  loadSettings()
  // 每次启动悬浮球时刷新用户信息缓存，失败不阻断启动
  const token = getToken()
  if (token) {
    fetchCurrentUser(token).then(setUser).catch(() => {})
  }
  // 先同步窗口大小，消除 tauri.conf.json 初始 120×120 与 ballSize 的不一致
  await invoke('update_window_size', { size: ballSize.value })
  initialized.value = true
  await invoke('show_main_window')
}

onMounted(async () => {
  listen('settings-updated', (event: any) => {
    const settings = event.payload as any
    // Handle both camelCase (from localStorage) and snake_case (from backend)
    if (settings.ball_size) ballSize.value = settings.ball_size
    if (settings.ballSize) ballSize.value = settings.ballSize
    if (settings.opacity !== undefined) opacity.value = settings.opacity
    if (settings.color_theme) colorTheme.value = settings.color_theme
    if (settings.colorTheme) colorTheme.value = settings.colorTheme
  })

  // 监听登录完成事件
  listen('login-complete', async () => {
    await invoke('close_login_window')
    await initApp()
  })

  // 监听托盘"打开AIDI"事件
  listen('open-aigc', async () => {
    const fsUserId = getUser()?.fsUserId ?? ''
    const aigcUrl = `https://aidi.yadea.com.cn/aigc/#/login?userId=${fsUserId}`
    const windows = await getAllWebviewWindows()
    const existing = windows.find(w => w.label === 'aigc-window')
    if (existing) {
      await (existing as any).navigate(aigcUrl)
      await existing.show()
      await existing.setFocus()
    } else {
      const webview = new WebviewWindow('aigc-window', {
        url: aigcUrl,
        title: 'AIGC',
        width: 1200,
        height: 800,
        center: true,
        decorations: true,
        resizable: true,
        alwaysOnTop: false,
      })
      webview.once('tauri://error', (e) => console.error('Error creating aigc window:', e))
    }
  })

  if (isLoggedIn()) {
    await initApp()
  } else {
    await invoke('show_login_window')
  }
})
</script>

<template>
  <div class="app">
    <FloatingBall v-if="initialized" :size="ballSize" :opacity="opacity" :colorTheme="colorTheme" />
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
  pointer-events: none;   /* 透明 padding 区域鼠标穿透到桌面 */
}
</style>
