<script setup lang="ts">
import { onMounted, onUnmounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, emit } from '@tauri-apps/api/event'
import { WebviewWindow, getAllWebviewWindows } from '@tauri-apps/api/webviewWindow'
import { getUser } from '../stores/auth'
import { colorThemes } from '../shared/colorThemes'

const props = defineProps<{
  size?: number
  opacity?: number
  colorTheme?: string
  isInputExpanded?: boolean
}>()

// 开发模式：保留 devtools 右键菜单；生产模式：右键唤起 AIDI 菜单
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
let dragStartWindowX = 0 // 拖拽开始时的窗口物理坐标（由 prepare_drag 返回）
let dragStartWindowY = 0
let dragWindowReady = false // prepare_drag 是否已返回初始坐标
let hasMoved = false
const CLICK_THRESHOLD = 5 // 移动超过5像素认为是拖拽
const CLICK_TIME_THRESHOLD = 350 // 按下超过350ms认为是拖拽（Windows IPC延迟较大，200ms过严）

// 双击检测
let lastClickTime = 0
const DOUBLE_CLICK_THRESHOLD = 300 // 双击间隔时间（毫秒）

// hover 状态
let hoverTimeout: number | null = null
let hideDockTimeout: number | null = null
let hoverVersion = 0

// 右键菜单处理：生产模式阻止原生菜单（自定义菜单由 handleMouseDown 触发）
function handleContextMenu(e: MouseEvent) {
  if (!isDev) {
    e.preventDefault()
  }
}

// 鼠标按下 - 开始拖拽
async function handleMouseDown(e: MouseEvent) {
  // 右键：生产模式唤起 AIDI 菜单，开发模式保留 devtools 右键菜单
  if (e.button === 2) {
    if (!isDev && !props.isInputExpanded) {
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

  // 重置就绪标志（handleMouseMove 会等待 prepare_drag 返回后才开始移动）
  dragWindowReady = false

  // 先挂载事件监听，不等待 IPC 返回，确保双击检测不被阻断
  document.addEventListener('mousemove', handleMouseMove)
  document.addEventListener('mouseup', handleMouseUp)

  // await prepare_drag 取得窗口初始物理坐标，之后 mousemove 使用绝对坐标，无需 outer_position
  const [wx, wy] = await invoke<[number, number]>('prepare_drag')
  dragStartWindowX = wx
  dragStartWindowY = wy
  dragWindowReady = true
}

// 拖拽节流状态
let lastDragTime = 0
const DRAG_THROTTLE_MS = 8 // 约 120fps，足够流畅

// 鼠标移动 - 自定义拖拽逻辑（使用绝对坐标，Windows 上减少 Win32 API 调用）
function handleMouseMove(e: MouseEvent) {
  if (!isDragging) return

  // 等待 prepare_drag 返回初始坐标后再执行移动
  if (!dragWindowReady) return

  // 节流：限制调用频率
  const now = Date.now()
  if (now - lastDragTime < DRAG_THROTTLE_MS) return
  lastDragTime = now

  // 计算总偏移量（从拖拽起点到当前鼠标位置）
  const totalDx = e.screenX - dragStartMouseX
  const totalDy = e.screenY - dragStartMouseY

  // 检测是否移动了（用于区分点击和拖拽）
  if (Math.abs(totalDx) > CLICK_THRESHOLD || Math.abs(totalDy) > CLICK_THRESHOLD) {
    hasMoved = true
  }

  // 计算绝对目标坐标（物理像素），不依赖 outer_position，消除 Rust 侧额外 Win32 调用
  const dpr = window.devicePixelRatio || 1
  const newX = Math.round(dragStartWindowX + totalDx * dpr)
  const newY = Math.round(dragStartWindowY + totalDy * dpr)
  invoke('move_window_to', { x: newX, y: newY })
}

// 鼠标进入 - 触发吸附弹出
function handleMouseEnter() {
  if (isDragging) return
  if (props.isInputExpanded) return // 输入框展开时不显示菜单

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
            // 窗口创建成功
          })
          webview.once('tauri://error', () => {
            // 窗口创建失败，忽略
          })
        }
      } catch (error) {
        // 处理窗口错误，忽略
      }
    } else {
      // 记录本次点击时间，等待可能的第二次点击
      lastClickTime = now
      // 单击悬浮球时，通知收起输入框
      emit('collapse-input')
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
  overflow: hidden;        /* 隐藏圆角外的内容 */
  border-radius: 50%;      /* 添加圆形（Windows 上替代 SetWindowRgn） */
  background: transparent; /* 确保背景透明 */
  pointer-events: auto;    /* 父容器穿透后，球体恢复响应鼠标事件 */
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

/* 主球体 */
.ball {
  position: absolute;
  width: 50px;
  height: 50px;
  left: 5px;
  top: 5px;
  border-radius: 50%;
  background: #FF6B6B;
  pointer-events: none;
}

/* 内圈细线 */
.inner-ring {
  position: absolute;
  width: 40px;
  height: 40px;
  left: 10px;
  top: 10px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.25);
  pointer-events: none;
}

/* AIDI 文字 */
.aidi-text {
  position: absolute;
  width: 50px;
  height: 50px;
  left: 5px;
  top: 5px;
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
