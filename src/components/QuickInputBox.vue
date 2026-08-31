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
const pendingScreenshots = ref<string[]>([]) // 截图 base64 data URL 列表（支持多张）
const isWikiActive = ref(false) // IT 数据库检索开关
const screenshotLimitNotice = ref<string | null>(null) // 截图超限提示（独立于权限提示）
// 截图权限缺失提示：借 placeholder 显示，避免在小窗口里做浮层被窗口边界裁掉
const permissionNotice = ref<string | null>(null)
let noticeTimer: ReturnType<typeof setTimeout> | null = null

// 事件监听器清理函数
let unlistenCollapse: UnlistenFn | null = null
let unlistenStreamEnd: UnlistenFn | null = null
let unlistenStreamStart: UnlistenFn | null = null
let unlistenQuickScreenshot: UnlistenFn | null = null
let unlistenScreenshotPermission: UnlistenFn | null = null

const ballSize = computed(() => props.size || 60)

/**
 * 显示截图权限提示。
 * 用独立提示行替换输入框（而非 placeholder）：长按空格会把空格打进输入框，
 * inputText 非空时 placeholder 不显示，正好是最需要提示的场景。
 * inputText 是 ref，textarea 短暂卸载不会丢已输入内容。
 */
function showPermissionNotice(message: string) {
  permissionNotice.value = message
  if (noticeTimer) clearTimeout(noticeTimer)
  // 8s 而非 5s：提示行是可点击的授权入口，得留出看见 + 点击的时间；
  // 但它占着输入框位置，不能常驻，所以仍然自动恢复。
  noticeTimer = setTimeout(() => {
    permissionNotice.value = null
    noticeTimer = null
    nextTick(() => inputRef.value?.focus())
  }, 8000)
}

/** 点击提示行 → 打开系统「屏幕录制」设置面板 */
function openPermissionSettings() {
  invoke('open_screen_recording_settings').catch(() => {})
  // 立刻收起提示，把输入框还给用户（授权在系统设置里进行，与此处无关）
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = null
  permissionNotice.value = null
  nextTick(() => inputRef.value?.focus())
}

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
          inputRef.value.style.overflowY = textareaActualHeight >= 140 ? 'auto' : 'hidden'
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
  // 收起时清除待发截图（避免后端 static 残留，被下次纯文本消息误带）
  if (pendingScreenshots.value.length > 0) {
    pendingScreenshots.value = []
    invoke('clear_pending_screenshot').catch(() => {})
  }
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
  const maxHeight = 140 // 约 7 行

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

  // 计算截图行高度（如果有截图，需要额外空间）
  const screenshotRow = textarea.closest('.input-box')?.querySelector('.screenshot-row') as HTMLElement | null
  let screenshotHeight = 0
  if (screenshotRow) {
    screenshotHeight = screenshotRow.offsetHeight + 4 // 4px gap
  }

  // 计算容器高度（textarea 高度 + 截图行高度）
  textareaHeight.value = Math.max(ballSize.value, textareaActualHeight + screenshotHeight)

  // 通知父组件高度变化
  emit('heightChange', textareaHeight.value)
}

// 移除截图预览（按索引删除单张）
function removeScreenshot(index: number) {
  pendingScreenshots.value.splice(index, 1)
  if (pendingScreenshots.value.length === 0) {
    invoke('clear_pending_screenshot').catch(() => {})
  }
  // 截图行变化后重算容器高度
  nextTick(() => autoResize())
}

// 点击缩略图查看大图（交系统看图工具打开，可缩放）
function openPreview(index: number) {
  const dataUrl = pendingScreenshots.value[index]
  if (!dataUrl) return
  invoke('open_screenshot_preview', { dataUrl }).catch(() => {})
}

// 发送消息
async function sendMessage() {
  const text = inputText.value.trim()
  const hasImg = pendingScreenshots.value.length > 0
  if ((!text && !hasImg) || isSending.value) return

  isSending.value = true
  // 先清本地预览、再 await：collapseInput() 在有截图时会调用
  // clear_pending_screenshot 清空后端缓存。send_chat_message 期间可能要新建聊天窗口
  // 并加载远程页面，耗时较长，此间若用户点击别处触发收起，
  // 就会把 ChatView 尚未取走的截图清掉，导致只发出文字、图丢失。
  const imgSnapshot = [...pendingScreenshots.value]
  pendingScreenshots.value = []
  try {
    // 调用 Rust 后端发送消息并显示聊天窗口
    // 注：截图走后端 static 缓存，ChatView 发送时主动拉取（多模态）
    await invoke('send_chat_message', { message: text, enableWikiSearch: isWikiActive.value })
    inputText.value = ''
    // 发送成功后重置 IT 数据库按钮状态
    isWikiActive.value = false
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
    // 发送失败：恢复截图预览，让用户能直接重试（后端缓存仍在，未被取走）
    if (imgSnapshot.length > 0) pendingScreenshots.value = imgSnapshot
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
  // 监听截图预览（长按空格 / Cmd+E，自动展开输入框并挂上缩略图）
  unlistenQuickScreenshot = await listen<{ imageBase64: string }>('quick-screenshot-preview', (event) => {
    if (event.payload?.imageBase64) {
      // 截图成功，清掉可能还在显示的权限提示
      if (noticeTimer) clearTimeout(noticeTimer)
      noticeTimer = null
      permissionNotice.value = null
      screenshotLimitNotice.value = null
      // 最多允许 8 张截图，超出则提示
      if (pendingScreenshots.value.length >= 8) {
        screenshotLimitNotice.value = '最多只能添加 8 张截图'
        if (noticeTimer) clearTimeout(noticeTimer)
        noticeTimer = setTimeout(() => {
          screenshotLimitNotice.value = null
          noticeTimer = null
        }, 3000)
        return
      }
      pendingScreenshots.value.push(event.payload.imageBase64)
      if (!isExpanded.value) {
        toggleInput()
      } else {
        nextTick(() => {
          inputRef.value?.focus()
          autoResize() // 截图行变化后重算容器高度
        })
      }
    }
  })
  // 监听截图权限缺失（此前后端已发此事件但前端无人接收，表现为按了没反应）
  unlistenScreenshotPermission = await listen<{ message: string }>('screenshot-permission-needed', (event) => {
    const message = event.payload?.message || '需要「屏幕录制」权限才能截图'
    if (!isExpanded.value) {
      toggleInput()
    }
    showPermissionNotice(message)
  })
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
  if (unlistenCollapse) unlistenCollapse()
  if (unlistenStreamStart) unlistenStreamStart()
  if (unlistenStreamEnd) unlistenStreamEnd()
  if (unlistenQuickScreenshot) unlistenQuickScreenshot()
  if (unlistenScreenshotPermission) unlistenScreenshotPermission()
  if (noticeTimer) clearTimeout(noticeTimer)
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
          height: `${textareaHeight}px`
        }"
      >
        <!-- 截图缩略图预览区（可换行，不挤压输入区） -->
        <div v-if="pendingScreenshots.length > 0" class="screenshot-row">
          <div v-for="(src, i) in pendingScreenshots" :key="i" class="screenshot-thumb">
            <img :src="src" :alt="`截图预览 ${i + 1}`" :title="`第 ${i + 1} 张截图 — 点击查看大图`" @click.stop="openPreview(i)" />
            <span class="thumb-index">{{ i + 1 }}</span>
            <button class="thumb-remove" @click.stop="removeScreenshot(i)" title="移除截图">×</button>
          </div>
        </div>
        <div class="input-wrapper">
          <svg class="input-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"/>
            <path d="M21 21l-4.35-4.35"/>
          </svg>
          <!-- 截图超限提示（短暂显示，3s 后自动消失） -->
          <Transition name="fade">
            <div v-if="screenshotLimitNotice" class="screenshot-limit-notice">
              {{ screenshotLimitNotice }}
            </div>
          </Transition>
          <!-- 截图权限缺失提示（替换输入框，5s 后自动恢复） -->
          <!-- 点击打开系统设置面板；title 承载完整说明（提示行宽度有限，只放短文案） -->
          <div
            v-if="permissionNotice"
            class="permission-notice"
            title="需要「屏幕录制」权限才能截图。点击打开「系统设置 → 隐私与安全性 → 屏幕录制」，勾选 AIDI 后重启应用。"
            @click.stop="openPermissionSettings"
          >
            <svg class="notice-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/>
              <path d="M12 8v4M12 16h.01"/>
            </svg>
            <span class="notice-text">{{ permissionNotice }}</span>
            <span class="notice-action">点击授权</span>
          </div>
          <textarea
            v-else
            ref="inputRef"
            v-model="inputText"
            class="chat-input"
            placeholder="AIDI 一下，你就知道~"
            rows="1"
            :disabled="isSending"
            @keydown="handleKeydown"
            @input="autoResize"
          />
          <!-- IT 数据库检索切换按钮 -->
          <button
            class="wiki-toggle-btn"
            :class="{ 'wiki-toggle-btn--active': isWikiActive }"
            :title="isWikiActive ? 'IT数据库已开启（点击关闭）' : '开启IT数据库检索'"
            @click.stop="isWikiActive = !isWikiActive"
          >
            <span class="wiki-toggle-label">IT数据库</span>
          </button>
          <!-- 停止按钮（发送中） -->
          <button v-if="isSending" class="action-btn stop-btn" @click.stop="stopMessage" title="停止">
            <span class="stop-icon" />
          </button>
          <!-- 发送按钮（有内容时显示） -->
          <button v-else-if="inputText.trim() || pendingScreenshots.length > 0" class="action-btn send-btn" @click.stop="sendMessage" title="发送">
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
  width: 400px;
  border-radius: 0;
  background: transparent;
  display: flex;
  flex-direction: column;
  padding: 0 12px 0 8px;
}

.input-wrapper {
  display: flex;
  align-items: center;
  width: 100%;
  flex: 1;
  gap: 8px;
}

/* 截图缩略图行 — 可换行，不挤占输入区 */
.screenshot-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 4px 0;
  width: 100%;
}

.input-icon {
  width: 16px;
  height: 16px;
  color: rgba(255, 255, 255, 0.5);
  flex-shrink: 0;
}

/* 截图缩略图（Cmd+E 预览） */
.screenshot-thumb {
  position: relative;
  flex-shrink: 0;
  width: 34px;
  height: 34px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.25);
}
.screenshot-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 6px;
  display: block;
  cursor: zoom-in;
  transition: transform 120ms ease;
}
.screenshot-thumb img:hover {
  transform: scale(1.08);
}
.thumb-remove {
  position: absolute;
  top: -5px;
  right: -5px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.7);
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.3);
  font-size: 11px;
  line-height: 1;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}
.thumb-remove:hover {
  background: rgba(220, 38, 38, 0.9);
}
/* 缩略图序号角标（左下角，显示第几张） */
.thumb-index {
  position: absolute;
  bottom: -2px;
  left: -2px;
  min-width: 14px;
  height: 14px;
  border-radius: 7px;
  background: rgba(37, 99, 235, 0.9);
  color: #fff;
  font-size: 9px;
  font-weight: 700;
  line-height: 14px;
  text-align: center;
  padding: 0 3px;
  pointer-events: none;
}

/* 截图权限缺失提示：占据输入框位置，5s 后自动恢复成输入框 */
.permission-notice {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  min-height: 20px;
  font-size: 12px;
  line-height: 1.5;
  color: #FCA5A5;
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
  letter-spacing: 0.01em;
  cursor: pointer;
}

.permission-notice:hover .notice-action {
  background: rgba(252, 165, 165, 0.28);
}

/* 「点击授权」标签：不参与压缩，保证在窄窗口里始终可见可点 */
.notice-action {
  flex-shrink: 0;
  margin-left: auto;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(252, 165, 165, 0.16);
  border: 1px solid rgba(252, 165, 165, 0.35);
  font-size: 11px;
  white-space: nowrap;
}

.notice-icon {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

/* 窗口宽度固定（展开态 303px），文案超长时省略而不是撑破布局 */
.notice-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chat-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.92);
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
  letter-spacing: 0.01em;
  resize: none;
  line-height: 1.5;
  min-height: 28px;
}

.chat-input::placeholder {
  color: rgba(255, 255, 255, 0.38);
  text-align: center;
  font-size: 14px;
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

/* IT 数据库切换按钮 */
.wiki-toggle-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 3px;
  height: 24px;
  padding: 0 6px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 6px;
  background: transparent;
  color: rgba(255, 255, 255, 0.5);
  cursor: pointer;
  font-size: 10px;
  font-weight: 500;
  transition: all 150ms ease;
}

.wiki-toggle-btn svg {
  width: 12px;
  height: 12px;
}

.wiki-toggle-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.8);
}

.wiki-toggle-btn--active {
  background: rgba(37, 99, 235, 0.25);
  border-color: rgba(37, 99, 235, 0.6);
  color: rgba(147, 197, 253, 1);
}

.wiki-toggle-btn--active:hover {
  background: rgba(37, 99, 235, 0.35);
}

/* 截图超限提示 */
.screenshot-limit-notice {
  position: absolute;
  bottom: calc(100% + 4px);
  left: 50%;
  transform: translateX(-50%);
  background: rgba(0, 0, 0, 0.85);
  color: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 6px;
  white-space: nowrap;
  pointer-events: none;
  z-index: 10;
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 200ms ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
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
    width: 400px;
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