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

// 登录完成后的初始化逻辑（供 Rust 端调用）
async function handleLoginComplete() {
  await invoke('update_login_status', { isLoggedIn: true })
  await initApp()
  await syncAuthToBackend()
}

// 暴露全局函数供 Rust 调用
if (typeof window !== 'undefined') {
  (window as any).__aidiHandleLoginComplete = handleLoginComplete
}

/** 同步认证信息到 Rust 后端（用于后台静默上报） */
async function syncAuthToBackend() {
  const token = getToken()
  const user = getUser()
  if (token && user) {
    try {
      // 同步 token
      await invoke('set_auth_token', { token })
      // 同步用户信息（userCode 使用 fsUserId）
      await invoke('set_report_user_info', {
        userCode: user.fsUserId || user.id,
        userName: user.name || user.nickName
      })
    } catch (e) {
      console.error('同步认证信息到后端失败:', e)
    }
  }
}

async function initApp() {
  loadSettings()
  // 每次启动悬浮球时刷新用户信息缓存，失败不阻断启动
  const token = getToken()
  if (token) {
    fetchCurrentUser(token).then(async (user) => {
      setUser(user)
      // 同步认证信息到 Rust 后端
      await syncAuthToBackend()
    }).catch(() => {})
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
    // 登录成功后更新托盘菜单状态
    await invoke('update_login_status', { isLoggedIn: true })
    await initApp()
    // 登录完成后同步认证信息到后端
    await syncAuthToBackend()
  })

  // 监听托盘"打开AIDI"事件
  listen('open-aigc', async () => {
    const fsUserId = getUser()?.fsUserId ?? ''
    const appDomain = import.meta.env.VITE_APP_DOMAIN || 'https://aidi.yadea.com.cn'
    const aigcUrl = `${appDomain}/aigc/#/login?userId=${fsUserId}`
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

  const loggedIn = isLoggedIn()
  // 应用启动时同步登录状态到 Rust 端（用于托盘菜单显示）
  await invoke('update_login_status', { isLoggedIn: loggedIn })

  if (loggedIn) {
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
