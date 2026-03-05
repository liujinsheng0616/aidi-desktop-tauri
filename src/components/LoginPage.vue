<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { fetchUserIdByCode, fetchTokenByUserId, fetchCurrentUser, setToken, setUser } from '../stores/auth'

type Status = 'qr' | 'processing' | 'success' | 'error'

const status = ref<Status>('qr')
const errorMessage = ref('')
const iframeRef = ref<HTMLIFrameElement | null>(null)

const appId = import.meta.env.VITE_FS_APPID as string
const redirectUriRaw = import.meta.env.VITE_FS_REDIRECT_URI as string
const redirectUri = encodeURIComponent(redirectUriRaw)
// gotoUrl 保留原始值，postMessage 回调时需要拼接 tmp_code 再跳转
const gotoUrl = `https://passport.feishu.cn/suite/passport/oauth/authorize?client_id=${appId}&redirect_uri=${redirectUri}&response_type=code&state=FS`

const qrIframeSrc = `https://passport.feishu.cn/suite/passport/sso/qr?goto=${encodeURIComponent(gotoUrl)}`

// 写日志到桌面文件
async function logDebug(message: string) {
  console.log(message)
  try {
    await invoke('log_debug', { message })
  } catch {
    // 忽略错误
  }
}

let deepLinkUnsubscribe: (() => void) | null = null

async function retryLogin() {
  await logDebug('[Login] 点击重新登录，跳转到 /login.html')
  window.location.href = '/login.html'
}

async function handleCode(code: string) {
  await logDebug(`[Login] handleCode 开始处理, code=${code.substring(0, 15)}...`)
  status.value = 'processing'
  try {
    await logDebug('[Login] 正在调用 fetchUserIdByCode...')
    const userId = await fetchUserIdByCode(code)
    await logDebug(`[Login] fetchUserIdByCode 返回 userId=${userId}`)

    await logDebug('[Login] 正在调用 fetchTokenByUserId...')
    const token = await fetchTokenByUserId(userId)
    await logDebug(`[Login] fetchTokenByUserId 返回 token=${token.substring(0, 10)}...`)

    setToken(token)

    await logDebug('[Login] 正在调用 fetchCurrentUser...')
    const user = await fetchCurrentUser(token)
    await logDebug(`[Login] fetchCurrentUser 返回 user=${JSON.stringify(user)}`)

    setUser(user)
    status.value = 'success'

    await logDebug('[Login] 登录成功，正在保存登录信息到文件...')
    // 调用 Rust 命令保存登录信息到 auth.json（供主窗口读取）
    await invoke('save_login_info', {
      token,
      userId: user.id,
      userName: user.name,
      userJson: JSON.stringify(user)
    })
    await logDebug('[Login] 登录信息已保存到文件')

    await logDebug('[Login] 等待 800ms 后调用 on_login_success')
    await new Promise(r => setTimeout(r, 800))

    // 直接调用 Rust 命令处理登录完成，不依赖 main 窗口的事件监听
    await invoke('on_login_success')
    await logDebug('[Login] on_login_success 调用完成')
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : '登录失败，请重试'
    await logDebug(`[Login] handleCode 错误: ${errMsg}`)
    status.value = 'error'
    errorMessage.value = errMsg
  }
}

// 飞书 sso/qr 页面扫码授权后，通过 postMessage 把 tmp_code（纯字符串）发给父页面
// 父页面需模拟 Feishu QRLogin SDK 行为：拼接 tmp_code 跳转到 OAuth 授权端点
// 授权端点再重定向回 redirect_uri?code=xxx，onMounted 读取 code 完成登录
async function onMessage(e: MessageEvent) {
  await logDebug(`[Login] onMessage 触发, origin=${e.origin}, dataType=${typeof e.data}`)

  const feishuOrigins = ['feishu.cn', 'larksuite.com', 'larkoffice.com']
  if (!feishuOrigins.some(o => e.origin.endsWith(o))) {
    await logDebug(`[Login] onMessage 忽略非飞书来源: ${e.origin}`)
    return
  }
  if (typeof e.data !== 'string' || !e.data) {
    await logDebug(`[Login] onMessage 忽略非字符串数据`)
    return
  }

  await logDebug(`[Login] 收到飞书 postMessage, data=${e.data.substring(0, 50)}...`)

  // e.data 就是 tmp_code，跳转让 Feishu 换取真正的 code
  const redirectUrl = `${gotoUrl}&tmp_code=${encodeURIComponent(e.data)}`
  await logDebug(`[Login] 准备跳转到飞书授权页面: ${redirectUrl}`)

  try {
    window.location.href = redirectUrl
    await logDebug(`[Login] 跳转命令已执行`)
  } catch (err) {
    await logDebug(`[Login] 跳转失败: ${err}`)
    status.value = 'error'
    errorMessage.value = `跳转失败: ${err instanceof Error ? err.message : String(err)}`
  }
}

// 兜底：iframe 同源跳转时（redirect_uri 落在 localhost）直接从 iframe URL 提取 code
async function onIframeLoad() {
  await logDebug(`[Login] onIframeLoad 触发`)
  try {
    const search = iframeRef.value?.contentWindow?.location.search ?? ''
    await logDebug(`[Login] iframe search: ${search}`)
    const code = new URLSearchParams(search).get('code')
    if (code) {
      await logDebug(`[Login] 从 iframe 提取到 code: ${code.substring(0, 10)}...`)
      await handleCode(code)
    }
  } catch (e) {
    // 跨域帧抛出 SecurityError，忽略
    await logDebug(`[Login] onIframeLoad 跨域错误 (预期行为)`)
  }
}

// 处理 deep link 回调（生产环境）
// 飞书重定向到 aidi://auth?code=xxx
async function handleDeepLink(urls: string[]) {
  await logDebug(`[Login] handleDeepLink 收到 URLs: ${JSON.stringify(urls)}`)
  for (const url of urls) {
    try {
      const urlObj = new URL(url)
      const code = urlObj.searchParams.get('code')
      if (code) {
        await logDebug(`[Login] DeepLink 提取到 code: ${code.substring(0, 10)}...`)
        await handleCode(code)
        return
      }
    } catch (e) {
      await logDebug(`[Login] DeepLink URL 解析失败: ${url}, error=${e}`)
    }
  }
}

onMounted(async () => {
  // 全局错误捕获
  const handleError = async (event: ErrorEvent) => {
    await logDebug(`[Login] 全局错误: ${event.message}, filename=${event.filename}, lineno=${event.lineno}`)
  }
  const handleRejection = async (event: PromiseRejectionEvent) => {
    await logDebug(`[Login] Promise 拒绝: ${event.reason}`)
  }
  window.addEventListener('error', handleError)
  window.addEventListener('unhandledrejection', handleRejection)

  // 输出调试日志到桌面文件
  await logDebug('========== LoginPage onMounted ==========')
  await logDebug('[Login] 环境变量:')
  await logDebug(`  VITE_FS_APPID: ${appId}`)
  await logDebug(`  VITE_FS_REDIRECT_URI: ${redirectUriRaw}`)
  await logDebug(`  gotoUrl: ${gotoUrl}`)
  await logDebug(`  qrIframeSrc: ${qrIframeSrc}`)
  await logDebug(`  当前 URL: ${window.location.href}`)
  await logDebug(`  当前 origin: ${window.location.origin}`)
  await logDebug(`  当前 pathname: ${window.location.pathname}`)
  await logDebug(`  当前 search: ${window.location.search}`)

  // 开发环境：OAuth 回调页面以 ?code=xxx 重新加载
  const code = new URLSearchParams(window.location.search).get('code')
  if (code) {
    await logDebug(`[Login] 检测到 URL 中的 code 参数: ${code.substring(0, 15)}...`)
    await handleCode(code)
    return
  }

  await logDebug(`[Login] 未检测到 code 参数，进入扫码模式...`)

  // 注册 deep link 监听器（前端插件方式）
  try {
    deepLinkUnsubscribe = await onOpenUrl(handleDeepLink)
    await logDebug('[Login] DeepLink 插件监听器已注册')
  } catch (e) {
    await logDebug(`[Login] DeepLink 插件注册失败: ${e}`)
  }

  // 监听 Rust 端转发的 deep link 事件（备用方式）
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen<string[]>('deep-link-received', async (event) => {
      await logDebug(`[Login] 收到 Rust 转发的 deep-link-received 事件: ${JSON.stringify(event.payload)}`)
      await handleDeepLink(event.payload)
    })
    await logDebug('[Login] Rust deep link 事件监听器已注册')
    // 保存取消监听函数
    const originalUnsubscribe = deepLinkUnsubscribe
    deepLinkUnsubscribe = async () => {
      if (originalUnsubscribe) originalUnsubscribe()
      unlisten()
    }
  } catch (e) {
    await logDebug(`[Login] Rust deep link 事件监听失败: ${e}`)
  }

  // 监听 iframe postMessage
  window.addEventListener('message', onMessage)
  await logDebug('[Login] postMessage 监听器已注册')
})

onUnmounted(() => {
  window.removeEventListener('message', onMessage)
  if (deepLinkUnsubscribe) {
    deepLinkUnsubscribe()
  }
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
