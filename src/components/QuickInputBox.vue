<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue'
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
const isSending = ref(false) // 发送中状态

// 事件监听器清理函数
let unlistenCollapse: UnlistenFn | null = null
let unlistenStreamEnd: UnlistenFn | null = null
let unlistenStreamStart: UnlistenFn | null = null

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
  if (!text || isSending.value) return

  isSending.value = true
  try {
    // 调用 Rust 后端发送消息并显示聊天窗口
    await invoke('send_chat_message', { message: text })
    inputText.value = ''
    // 发送后重置高度和 textarea DOM 样式，光标回到初始位置
    textareaHeight.value = ballSize.value
    savedHeight.value = 0
    if (inputRef.value) {
      inputRef.value.style.height = 'auto'
      inputRef.value.style.overflowY = 'hidden'
      inputRef.value.setSelectionRange(0, 0)
    }
    emit('heightChange', textareaHeight.value)
    inputRef.value?.focus()
  } catch (error) {
    isSending.value = false
  }
}

// 停止发送
function stopMessage() {
  isSending.value = false
  // 向聊天窗口发送停止事件
  emitTauriEvent('stop-chat-stream').catch(() => {})
}

// 按键处理
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    // keyCode === 229 表示 IME 正在组合输入，此时不处理
    if (e.keyCode === 229) {
      return
    }
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
  // 监听聊天流开始事件（重试时由 ChatView 触发）
  unlistenStreamStart = await listen('chat-stream-start', () => {
    isSending.value = true
  })
  // 监听聊天流结束事件，恢复发送按钮并重新聚焦输入框
  unlistenStreamEnd = await listen('chat-stream-end', () => {
    isSending.value = false
    nextTick(() => inputRef.value?.focus())
  })
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
  if (unlistenCollapse) unlistenCollapse()
  if (unlistenStreamStart) unlistenStreamStart()
  if (unlistenStreamEnd) unlistenStreamEnd()
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
            :disabled="isSending"
            @keydown="handleKeydown"
            @input="autoResize"
          />
          <!-- 停止按钮（发送中） -->
          <button v-if="isSending" class="action-btn stop-btn" @click.stop="stopMessage" title="停止">
            <span class="stop-icon" />
          </button>
          <!-- 发送按钮（有内容时显示） -->
          <button v-else-if="inputText.trim()" class="action-btn send-btn" @click.stop="sendMessage" title="发送">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <path d="M5 12h14M13 6l6 6-6 6"/>
            </svg>
          </button>
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

.chat-input:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* 发送 / 停止按钮公共样式 */
.action-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 50%;
  cursor: pointer;
  padding: 0;
  transition: transform 120ms ease, opacity 120ms ease;
}

.action-btn:active {
  transform: scale(0.88);
}

/* 发送按钮 */
.send-btn {
  background: rgba(255, 255, 255, 0.18);
}

.send-btn:hover {
  background: rgba(255, 255, 255, 0.28);
}

.send-btn svg {
  width: 13px;
  height: 13px;
  color: rgba(255, 255, 255, 0.9);
}

/* 停止按钮 */
.stop-btn {
  background: rgba(255, 80, 80, 0.25);
  animation: stop-pulse 1.2s ease-in-out infinite;
}

.stop-btn:hover {
  background: rgba(255, 80, 80, 0.45);
}

.stop-icon {
  display: block;
  width: 8px;
  height: 8px;
  border-radius: 1.5px;
  background: rgba(255, 120, 120, 0.95);
}

@keyframes stop-pulse {
  0%, 100% { box-shadow: 0 0 0 0 rgba(255, 80, 80, 0.4); }
  50%       { box-shadow: 0 0 0 4px rgba(255, 80, 80, 0); }
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
