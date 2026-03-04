const TOKEN_KEY = 'aidi-token'
const USER_KEY = 'aidi-user'

export interface UserInfo {
  id: string
  name: string
  nickName: string
  headImgUrl: string
  mobile: string
  username: string
  type: string
  fsUserId: string | null
}

// 简单的日志函数
function logAuth(message: string) {
  console.log(`[Auth] ${message}`)
}

// 接口1：code → userId
export async function fetchUserIdByCode(code: string): Promise<string> {
  const appId = import.meta.env.VITE_FS_APPID
  const baseUrl = import.meta.env.VITE_API_BASE_URL
  const params = new URLSearchParams({ code, appId })
  const url = `${baseUrl}/api-uaa/oauth/feishu/employee/authorize?${params}`

  logAuth(`fetchUserIdByCode 请求: ${url}`)

  const res = await fetch(url)
  logAuth(`fetchUserIdByCode 响应状态: ${res.status}`)

  if (!res.ok) {
    const text = await res.text()
    logAuth(`fetchUserIdByCode 错误响应: ${text}`)
    throw new Error('获取授权信息失败，请联系管理员')
  }

  const data = await res.json()
  logAuth(`fetchUserIdByCode 响应数据: ${JSON.stringify(data)}`)

  const userId = data.userId ?? data.data?.userId
  if (!userId) throw new Error('获取授权信息失败，请联系管理员')
  return userId
}

// 接口2：userId → access_token（Basic Auth）
export async function fetchTokenByUserId(userId: string): Promise<string> {
  const baseUrl = import.meta.env.VITE_API_BASE_URL
  const credentials = btoa(`${import.meta.env.VITE_BASIC_USERNAME}:${import.meta.env.VITE_BASIC_PASSWORD}`)
  const body = new URLSearchParams({ openId: userId })
  const url = `${baseUrl}/api-uaa/oauth/openId/token`

  logAuth(`fetchTokenByUserId 请求: ${url}, body: ${body.toString()}`)

  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Authorization': `Basic ${credentials}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: body.toString(),
  })
  logAuth(`fetchTokenByUserId 响应状态: ${res.status}`)

  if (!res.ok) {
    const text = await res.text()
    logAuth(`fetchTokenByUserId 错误响应: ${text}`)
    throw new Error(`获取 Token 失败: ${res.status}`)
  }

  const data = await res.json()
  logAuth(`fetchTokenByUserId 响应数据: ${JSON.stringify(data).substring(0, 200)}...`)

  const token = data.access_token
  if (!token) throw new Error('接口未返回有效 access_token')
  return token
}

// 静默刷新 token：使用缓存用户的 fsUserId 重新获取
async function refreshToken(): Promise<string> {
  const user = getUser()
  if (!user?.fsUserId) {
    const { invoke } = await import('@tauri-apps/api/core')
    const confirmed = await window.confirm('登录已过期，请重新登录')
    if (confirmed) {
      clearAuth()
      await invoke('show_login_window')
    }
    throw new Error('登录已过期，请重新登录')
  }
  const newToken = await fetchTokenByUserId(user.fsUserId)
  setToken(newToken)
  return newToken
}

// 带 Bearer token 的 fetch，401 时自动静默刷新后重试一次
export async function fetchWithAuth(path: string, options: RequestInit = {}): Promise<Response> {
  const baseUrl = import.meta.env.VITE_API_BASE_URL
  const makeHeaders = (token: string | null) => ({
    ...options.headers,
    'Authorization': `Bearer ${token}`,
  })

  let res = await fetch(`${baseUrl}${path}`, { ...options, headers: makeHeaders(getToken()) })

  if (res.status === 401) {
    const newToken = await refreshToken()
    res = await fetch(`${baseUrl}${path}`, { ...options, headers: makeHeaders(newToken) })
  }

  return res
}

// 接口3：获取当前登录人信息
export async function fetchCurrentUser(token: string): Promise<UserInfo> {
  const baseUrl = import.meta.env.VITE_API_BASE_URL
  const url = `${baseUrl}/api-user/users/current`

  logAuth(`fetchCurrentUser 请求: ${url}`)

  const res = await fetch(url, {
    headers: { 'Authorization': `Bearer ${token}` },
  })
  logAuth(`fetchCurrentUser 响应状态: ${res.status}`)

  if (!res.ok) {
    const text = await res.text()
    logAuth(`fetchCurrentUser 错误响应: ${text}`)
    throw new Error(`获取用户信息失败: ${res.status}`)
  }

  const data = await res.json()
  logAuth(`fetchCurrentUser 响应数据: ${JSON.stringify(data)}`)

  if (data.resp_code !== 0) throw new Error(data.resp_msg || '获取用户信息失败')
  const user = data.data
  if (!user) throw new Error('接口未返回有效用户信息')
  return user as UserInfo
}

export const getToken = () => localStorage.getItem(TOKEN_KEY)
export const setToken = (token: string) => localStorage.setItem(TOKEN_KEY, token)
export const getUser = (): UserInfo | null => {
  const raw = localStorage.getItem(USER_KEY)
  return raw ? JSON.parse(raw) : null
}
export const setUser = (user: UserInfo) => localStorage.setItem(USER_KEY, JSON.stringify(user))
export const clearAuth = () => {
  localStorage.removeItem(TOKEN_KEY)
  localStorage.removeItem(USER_KEY)
}
export const isLoggedIn = () => !!getToken()

export function buildFeishuOAuthUrl(): string {
  const appId = import.meta.env.VITE_FS_APPID
  const redirectUri = encodeURIComponent(import.meta.env.VITE_FS_REDIRECT_URI)
  return `https://passport.feishu.cn/suite/passport/oauth/authorize?client_id=${appId}&redirect_uri=${redirectUri}&response_type=code&state=FS`
}
