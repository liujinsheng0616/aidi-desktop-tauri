<script setup lang="ts">
import { onMounted, onUnmounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { WebviewWindow, getAllWebviewWindows } from '@tauri-apps/api/webviewWindow'
import { getUser } from '../stores/auth'

const props = defineProps<{
  size?: number
  opacity?: number
  colorTheme?: string
}>()

// 颜色主题配置
const colorThemes: Record<string, { primary: string; glow: string }> = {
  'cyan-purple': { primary: '#667eea', glow: 'rgba(102, 126, 234, 0.4)' },
  'ocean': { primary: '#0052d4', glow: 'rgba(0, 82, 212, 0.4)' },
  'forest': { primary: '#11998e', glow: 'rgba(17, 153, 142, 0.4)' },
  'fire': { primary: '#f12711', glow: 'rgba(241, 39, 17, 0.4)' },
  'midnight': { primary: '#1a1a1a', glow: 'rgba(50, 50, 50, 0.4)' },
}

// 判断是否为开发模式
const isDev = import.meta.env.DEV

// 使用设计规范中的 120px 作为基准尺寸
const ballSize = computed(() => props.size || 60)
const ballOpacity = computed(() => (props.opacity ?? 100) / 100)
const ballScale = computed(() => ballSize.value / 60)
const currentTheme = computed(() => colorThemes[props.colorTheme || 'cyan-purple'] || colorThemes['cyan-purple'])

// 拖拽状态
let isDragging = false
let dragStartTime = 0
let dragStartMouseX = 0 // 拖拽开始时的鼠标位置
let dragStartMouseY = 0
let hasMoved = false
const CLICK_THRESHOLD = 5 // 移动超过5像素认为是拖拽
const CLICK_TIME_THRESHOLD = 200 // 按下超过200ms认为是拖拽

// 双击检测
let lastClickTime = 0
const DOUBLE_CLICK_THRESHOLD = 300 // 双击间隔时间（毫秒）

// hover 状态
let hoverTimeout: number | null = null
let hideDockTimeout: number | null = null
let hoverVersion = 0

// 右键菜单处理 - 开发模式下保留默认行为以支持 Inspect
function handleContextMenu(e: MouseEvent) {
  if (!isDev) {
    e.preventDefault()
    // 自定义菜单在 handleMouseDown 中已处理
  }
  // 开发模式下不阻止默认行为，允许打开 DevTools
}

// 鼠标按下 - 开始拖拽
async function handleMouseDown(e: MouseEvent) {
  // 右键显示菜单（仅生产模式）
  if (e.button === 2) {
    if (!isDev) {
      e.preventDefault()
      invoke('show_menu')
    }
    return
  }

  if (e.button !== 0) return

  // 立即标记为拖拽中，防止竞态
  isDragging = true
  hasMoved = false
  dragStartTime = Date.now()

  // 清除 hover 定时器
  hoverVersion++
  if (hoverTimeout) {
    clearTimeout(hoverTimeout)
    hoverTimeout = null
  }
  if (hideDockTimeout) {
    clearTimeout(hideDockTimeout)
    hideDockTimeout = null
  }

  invoke('hide_menu')

  // 记录拖拽开始时的鼠标位置
  dragStartMouseX = e.screenX
  dragStartMouseY = e.screenY
  lastMouseX = e.screenX
  lastMouseY = e.screenY

  // 通知后端准备拖拽（只更新状态，不移动窗口）
  await invoke('prepare_drag')
  // 初始化后端拖拽位置（使用增量更新模式）
  await invoke('start_drag')

  // 使用自定义拖拽逻辑，而不是 Tauri 的 startDragging()
  // 这样可以避免吸附状态下的"弹跳"问题
  document.addEventListener('mousemove', handleMouseMove)
  document.addEventListener('mouseup', handleMouseUp)
}

// 拖拽节流状态
let lastDragTime = 0
let lastMouseX = 0
let lastMouseY = 0
const DRAG_THROTTLE_MS = 8 // 约 120fps，足够流畅

// 鼠标移动 - 自定义拖拽逻辑（使用增量位置更新优化）
function handleMouseMove(e: MouseEvent) {
  if (!isDragging) return

  // 节流：限制调用频率
  const now = Date.now()
  if (now - lastDragTime < DRAG_THROTTLE_MS) return
  lastDragTime = now

  // 计算增量（相对于上次 mousemove）
  const dx = e.screenX - lastMouseX
  const dy = e.screenY - lastMouseY
  lastMouseX = e.screenX
  lastMouseY = e.screenY

  // 检测是否移动了（用于区分点击和拖拽）
  const totalDx = e.screenX - dragStartMouseX
  const totalDy = e.screenY - dragStartMouseY
  if (Math.abs(totalDx) > CLICK_THRESHOLD || Math.abs(totalDy) > CLICK_THRESHOLD) {
    hasMoved = true
  }

  // 使用增量更新（后端原子操作，更快速）
  // 乘以 devicePixelRatio 将逻辑像素转换为物理像素，与 outer_position() 的坐标体系一致
  const dpr = window.devicePixelRatio || 1
  invoke('move_window_by', { dx: Math.round(dx * dpr), dy: Math.round(dy * dpr) })
}

// 鼠标进入 - 触发吸附弹出
function handleMouseEnter() {
  if (isDragging) return

  // 取消隐藏定时器
  if (hideDockTimeout) {
    clearTimeout(hideDockTimeout)
    hideDockTimeout = null
  }

  // 通知后端：鼠标进入，触发弹出
  invoke('ball_enter')

  // 延迟500ms后显示菜单
  hoverTimeout = window.setTimeout(() => {
    if (!isDragging) {
      invoke('show_menu')
    }
  }, 500)
}

// 鼠标离开 - 延迟吸附回去
function handleMouseLeave() {
  const currentVersion = ++hoverVersion

  // 取消显示菜单的定时器
  if (hoverTimeout) {
    clearTimeout(hoverTimeout)
    hoverTimeout = null
  }

  // 通知后端：鼠标离开（即使正在拖拽也要通知，用于边缘吸附）
  invoke('ball_leave')

  // 如果正在拖拽，不隐藏菜单和吸附球
  if (isDragging) return

  // 立即隐藏菜单
  invoke('hide_menu_window')

  // 延迟隐藏吸附球
  hideDockTimeout = window.setTimeout(() => {
    if (hoverVersion === currentVersion) {
      invoke('hide_docked_ball')
    }
  }, 300)
}

// 鼠标松开
async function handleMouseUp() {
  if (!isDragging) return

  isDragging = false
  document.removeEventListener('mousemove', handleMouseMove)
  document.removeEventListener('mouseup', handleMouseUp)

  // 检测是否为双击（没有移动且时间很短）
  const clickDuration = Date.now() - dragStartTime
  if (!hasMoved && clickDuration < CLICK_TIME_THRESHOLD) {
    const now = Date.now()
    const timeSinceLastClick = now - lastClickTime

    if (timeSinceLastClick < DOUBLE_CLICK_THRESHOLD) {
      // 是双击，打开窗口并重置计时器
      lastClickTime = 0

      try {
        const windows = await getAllWebviewWindows()
        const existingWindow = windows.find(w => w.label === 'aigc-window')

        const fsUserId = getUser()?.fsUserId ?? ''
        const appDomain = import.meta.env.VITE_APP_DOMAIN || 'https://aidi.yadea.com.cn'
        const aigcUrl = `${appDomain}/aigc/#/login?userId=${fsUserId}`

        if (existingWindow) {
          // 窗口已存在，更新 URL 后显示并聚焦
          await (existingWindow as any).navigate(aigcUrl)
          await existingWindow.show()
          await existingWindow.setFocus()
        } else {
          // 窗口不存在，创建新窗口
          const webview = new WebviewWindow('aigc-window', {
            url: aigcUrl,
            title: 'AIGC',
            width: 1200,
            height: 800,
            center: true,
            decorations: true,
            resizable: true,
            alwaysOnTop: false
          })
          webview.once('tauri://created', () => {
            console.log('Webview window created')
          })
          webview.once('tauri://error', (e) => {
            console.error('Error creating webview window:', e)
          })
        }
      } catch (error) {
        console.error('Error handling window:', error)
      }
    } else {
      // 记录本次点击时间，等待可能的第二次点击
      lastClickTime = now
    }
  }

  // 拖拽结束，检测边缘吸附
  invoke('drag_end')
}

let unlistenCancelDockHide: (() => void) | null = null

onMounted(async () => {
  unlistenCancelDockHide = await listen('cancel-dock-hide', () => {
    hoverVersion++
    if (hideDockTimeout) {
      clearTimeout(hideDockTimeout)
      hideDockTimeout = null
    }
  })
})

onUnmounted(() => {
  document.removeEventListener('mousemove', handleMouseMove)
  document.removeEventListener('mouseup', handleMouseUp)
  if (hoverTimeout) clearTimeout(hoverTimeout)
  if (hideDockTimeout) clearTimeout(hideDockTimeout)
  if (unlistenCancelDockHide) unlistenCancelDockHide()
})
</script>

<template>
  <div
    class="floating-ball"
    :style="{
      width: '100%',
      height: '100%',
      opacity: ballOpacity
    }"
    @mousedown="handleMouseDown"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    @contextmenu="handleContextMenu"
  >
    <div class="ball-content" :style="{ transform: `scale(${ballScale})` }">
      <!-- 外圈光环 - 呼吸动画 -->
      <div class="glow-ring" :style="{ borderColor: currentTheme.glow }"></div>

      <!-- 主球体 -->
      <div class="ball" :style="{ background: currentTheme.primary }"></div>

      <!-- 内圈细线 -->
      <div class="inner-ring"></div>

      <!-- AIDI 文字 -->
      <div class="aidi-text">AIDI</div>

      <!-- 下划线装饰 -->
      <div class="underline"></div>

      <!-- 装饰点 -->
      <div class="accent-dot"></div>
    </div>
  </div>
</template>

<style scoped>
/* 悬浮球容器 */
.floating-ball {
  position: relative;
  cursor: grab;
  user-select: none;
  -webkit-user-select: none;
  -webkit-app-region: no-drag;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: visible;
  pointer-events: auto;   /* 父容器穿透后，球体恢复响应鼠标事件 */
}

/* 内部内容容器 - 固定 60px 基准尺寸 */
.ball-content {
  position: relative;
  width: 60px;
  height: 60px;
  flex-shrink: 0;
  transform-origin: center center;
}

.floating-ball:active {
  cursor: grabbing;
}

/* 外圈光环 - 呼吸动画 */
.glow-ring {
  position: absolute;
  width: 55px;
  height: 55px;
  left: 2.5px;
  top: 2.5px;
  border-radius: 50%;
  border: 1.5px solid rgba(255, 107, 107, 0.4);
  animation: breathe 2s ease-in-out infinite;
  pointer-events: none;
}

/* 主球体 */
.ball {
  position: absolute;
  width: 45px;
  height: 45px;
  left: 7.5px;
  top: 7.5px;
  border-radius: 50%;
  background: #FF6B6B;
  pointer-events: none;
}

/* 内圈细线 */
.inner-ring {
  position: absolute;
  width: 35px;
  height: 35px;
  left: 12.5px;
  top: 12.5px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.25);
  pointer-events: none;
}

/* AIDI 文字 */
.aidi-text {
  position: absolute;
  width: 45px;
  height: 45px;
  left: 7.5px;
  top: 7.5px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: 'DM Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  font-size: 10px;
  font-weight: 700;
  color: #FFFFFF;
  letter-spacing: 0.5px;
  pointer-events: none;
}

/* 下划线装饰 */
.underline {
  position: absolute;
  width: 20px;
  height: 1px;
  left: 20px;
  top: 35px;
  border-radius: 0.5px;
  background: rgba(255, 255, 255, 0.33);
  pointer-events: none;
}

/* 装饰点 */
.accent-dot {
  position: absolute;
  width: 2px;
  height: 2px;
  left: 41px;
  top: 34px;
  border-radius: 50%;
  background: #FFFFFF;
  pointer-events: none;
}

/* 呼吸动画 */
@keyframes breathe {
  0%, 100% {
    opacity: 0.6;
    transform: scale(1);
  }
  50% {
    opacity: 1;
    transform: scale(1.05);
  }
}

/* 悬停效果 - 加速呼吸 */
.floating-ball:hover .glow-ring {
  animation-duration: 1s;
}

/* 点击效果 */
.floating-ball:active .ball-content {
  transform: scale(0.95);
}

/* 可访问性 - 减少动画 */
@media (prefers-reduced-motion: reduce) {
  .glow-ring {
    animation: none;
  }
}
</style>
