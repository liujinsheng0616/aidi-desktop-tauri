<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import FloatingBall from './components/FloatingBall.vue'
import QuickInputBox from './components/QuickInputBox.vue'
import { isLoggedIn, getToken, fetchCurrentUser, setUser, getUser } from './stores/auth'
import { WebviewWindow, getAllWebviewWindows } from '@tauri-apps/api/webviewWindow'
import { colorThemes } from './shared/colorThemes'

const ballSize = ref(60)
const opacity = ref(100)
const colorTheme = ref('cyan-purple')
const initialized = ref(false)
const inputBoxHeight = ref(60) // 输入框动态高度
const isInputExpanded = ref(false) // 输入框展开状态

// 胶囊容器高度（随输入框动态变化）
const capsuleHeight = computed(() => {
  if (inputBoxHeight.value > ballSize.value) {
    return inputBoxHeight.value
  }
  return ballSize.value
})

// 胶囊容器宽度（显式控制，用于动画）
const shellWidth = ref(99) // 默认收起态宽度

// 胶囊容器样式
const capsuleStyle = computed(() => {
  const h = capsuleHeight.value
  const currentTheme = colorThemes[colorTheme.value] || colorThemes['cyan-purple']
  return {
    width: `${shellWidth.value}px`,
    height: `${h}px`,
    borderRadius: `${Math.min(h / 2, 40)}px`, // 限制最大圆角
    '--theme-primary': currentTheme.primary,
    '--theme-glow': currentTheme.glow,
  }
})

async function handleInputExpand(expanded: boolean) {
  isInputExpanded.value = expanded
  if (expanded) {
    shellWidth.value = 303 // 展开态
    await invoke('expand_input_window')
  } else {
    // 先让前端宽度动画完成
    shellWidth.value = 99
    // 等待 CSS 动画完成（200ms）后再收窄窗口
    setTimeout(() => invoke('collapse_input_window'), 200)
  }
}

// 监听输入框高度变化，调整窗口高度
async function handleInputHeightChange(height: number) {
  inputBoxHeight.value = height
  await invoke('resize_input_window_height', { height })
}

let isCreatingAigcWindow = false
let unlistenOpenAigc: (() => void) | null = null

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
      // 同步失败，忽略
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
  unlistenOpenAigc = await listen('open-aigc', async () => {
    if (isCreatingAigcWindow) return
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
      isCreatingAigcWindow = true
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
      webview.once('tauri://created', () => { isCreatingAigcWindow = false })
      webview.once('tauri://error', (e) => {
        isCreatingAigcWindow = false
      })
    }
  })

  // 监听 Rust 超时线程的重新检查请求（处理 Windows 上 WebView2 慢初始化场景）
  listen('request-login-check', async () => {
    if (initialized.value) return  // 已初始化，跳过
    const loggedIn = isLoggedIn()
    await invoke('update_login_status', { isLoggedIn: loggedIn })
    if (loggedIn) {
      await initApp()
    } else {
      await invoke('show_login_window')
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

onUnmounted(() => {
  unlistenOpenAigc?.()
})
</script>

<template>
  <div class="app">
    <div v-if="initialized" class="pill-shell" :style="capsuleStyle">
      <div class="floating-ball-wrapper" :style="{ width: `${ballSize}px`, height: `${ballSize}px` }">
        <FloatingBall :size="ballSize" :opacity="opacity" :colorTheme="colorTheme" :isInputExpanded="isInputExpanded" />
      </div>
      <div class="pill-divider"></div>
      <QuickInputBox :size="ballSize" :opacity="opacity" :colorTheme="colorTheme" @expand="handleInputExpand" @heightChange="handleInputHeightChange" />
    </div>
  </div>
</template>

<style scoped>
.app {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding-left: 0;
  background: transparent;
  pointer-events: none;
}

/* 统一胶囊容器 - 微信深灰风格 */
.pill-shell {
  display: flex;
  align-items: center;
  gap: 0;
  pointer-events: auto;
  position: relative;
  /* 深灰色纯色背景，无毛玻璃 */
  background: rgba(30, 30, 30, 0.85);
  /* 细微边框 */
  border: 1px solid rgba(255, 255, 255, 0.08);
  /* 平滑过渡 */
  transition:
    background 220ms ease,
    transform 150ms ease,
    width 200ms cubic-bezier(0.16, 1, 0.3, 1),
    height 150ms ease;
  overflow: visible;
}

.pill-shell:hover {
  background: rgba(40, 40, 40, 0.9);
  transform: translateY(-1px);
}

.pill-shell:active {
  transform: translateY(0px) scale(0.99);
}

/* 球和搜索之间的分割线 */
.pill-divider {
  width: 1px;
  height: 36%;
  background: rgba(255, 255, 255, 0.1);
  flex-shrink: 0;
  pointer-events: none;
}

.floating-ball-wrapper {
  flex-shrink: 0;
  position: relative;
}
</style>
