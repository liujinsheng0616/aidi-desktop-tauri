<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { feishuLogin, convertFeishuUser, setUser, type FeishuUserInfo } from '../stores/auth'

type Status = 'qr' | 'processing' | 'success' | 'error'

const status = ref<Status>('qr')
const errorMessage = ref('')
const iframeRef = ref<HTMLIFrameElement | null>(null)

const appId = import.meta.env.VITE_FS_APPID as string
const redirectUriRaw = import.meta.env.VITE_FS_REDIRECT_URI as string
const redirectUri = encodeURIComponent(redirectUriRaw)
const gotoUrl = `https://passport.feishu.cn/suite/passport/oauth/authorize?client_id=${appId}&redirect_uri=${redirectUri}&response_type=code&state=FS`
const qrIframeSrc = `https://passport.feishu.cn/suite/passport/sso/qr?goto=${encodeURIComponent(gotoUrl)}`


let deepLinkUnsubscribe: (() => void) | null = null

function retryLogin() {
  window.location.href = '/login.html'
}

async function log(msg: string) {
  try { await invoke('log_debug', { message: msg }) } catch {}
}

async function handleCode(code: string) {
  await log(`[Login] handleCode start, code=${code.substring(0, 15)}`)
  status.value = 'processing'
  try {
    // 新流程：直接通过 Rust 调用飞书 API 获取用户信息
    await log('[Login] feishuLogin (Rust direct API)...')
    const feishuUser: FeishuUserInfo = await feishuLogin(code)
    await log(`[Login] feishuUser=${JSON.stringify(feishuUser).substring(0, 100)}`)

    // 转换为内部用户格式
    const user = convertFeishuUser(feishuUser)
    await log(`[Login] converted user=${JSON.stringify(user).substring(0, 100)}`)

    setUser(user)
    status.value = 'success'

    // 保存登录信息到本地文件
    try {
      await invoke('save_login_info', {
        token: '',  // 新流程不再使用 token
        userId: user.id,
        userName: user.name,
        userJson: JSON.stringify(user)
      })
      await log('[Login] save_login_info ok')
    } catch (e) {
      await log(`[Login] save_login_info failed: ${e}`)
    }

    // 通过 URL 导航触发 Rust on_navigation 兜底（兼容 Windows/WebView2）
    const encodedUser = encodeURIComponent(JSON.stringify(user))
    const triggerUrl = `${window.location.origin}/aidi-login-success#invoke=login-success&user=${encodedUser}`
    await log(`[Login] navigating: ${triggerUrl.substring(0, 80)}`)
    window.location.href = triggerUrl

    // macOS 正常走 invoke 兜底（幂等安全）
    await new Promise(r => setTimeout(r, 800))
    try {
      await invoke('on_login_success')
      await log('[Login] on_login_success ok')
    } catch (e) {
      await log(`[Login] on_login_success failed: ${e}`)
    }
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err)
    await log(`[Login] handleCode ERROR: ${errMsg}`)
    // 通过 URL 导航报告错误（兜底，不依赖 IPC）
    const errorUrl = `${window.location.origin}/aidi-login-error?msg=${encodeURIComponent(errMsg.substring(0, 300))}`
    window.location.href = errorUrl
    status.value = 'error'
    errorMessage.value = errMsg
  }
}

async function onMessage(e: MessageEvent) {
  const feishuOrigins = ['feishu.cn', 'larksuite.com', 'larkoffice.com']
  if (!feishuOrigins.some(o => e.origin.endsWith(o))) return
  if (typeof e.data !== 'string' || !e.data) return

  const redirectUrl = `${gotoUrl}&tmp_code=${encodeURIComponent(e.data)}`
  try {
    window.location.href = redirectUrl
  } catch (err) {
    status.value = 'error'
    errorMessage.value = `跳转失败: ${err instanceof Error ? err.message : String(err)}`
  }
}

async function onIframeLoad() {
  try {
    const search = iframeRef.value?.contentWindow?.location.search ?? ''
    const code = new URLSearchParams(search).get('code')
    if (code) await handleCode(code)
  } catch {
    // 跨域帧 SecurityError，忽略
  }
}

async function handleDeepLink(urls: string[]) {
  for (const url of urls) {
    try {
      const code = new URL(url).searchParams.get('code')
      if (code) { await handleCode(code); return }
    } catch {}
  }
}

onMounted(async () => {
  await log(`[Login] onMounted, url=${window.location.href.substring(0, 120)}`)

  // 检测 OAuth 回调 code
  const code = new URLSearchParams(window.location.search).get('code')
  await log(`[Login] search=${window.location.search}, code=${code ? code.substring(0, 15) : 'null'}`)
  if (code) {
    await handleCode(code)
    return
  }

  // 注册 deep link 监听
  try {
    deepLinkUnsubscribe = await onOpenUrl(handleDeepLink)
  } catch {}

  // 监听 Rust 转发的 deep link 事件（备用）
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen<string[]>('deep-link-received', (event) => {
      handleDeepLink(event.payload)
    })
    const prev = deepLinkUnsubscribe
    deepLinkUnsubscribe = async () => { if (prev) prev(); unlisten() }
  } catch {}

  window.addEventListener('message', onMessage)
})

onUnmounted(() => {
  window.removeEventListener('message', onMessage)
  if (deepLinkUnsubscribe) deepLinkUnsubscribe()
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
