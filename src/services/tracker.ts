/** 持久化遥测上报。两个独立工程保持相同协议。 */
export interface TrackEvent {
  eventType: string
  userId: string
  userName?: string
  deviceId?: string
  sceneId?: string
  model?: string
  durationMs?: number
  success?: boolean
  detail?: Record<string, unknown>
  appVersion?: string
  platform?: string
}

export interface ConversationData {
  difyConversationId?: string
  difyMessageId?: string
  userId: string
  userName?: string
  query: string
  sceneIds?: string
  model: string
  enableWikiSearch?: boolean
  hasScreenshot?: boolean
  hasDiagnostic?: boolean
  responseDurationMs?: number
  responseLength?: number
  responsePreview?: string
  tokenUsage?: string
  success?: boolean
  errorMessage?: string
}

export interface DiagnosticData {
  userId: string
  userName?: string
  conversationId?: string
  [key: string]: unknown
}

export interface DeviceData {
  userId?: string
  userName?: string
  serialNumber?: string
  deviceModel?: string
  osVersion?: string
  cpu?: string
  memory?: string
  disk?: string
  rawInfo?: string
}

export interface OptimizationData {
  userId?: string
  userName?: string
  deviceId?: string
  optType: string
  optDetail?: string
  success?: boolean
  errorMessage?: string
}

// 每条记录独立持久化，避免多个窗口覆盖同一队列；收到数据库确认后才删除。
const BASE = import.meta.env.VITE_TRACKER_BASE_URL || 'http://localhost:46291'
const PREFIX = 'aidi-outbox-v2:'
let timer: ReturnType<typeof setInterval> | null = null
let flushing = false

interface TelemetrySession { token: string; expiresAt: number; userId: string }
async function telemetrySession(): Promise<TelemetrySession | null> {
  try {
    if ('__TAURI_INTERNALS__' in window) {
      const { invoke } = await import('@tauri-apps/api/core')
      const result = await invoke<TelemetrySession | null>('get_telemetry_session')
      if (result?.token && result.expiresAt > Date.now()) return result
    }
  } catch {}
  return null
}

function getAnonymousTrackingId() {
  let id = localStorage.getItem('aidi-anonymous-user-id')
  if (!id) {
    id = 'anonymous-' + crypto.randomUUID()
    localStorage.setItem('aidi-anonymous-user-id', id)
  }
  return id
}
export function getTrackingUserId(): string {
  try {
    const url = new URLSearchParams(location.hash.split('?')[1] || '').get('userId')
    if (url) return url
    const u = JSON.parse(localStorage.getItem('aidi-user') || '{}')
    if (u.fsUserId || u.id) return String(u.fsUserId || u.id)
    return getAnonymousTrackingId()
  } catch { return 'anonymous-session' }
}
function platform() { return /Windows/i.test(navigator.userAgent) ? 'win' : /Mac/i.test(navigator.userAgent) ? 'mac' : 'unknown' }
async function flush() {
  if (flushing) return
  flushing = true
  try {
    const keys = Object.keys(localStorage).filter(k => k.startsWith(PREFIX)).sort()
    let attempted = 0
    let identity: Promise<TelemetrySession | null> | undefined
    for (const key of keys) {
      const text = localStorage.getItem(key)
      if (!text) continue
      try {
        const item = JSON.parse(text)
        if (item.nextAttemptAt && item.nextAttemptAt > Date.now()) continue
        if (attempted >= 50) break
        attempted++
        const reportedUser = item.body.events?.[0]?.userId ?? item.body.userId
        if (typeof reportedUser === 'string' && !reportedUser.startsWith('anonymous-')) identity ??= telemetrySession()
        const session = identity ? await identity : null
        const credential = session?.userId === reportedUser ? session?.token : undefined
        const headers: Record<string, string> = {
          'Content-Type': 'application/json',
          'X-AIDI-Anonymous-Id': getAnonymousTrackingId(),
          ...(credential ? { Authorization: 'Bearer ' + credential } : {}),
        }
        const response = await fetch(BASE + item.endpoint, {
          method: 'POST', headers, body: JSON.stringify(item.body),
          signal: AbortSignal.timeout(10000),
        })
        if ([400, 404, 413, 422].includes(response.status)) {
          localStorage.setItem('aidi-rejected-v2:' + key.slice(PREFIX.length), JSON.stringify({ ...item, status: response.status, rejectedAt: new Date().toISOString() }))
          localStorage.removeItem(key)
          console.warn('[tracker] 无法接收的记录已单独保留', response.status)
          continue
        }
        const ack = await response.json()
        if (!response.ok || ack.ok !== true || ack.skipped > 0) throw new Error('上报未确认: ' + response.status)
        localStorage.removeItem(key)
      } catch (error) {
        console.warn('[tracker] 数据保留，稍后补发', error)
        try {
          const saved = JSON.parse(text)
          saved.attempts = (saved.attempts || 0) + 1
          saved.nextAttemptAt = Date.now() + Math.min(300000, 10000 * 2 ** Math.min(saved.attempts - 1, 5))
          localStorage.setItem(key, JSON.stringify(saved))
        } catch {}
      }
    }
  } finally { flushing = false }
}
function retry() { void flush().catch(error => console.warn('[tracker]', error)) }
export function initTracker() {
  if (timer) return
  timer = setInterval(retry, 10000)
  window.addEventListener('online', retry)
  // 不使用无法确认写库结果的 sendBeacon；未确认记录留待下次启动补发。
  retry()
}
function enqueue(endpoint: string, data: Record<string, unknown>, event = false) {
  try {
    initTracker()
    const id = crypto.randomUUID()
    const optimization = endpoint === '/api/optimizations'
    const body = {
      ...(event || optimization ? { deviceId: localStorage.getItem('aidi-device-serial') || undefined } : {}),
      ...data,
      userId: data.userId && data.userId !== 'anonymous' ? data.userId : getTrackingUserId(),
      ...(event ? { eventId: id } : { reportId: id }),
      ...(event || optimization || endpoint === '/api/conversations' ? { occurredAt: new Date().toISOString() } : {}),
    }
    localStorage.setItem(PREFIX + id, JSON.stringify({ endpoint, body: event ? { events: [body] } : body }))
    retry()
  } catch (error) { console.warn('[tracker] 无法持久化上报数据', error) }
}
export function track(eventType: string, props: Partial<TrackEvent> = {}) {
  enqueue('/api/events/batch', { ...props, eventType, platform: props.platform || platform() }, true)
}
export const trackEvent = track
export function trackConversation(data: ConversationData) { enqueue('/api/conversations', { ...data }) }
export function trackDiagnostic(data: DiagnosticData) { enqueue('/api/diagnostics', { ...data }) }
export function trackDevice(data: DeviceData) { enqueue('/api/devices', { ...data }) }
export function trackOptimization(data: OptimizationData) { enqueue('/api/optimizations', { ...data }) }
export function destroyTracker() {
  if (timer) clearInterval(timer)
  timer = null
  window.removeEventListener('online', retry)
}

/** 主窗口启动时收集一次设备快照，不依赖用户打开优化器。 */
export async function trackDesktopStartup() {
  initTracker()
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const login = await invoke<any>('get_login_info')
    const u = typeof login?.user === 'string' ? JSON.parse(login.user) : login?.user
    const userId = u?.fsUserId || u?.id || getTrackingUserId()
    track('app_launch', { userId })
    const snapshot = await invoke<any>('optimizer_system_info')
    if (snapshot?.status === 'error' || !snapshot?.details) return
    const d = snapshot.details
    if (d.serialNumber) localStorage.setItem('aidi-device-serial', d.serialNumber)
    trackDevice({ userId, serialNumber: d.serialNumber, deviceModel: d.model, osVersion: d.os?.version, cpu: d.cpu?.name, memory: d.memory?.totalGB ? `${d.memory.totalGB}GB` : undefined, disk: d.storage?.totalGB ? `${d.storage.totalGB}GB` : undefined, rawInfo: JSON.stringify(d) })
  } catch (error) { console.warn('[tracker] 启动快照采集失败', error) }
}
