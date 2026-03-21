<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, emit as emitTauriEvent, UnlistenFn } from '@tauri-apps/api/event'

const props = defineProps<{
  size?: number
  opacity?: number
  colorTheme?: string
}>()

const emit = defineEmits<{
  expand: [boolean]
  heightChange: [number]
}>()

// 状态
const isExpanded = ref(false)
const inputText = ref('')
const inputRef = ref<HTMLTextAreaElement | null>(null)
const textareaHeight = ref(props.size || 60) // 动态高度，初始等于球高度
const savedHeight = ref(0) // 保存收起前的高度

// 事件监听器清理函数
let unlistenCollapse: UnlistenFn | null = null

// IME 组合输入状态（e.isComposing 在 Tauri WebView 里不可靠）
let imeComposing = false

const ballSize = computed(() => props.size || 60)

// 点击搜索按钮 - 展开/收起输入框
function toggleInput() {
  isExpanded.value = !isExpanded.value
  emit('expand', isExpanded.value)
  if (isExpanded.value) {
    // 创建聊天窗口但保持隐藏，由 ChatView 决定是否显示
    invoke('show_chat_window', { initialMessage: null, visible: false }).catch(() => {})
    // 通知 ChatView 检查是否需要显示聊天窗口（使用 Tauri 全局事件）
    emitTauriEvent('input-expanded').catch(() => {})

    // 展开后自动聚焦输入框并调整高度
    setTimeout(() => {
      inputRef.value?.focus()
      // 优先恢复保存的高度
      if (savedHeight.value > 0) {
        textareaHeight.value = savedHeight.value
        // 同步设置 textarea 元素高度（容器高度 - 24px padding）
        if (inputRef.value) {
          const textareaActualHeight = savedHeight.value - 24
          inputRef.value.style.height = `${textareaActualHeight}px`
          // 判断是否需要滚动条
          inputRef.value.style.overflowY = textareaActualHeight >= 100 ? 'auto' : 'hidden'
        }
        emit('heightChange', textareaHeight.value)
      } else if (inputText.value) {
        // 没有保存高度但有内容时，重新计算
        autoResize()
      }
    }, 50)
  } else {
    // 收起时：隐藏聊天窗口
    invoke('hide_chat_window').catch(() => {})
  }
}

// 点击外部收起（但不收起拖动浮动球时的点击）
function handleClickOutside(e: MouseEvent) {
  const target = e.target as HTMLElement
  // 如果点击的是浮动球区域（拖动操作），不收起输入框
  if (target.closest('.floating-ball')) {
    return
  }
  if (!target.closest('.quick-input-container')) {
    collapseInput()
  }
}

// 收起输入框
function collapseInput() {
  // 保存当前高度（非初始高度时才保存）
  if (textareaHeight.value > ballSize.value) {
    savedHeight.value = textareaHeight.value
  }
  isExpanded.value = false
  textareaHeight.value = ballSize.value // 重置高度
  // 重置 textarea 样式
  if (inputRef.value) {
    inputRef.value.style.height = 'auto'
    inputRef.value.style.overflowY = 'hidden'
  }
  emit('expand', false)
  emit('heightChange', textareaHeight.value) // 通知父组件重置高度
  // 收起时隐藏聊天窗口
  invoke('hide_chat_window').catch(() => {})
}

// 自动调整容器高度
function autoResize() {
  const textarea = inputRef.value
  if (!textarea) return

  // 先重置高度以获取真实的 scrollHeight
  textarea.style.height = 'auto'
  const scrollHeight = textarea.scrollHeight
  const maxHeight = 100 // 约 5 行

  let textareaActualHeight: number
  if (scrollHeight <= maxHeight) {
    textareaActualHeight = scrollHeight
    textarea.style.overflowY = 'hidden'
  } else {
    textareaActualHeight = maxHeight
    textarea.style.overflowY = 'auto'
  }

  // 设置 textarea 实际高度
  textarea.style.height = `${textareaActualHeight}px`

  // 计算容器高度（textarea 高度 + padding）
  textareaHeight.value = Math.max(ballSize.value, textareaActualHeight + 24)

  // 通知父组件高度变化
  emit('heightChange', textareaHeight.value)
}

// 发送消息
async function sendMessage() {
  const text = inputText.value.trim()
  if (!text) return

  try {
    // 调用 Rust 后端发送消息并显示聊天窗口
    await invoke('send_chat_message', { message: text })
    inputText.value = ''
    // 发送后重置高度
    textareaHeight.value = ballSize.value
    emit('heightChange', textareaHeight.value)
    // 发送后保持输入框展开，等待用户继续输入
    inputRef.value?.focus()
  } catch (error) {
    // 发送失败，忽略
  }
}

// 按键处理
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !imeComposing) {
    e.preventDefault()
    sendMessage()
  } else if (e.key === 'Escape') {
    collapseInput()
  }
}

onMounted(async () => {
  document.addEventListener('click', handleClickOutside)
  // 监听单击悬浮球事件，收起输入框
  unlistenCollapse = await listen('collapse-input', () => {
    if (isExpanded.value) {
      collapseInput()
    }
  })
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
  if (unlistenCollapse) {
    unlistenCollapse()
  }
})
</script>

<template>
  <div class="quick-input-container" :style="{ opacity: (opacity ?? 100) / 100 }">
    <!-- 搜索按钮（初始状态） -->
    <div
      v-if="!isExpanded"
      class="search-button"
      @click.stop="toggleInput"
    >
      <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8"/>
        <path d="M21 21l-4.35-4.35"/>
      </svg>
    </div>

    <!-- 展开的输入框 -->
    <Transition name="expand">
      <div
        v-if="isExpanded"
        class="input-box"
        :style="{
          height: `${Math.max(ballSize, textareaHeight)}px`
        }"
      >
        <div class="input-wrapper">
          <svg class="input-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"/>
            <path d="M21 21l-4.35-4.35"/>
          </svg>
          <textarea
            ref="inputRef"
            v-model="inputText"
            class="chat-input"
            placeholder="AIDI 一下，你就知道~"
            rows="1"
            @keydown="handleKeydown"
            @input="autoResize"
            @compositionstart="imeComposing = true"
            @compositionend="() => setTimeout(() => { imeComposing = false }, 0)"
          />
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.quick-input-container {
  display: flex;
  align-items: center;
  pointer-events: auto;
}

/* 搜索按钮 — 融入胶囊，无独立背景 */
.search-button {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 0;
  cursor: pointer;
  background: transparent;
  flex-shrink: 0;
  transition: transform 150ms ease-out, opacity 150ms ease-out;
  padding: 0;
}

.search-button:hover {
  transform: scale(1.1);
  opacity: 0.85;
}

.search-button:active {
  transform: scale(0.94);
  opacity: 1;
}

.search-icon {
  width: 18px;
  height: 18px;
  color: rgba(255, 255, 255, 0.85);
}

/* 输入框容器 — 融入胶囊，无独立背景 */
.input-box {
  width: 240px;
  border-radius: 0;
  background: transparent;
  display: flex;
  align-items: center;
  padding: 0 12px 0 8px;
}

.input-wrapper {
  display: flex;
  align-items: center;
  width: 100%;
  height: 100%;
  gap: 8px;
}

.input-icon {
  width: 16px;
  height: 16px;
  color: rgba(255, 255, 255, 0.5);
  flex-shrink: 0;
}

.chat-input {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.92);
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
  letter-spacing: 0.01em;
  resize: none;
  line-height: 1.5;
  min-height: 20px;
}

.chat-input::placeholder {
  color: rgba(255, 255, 255, 0.38);
}

/* 展开动画 */
.expand-enter-active {
  animation: expand-in 200ms cubic-bezier(0.16, 1, 0.3, 1);
}

.expand-leave-active {
  animation: expand-in 200ms cubic-bezier(0.16, 1, 0.3, 1) reverse;
}

@keyframes expand-in {
  from {
    opacity: 0;
    width: 0;
    transform: translateX(-8px);
  }
  to {
    opacity: 1;
    width: 240px;
    transform: translateX(0);
  }
}

/* 深色模式 */
.dark .input-box {
  background: rgba(26, 26, 46, 0.9);
  border-color: rgba(255, 255, 255, 0.1);
}

.dark .chat-input {
  color: #E5E7EB;
}

.dark .chat-input::placeholder {
  color: #6B7280;
}

.dark .input-icon {
  color: #6B7280;
}
</style>
