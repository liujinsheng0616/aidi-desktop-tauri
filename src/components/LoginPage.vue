<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { emitTo } from '@tauri-apps/api/event'
import { fetchUserIdByCode, fetchTokenByUserId, fetchCurrentUser, setToken, setUser } from '../stores/auth'

type Status = 'qr' | 'processing' | 'success' | 'error'

const status = ref<Status>('qr')
const errorMessage = ref('')
const iframeRef = ref<HTMLIFrameElement | null>(null)

const appId = import.meta.env.VITE_FS_APPID as string
const redirectUri = encodeURIComponent(import.meta.env.VITE_FS_REDIRECT_URI as string)
// gotoUrl 保留原始值，postMessage 回调时需要拼接 tmp_code 再跳转
const gotoUrl = `https://passport.feishu.cn/suite/passport/oauth/authorize?client_id=${appId}&redirect_uri=${redirectUri}&response_type=code&state=FS`

const qrIframeSrc = `https://passport.feishu.cn/suite/passport/sso/qr?goto=${encodeURIComponent(gotoUrl)}`

function retryLogin() {
  window.location.href = '/login.html'
}

async function handleCode(code: string) {
  status.value = 'processing'
  try {
    const userId = await fetchUserIdByCode(code)
    const token = await fetchTokenByUserId(userId)
    setToken(token)
    const user = await fetchCurrentUser(token)
    setUser(user)
    status.value = 'success'
    await new Promise(r => setTimeout(r, 800))
    await emitTo('main', 'login-complete', {})
  } catch (err) {
    status.value = 'error'
    errorMessage.value = err instanceof Error ? err.message : '登录失败，请重试'
  }
}

// 飞书 sso/qr 页面扫码授权后，通过 postMessage 把 tmp_code（纯字符串）发给父页面
// 父页面需模拟 Feishu QRLogin SDK 行为：拼接 tmp_code 跳转到 OAuth 授权端点
// 授权端点再重定向回 redirect_uri?code=xxx，onMounted 读取 code 完成登录
function onMessage(e: MessageEvent) {
  const feishuOrigins = ['feishu.cn', 'larksuite.com', 'larkoffice.com']
  if (!feishuOrigins.some(o => e.origin.endsWith(o))) return
  if (typeof e.data !== 'string' || !e.data) return
  // e.data 就是 tmp_code，跳转让 Feishu 换取真正的 code
  window.location.href = `${gotoUrl}&tmp_code=${encodeURIComponent(e.data)}`
}

// 兜底：iframe 同源跳转时（redirect_uri 落在 localhost）直接从 iframe URL 提取 code
function onIframeLoad() {
  try {
    const search = iframeRef.value?.contentWindow?.location.search ?? ''
    const code = new URLSearchParams(search).get('code')
    if (code) handleCode(code)
  } catch {
    // 跨域帧抛出 SecurityError，忽略
  }
}

onMounted(async () => {
  // OAuth 回调：页面以 ?code=xxx 重新加载
  const code = new URLSearchParams(window.location.search).get('code')
  if (code) {
    await handleCode(code)
    return
  }
  window.addEventListener('message', onMessage)
})

onUnmounted(() => {
  window.removeEventListener('message', onMessage)
})
</script>

<template>
  <div class="login-page" data-tauri-drag-region>
    <!-- 二维码模式 -->
    <div v-if="status === 'qr'" class="body" data-tauri-drag-region>
      <h1 class="title" data-tauri-drag-region>扫码登录</h1>
      <p class="subtitle" data-tauri-drag-region>请使用飞书移动端扫描二维码</p>

      <div class="qr-wrap">
        <iframe
          ref="iframeRef"
          :src="qrIframeSrc"
          width="300"
          height="300"
          frameborder="0"
          scrolling="no"
          class="qr-iframe"
          @load="onIframeLoad"
        ></iframe>
      </div>
    </div>

    <!-- 回调处理模式（processing / success / error） -->
    <div v-else class="body">
      <!-- 处理中 -->
      <template v-if="status === 'processing'">
        <div class="spinner"></div>
        <p class="status-text">正在验证登录信息...</p>
      </template>

      <!-- 成功 -->
      <template v-else-if="status === 'success'">
        <div class="status-icon success-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </div>
        <p class="status-text success">登录成功</p>
      </template>

      <!-- 失败 -->
      <template v-else>
        <div class="status-icon error-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </div>
        <p class="status-text error">{{ errorMessage }}</p>
        <button class="retry-btn" @click="retryLogin">重新登录</button>
      </template>
    </div>
  </div>
</template>

<style scoped>
* { box-sizing: border-box; }

.login-page {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #fff;
  overflow: hidden;
  user-select: none;
  -webkit-user-select: none;
}

/* ── 主体 ── */
.body {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 24px;
}

.title {
  color: #1a1a1a;
  font-size: 22px;
  font-weight: 700;
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Segoe UI', sans-serif;
  margin: 0 0 2px;
  pointer-events: none;
}

.subtitle {
  color: #888;
  font-size: 13px;
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Segoe UI', sans-serif;
  margin: 0 0 12px;
  pointer-events: none;
}

/* 二维码 */
.qr-wrap {
  width: 220px;
  height: 220px;
  border-radius: 8px;
  background: #fff;
  border: 1px solid #eee;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  overflow: hidden;
}

.qr-iframe {
  display: block;
  border: none;
  transform: scale(0.9);
  transform-origin: center center;
}

/* ── 回调状态 ── */
.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid #f0f0f0;
  border-top-color: #1677ff;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }

.status-icon {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.success-icon {
  background: rgba(34, 197, 94, 0.12);
  color: #22c55e;
}

.error-icon {
  background: rgba(239, 68, 68, 0.12);
  color: #ef4444;
}

.status-icon svg {
  width: 26px;
  height: 26px;
}

.status-text {
  color: #555;
  font-size: 14px;
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Segoe UI', sans-serif;
  margin: 0;
}

.status-text.success {
  color: #22c55e;
}

.status-text.error {
  color: #ef4444;
  text-align: center;
  max-width: 280px;
}

.retry-btn {
  padding: 9px 22px;
  background: #1677ff;
  color: #fff;
  border: none;
  border-radius: 7px;
  font-size: 13px;
  cursor: pointer;
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Segoe UI', sans-serif;
  transition: background 0.15s;
}

.retry-btn:hover {
  background: #0e5fd8;
}
</style>
