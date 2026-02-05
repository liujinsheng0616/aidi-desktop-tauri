<script setup lang="ts">
import { onMounted, onUnmounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const props = defineProps<{
  size?: number
  opacity?: number
  colorTheme?: string
}>()

const ballSize = computed(() => props.size || 48)
const ballOpacity = computed(() => (props.opacity ?? 100) / 100)

const gradients: Record<string, string> = {
  'cyan-purple': 'linear-gradient(135deg, #667eea 0%, #764ba2 50%, #f093fb 100%)',
  'ocean': 'linear-gradient(135deg, #0052d4 0%, #4364f7 50%, #6fb1fc 100%)',
  'forest': 'linear-gradient(135deg, #11998e 0%, #38ef7d 100%)',
  'fire': 'linear-gradient(135deg, #f12711 0%, #f5af19 100%)',
  'midnight': 'linear-gradient(135deg, #000000 0%, #1a1a1a 50%, #333333 100%)',
}

const gradient = computed(() => gradients[props.colorTheme || 'cyan-purple'])

// 拖拽状态
let isDragging = false
let startX = 0
let startY = 0

// hover 状态
let hoverTimeout: number | null = null
let hideDockTimeout: number | null = null
let hoverVersion = 0

// 鼠标按下 - 开始拖拽
function handleMouseDown(e: MouseEvent) {
  if (e.button !== 0) return

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
  invoke('start_drag')

  isDragging = true
  startX = e.screenX
  startY = e.screenY

  document.addEventListener('mousemove', handleMouseMove)
  document.addEventListener('mouseup', handleMouseUp)
}

// 鼠标进入 - 触发吸附弹出
function handleMouseEnter() {
  if (isDragging) return

  const currentVersion = ++hoverVersion

  // 取消隐藏定时器
  if (hideDockTimeout) {
    clearTimeout(hideDockTimeout)
    hideDockTimeout = null
  }

  // 通知后端：鼠标进入，触发弹出
  invoke('ball_enter')

  // 延迟显示菜单
  hoverTimeout = window.setTimeout(() => {
    if (hoverVersion === currentVersion && !isDragging) {
      invoke('show_menu')
    }
  }, 400)
}

// 鼠标离开 - 延迟吸附回去
function handleMouseLeave() {
  if (isDragging) return

  const currentVersion = ++hoverVersion

  if (hoverTimeout) {
    clearTimeout(hoverTimeout)
    hoverTimeout = null
  }

  // 通知后端：鼠标离开
  invoke('ball_leave')

  // 立即隐藏菜单
  console.log('Ball mouse leave - hiding menu')
  invoke('hide_menu_window')

  // 延迟隐藏吸附球
  hideDockTimeout = window.setTimeout(() => {
    if (hoverVersion === currentVersion) {
      invoke('hide_docked_ball')
    }
  }, 300)
}

// 鼠标移动
function handleMouseMove(e: MouseEvent) {
  if (!isDragging) return

  const dx = e.screenX - startX
  const dy = e.screenY - startY
  startX = e.screenX
  startY = e.screenY

  invoke('move_window_by', { dx, dy })
}

// 鼠标松开
function handleMouseUp() {
  if (!isDragging) return

  isDragging = false
  document.removeEventListener('mousemove', handleMouseMove)
  document.removeEventListener('mouseup', handleMouseUp)

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
    class="ball-container"
    :style="{ opacity: ballOpacity }"
    @mousedown="handleMouseDown"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
  >
    <div class="outer-ring" :style="{ width: ballSize + 8 + 'px', height: ballSize + 8 + 'px' }"></div>
    <div class="inner-ring" :style="{ width: ballSize + 4 + 'px', height: ballSize + 4 + 'px' }"></div>
    <div
      class="floating-ball"
      :style="{ width: ballSize + 'px', height: ballSize + 'px' }"
    >
      <div class="ball-inner">
        <div class="ball-glow"></div>
        <div class="ball-highlight"></div>
        <img src="/src/assets/icon.svg" class="ball-icon" alt="AI" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.ball-container {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  cursor: grab;
  user-select: none;
  -webkit-user-select: none;
  -webkit-app-region: no-drag;
}

.ball-container:active {
  cursor: grabbing;
}

.outer-ring {
  position: absolute;
  border-radius: 50%;
  background: conic-gradient(
    from 0deg,
    transparent 0deg,
    rgba(0, 212, 255, 0.5) 60deg,
    transparent 120deg,
    rgba(124, 58, 237, 0.5) 180deg,
    transparent 240deg,
    rgba(244, 114, 182, 0.5) 300deg,
    transparent 360deg
  );
  -webkit-mask: radial-gradient(transparent 65%, black 67%, black 75%, transparent 77%);
  mask: radial-gradient(transparent 65%, black 67%, black 75%, transparent 77%);
  opacity: 0.6;
  transition: opacity 0.3s ease;
  pointer-events: none;
}

.inner-ring {
  position: absolute;
  border-radius: 50%;
  background: conic-gradient(
    from 180deg,
    rgba(0, 212, 255, 0.7) 0deg,
    rgba(124, 58, 237, 0.7) 120deg,
    rgba(244, 114, 182, 0.7) 240deg,
    rgba(0, 212, 255, 0.7) 360deg
  );
  -webkit-mask: radial-gradient(transparent 75%, black 77%, black 90%, transparent 92%);
  mask: radial-gradient(transparent 75%, black 77%, black 90%, transparent 92%);
  opacity: 0.7;
  transition: opacity 0.3s ease;
  pointer-events: none;
}

.floating-ball {
  position: relative;
  border-radius: 50%;
  z-index: 1;
  pointer-events: none;
}

.ball-inner {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: v-bind(gradient);
  position: relative;
  overflow: hidden;
}

.ball-glow {
  position: absolute;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: radial-gradient(
    circle at 30% 30%,
    rgba(255, 255, 255, 0.5) 0%,
    transparent 50%
  );
  pointer-events: none;
}

.ball-highlight {
  position: absolute;
  width: 6px;
  height: 6px;
  background: rgba(255, 255, 255, 0.9);
  border-radius: 50%;
  top: 20%;
  left: 25%;
  pointer-events: none;
}

.ball-icon {
  position: absolute;
  width: 70%;
  height: 70%;
  top: 15%;
  left: 15%;
  z-index: 2;
  filter: drop-shadow(0 0 8px rgba(255, 255, 255, 0.3));
  pointer-events: none;
  -webkit-user-drag: none;
  user-select: none;
}

.ball-container:hover .outer-ring {
  opacity: 1;
  animation: rotate-cw 3s linear infinite;
}

.ball-container:hover .inner-ring {
  opacity: 1;
  animation: rotate-ccw 2s linear infinite;
}

@keyframes rotate-cw {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@keyframes rotate-ccw {
  from { transform: rotate(360deg); }
  to { transform: rotate(0deg); }
}
</style>
