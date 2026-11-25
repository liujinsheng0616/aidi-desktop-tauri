import { ref, reactive, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { platform } from '@tauri-apps/plugin-os'
import { getUser, fetchWithAuth } from './auth'
import { trackDevice, trackOptimization, trackEvent, getTrackingUserId } from '@/services/tracker'

export type Status = 'good' | 'warning' | 'danger' | 'error' | 'info' | 'scanning'

export interface ScanResult {
  dimension: 'disk' | 'memory' | 'startup' | 'health' | 'system'
  status: Status
  summary: string
  details: any
}

export interface DiskFile {
  path: string
  size: number
  modified: string
  category: string
}

export interface DiskDetails {
  files: DiskFile[]
  totalSize: number
  categories: {
    temp: { size: number; count: number; needsAuth?: boolean }
    systemTemp: { size: number; count: number; needsAuth?: boolean }
    prefetch: { size: number; count: number; needsAuth?: boolean }
    recycleBin: { size: number; count: number; needsAuth?: boolean }
    browserCache: { size: number; count: number; needsAuth?: boolean }
  }
}

export interface ProcessInfo {
  name: string
  pid: number
  memory: number
  memoryMB: number
}

export interface MemoryDetails {
  total: number
  totalGB: number
  used: number
  usedGB: number
  free: number
  freeGB: number
  usedPercent: number
  availablePercent: number
  topProcesses: ProcessInfo[]
}

export interface StartupItem {
  name: string
  command: string
  source: string
  enabled: boolean
  location: string
}

export interface StartupDetails {
  count: number
  items: StartupItem[]
}

export interface VolumeInfo {
  drive: string
  label: string
  size: number
  sizeGB: number
  free: number
  freeGB: number
  used: number
  usedGB: number
  usedPercent: number
  fileSystem: string
  status: Status
}

export interface DiskInfo {
  name: string
  mediaType: string
  size: number
  sizeGB: number
  healthStatus: string
  operationalStatus: string
}

export interface HealthDetails {
  volumes: VolumeInfo[]
  physicalDisks: DiskInfo[]
}

export interface SystemDetails {
  hostname: string
  ip?: string
  manufacturer: string
  model: string
  serialNumber: string
  manufactureDate: string
  os: {
    name: string
    version: string
    build: string
    architecture: string
    installDate: string
    lastBoot: string
  }
  cpu: {
    name: string
    cores: number
    threads: number
    maxSpeed: string
  }
  memory: {
    totalGB: number
  }
  gpu: {
    name: string
    driverVersion: string
    resolution: string
  }
  storage: {
    totalGB: number
  }
}

// Global optimization selection state
const optimizeSelections = reactive({
  disk: {
    categories: [] as string[]  // ['temp', 'recycleBin', ...]
  },
  memory: {
    enabled: false
  },
  startup: {
    items: [] as StartupItem[]  // startup items to disable
  }
})

// Computed: whether any optimization items are selected
const hasSelections = computed(() => {
  return optimizeSelections.disk.categories.length > 0 ||
         optimizeSelections.memory.enabled ||
         optimizeSelections.startup.items.length > 0
})

// Selection count for display
const selectionCount = computed(() => {
  let count = optimizeSelections.disk.categories.length
  if (optimizeSelections.memory.enabled) count += 1
  count += optimizeSelections.startup.items.length
  return count
})

// Reactive state
const isScanning = ref(false)
const isOperating = ref(false)  // 全局操作中状态（清理/优化期间置为 true）
const scanProgress = ref(0)
const scanStatus = ref('')
const lastScanTime = ref<string | null>(null)
const results = ref<ScanResult[]>([])
const error = ref<string | null>(null)
const deviceReported = ref(false)  // 设备信息是否已上报

// Platform detection using Tauri OS plugin
const isMac = ref(false)

// Initialize platform detection
const p = platform()
isMac.value = p === 'macos'

// Scan steps — macOS 和 Windows 均支持全部5项
const scanSteps = computed(() => [
  { key: 'disk', label: '磁盘清理' },
  { key: 'memory', label: '内存状态' },
  { key: 'startup', label: '启动项' },
  { key: 'health', label: '磁盘健康' },
  { key: 'system', label: '系统信息' },
])

// Computed
const diskResult = computed(() => results.value.find(r => r.dimension === 'disk'))
const memoryResult = computed(() => results.value.find(r => r.dimension === 'memory'))
const startupResult = computed(() => results.value.find(r => r.dimension === 'startup'))
const healthResult = computed(() => results.value.find(r => r.dimension === 'health'))
const systemResult = computed(() => results.value.find(r => r.dimension === 'system'))

const hasIssues = computed(() => {
  return results.value.some(r => r.status === 'warning' || r.status === 'danger')
})

// Actions
async function scanAll() {
  if (isScanning.value) return
  isScanning.value = true
  scanProgress.value = 5  // Start at 5% immediately to show activity
  scanStatus.value = '正在扫描...'
  error.value = null
  deviceReported.value = false

  // 埋点：优化器扫描启动
  const _user = getUser()
  trackEvent('optimizer_scan', { userId: _user?.fsUserId || _user?.id || getTrackingUserId() })

  try {
    // Map step keys to Tauri command names
    const tauriCommands: Record<string, string> = {
      disk: 'optimizer_disk_scan',
      memory: 'optimizer_memory_status',
      startup: 'optimizer_startup_list',
      health: 'optimizer_disk_health',
      system: 'optimizer_system_info'
    }

    const total = scanSteps.value.length
    let completed = 0
    const scanResults: ScanResult[] = []

    // Create all scan promises with progress tracking
    const scanPromises = scanSteps.value.map(async (step) => {
      try {
        const result = await invoke<ScanResult>(tauriCommands[step.key])
        completed++
        // Update progress as each task completes (reserve 5% at start and end)
        scanProgress.value = 5 + Math.round((completed / total) * 90)
        scanStatus.value = `已完成 ${step.label} (${completed}/${total})`
        return result
      } catch (e: any) {
        completed++
        scanProgress.value = 5 + Math.round((completed / total) * 90)
        return { dimension: step.key as any, status: 'error' as Status, summary: e?.message || String(e), details: {} }
      }
    })

    // Run all scans in parallel
    const allResults = await Promise.all(scanPromises)

    // Sort results back to original order
    for (const step of scanSteps.value) {
      const result = allResults.find(r => r.dimension === step.key)
      if (result) scanResults.push(result)
    }

    scanProgress.value = 100
    scanStatus.value = '扫描完成'
    results.value = scanResults
    syncStartupSelections(startupResult.value)
    lastScanTime.value = new Date().toLocaleString()

    // 上报设备信息（不阻断扫描流程）
    const systemRes = scanResults.find(r => r.dimension === 'system')
    if (systemRes && systemRes.status !== 'error') {
      reportDeviceInfo(systemRes.details as SystemDetails)
      // 同时上报到 Node.js tracker（供管理后台展示）
      const user = getUser()
      {
        const d = systemRes.details as SystemDetails
        trackDevice({
          userId: user?.fsUserId || user?.id || getTrackingUserId(),
          userName: user?.name,
          serialNumber: d.serialNumber,
          deviceModel: d.model,
          osVersion: d.os?.version,
          cpu: d.cpu?.name,
          memory: d.memory?.totalGB ? `${d.memory.totalGB}GB` : undefined,
          disk: d.storage?.totalGB ? `${d.storage.totalGB}GB` : undefined,
          rawInfo: JSON.stringify(d),
        })
      }
    }
  } catch (e: any) {
    error.value = e.message
  } finally {
    isScanning.value = false
  }
}

function optimizationIdentity() {
  const user = getUser()
  return { userId: user?.fsUserId || user?.id || getTrackingUserId(), userName: user?.name, deviceId: systemResult.value?.details?.serialNumber }
}
async function runOptimization(command: string, args: Record<string, unknown> | undefined, optType: string, detail: (result: any) => Record<string, unknown>) {
  try {
    const result: any = await invoke(command, args)
    const success = (optType === 'startup_toggle' ? result?.success === true : result?.success !== false) && !(result?.failedCount > 0)
    trackOptimization({ ...optimizationIdentity(), optType, optDetail: JSON.stringify(detail(result)), success, errorMessage: success ? undefined : result?.errorReason || result?.message || '操作部分或全部失败' })
    return result
  } catch (error) {
    trackOptimization({ ...optimizationIdentity(), optType, success: false, errorMessage: String(error) })
    throw error
  }
}
async function cleanDisk(categories: string[]) {
  return runOptimization('optimizer_disk_clean', { categoriesJson: JSON.stringify(categories) }, 'disk_clean', r => ({ categories, cleanedMB: r?.cleanedMB ?? (r?.cleaned || 0) / 1024 / 1024, failedCount: r?.failedCount || 0, details: r?.details }))
}
async function optimizeMemory() {
  return runOptimization('optimizer_memory_optimize', undefined, 'memory_optimize', r => ({ freedMB: r?.freedMB ?? (r?.freedBytes || 0) / 1024 / 1024, processesOptimized: r?.processesOptimized }))
}
async function toggleStartup(item: StartupItem, enabled: boolean) {
  const result = await runOptimization('optimizer_startup_toggle', { itemJson: JSON.stringify({ ...item, enabled }) }, 'startup_toggle', result => ({
    name: item.name,
    enabled,
    startupDisabled: !enabled && result?.success === true ? 1 : 0,
  }))
  if (result?.success === true) {
    const current = startupResult.value?.details?.items?.find((candidate: StartupItem) => sameStartupItem(candidate, item))
    if (current) current.enabled = enabled
    toggleStartupItemSelection(item, false)
  }
  return result
}

async function refreshDisk() {
  const result = await invoke<ScanResult>('optimizer_disk_scan')
  const idx = results.value.findIndex(r => r.dimension === 'disk')
  if (idx >= 0) {
    results.value[idx] = result
  }
  return result
}

async function refreshMemory() {
  const result = await invoke<ScanResult>('optimizer_memory_status')
  const idx = results.value.findIndex(r => r.dimension === 'memory')
  if (idx >= 0) {
    results.value[idx] = result
  }
  return result
}

async function refreshStartup() {
  const result = await invoke<ScanResult>('optimizer_startup_list')
  const idx = results.value.findIndex(r => r.dimension === 'startup')
  if (idx >= 0 && result.status !== 'error') {
    results.value[idx] = result
  }
  syncStartupSelections(result)
  return result
}

// Optimize result for UI feedback
const lastOptimizeResult = ref<{
  success: boolean
  diskCleaned?: number
  memoryFreed?: number
  startupDisabled?: number
} | null>(null)

// Quick optimize: execute all selected optimizations
async function quickOptimize() {
  const tasks: Promise<any>[] = []
  const result = {
    success: true,
    diskCleaned: 0,
    memoryFreed: 0,
    startupDisabled: 0
  }

  // Clean disk if categories selected
  if (optimizeSelections.disk.categories.length > 0) {
    tasks.push(
      cleanDisk([...optimizeSelections.disk.categories])
        .then(r => { result.diskCleaned = r?.cleaned || 0; if (r?.success === false || r?.failedCount > 0) result.success = false })
    )
  }

  // Optimize memory if enabled
  if (optimizeSelections.memory.enabled) {
    tasks.push(
      optimizeMemory()
        .then(r => { result.memoryFreed = r?.freedBytes ?? (r?.freedMB || 0) * 1024 * 1024; if (r?.success === false) result.success = false })
    )
  }

  // Disable selected startup items
  const startupItems = [...optimizeSelections.startup.items]
  const operationCount = tasks.length + startupItems.length
  if (startupItems.length > 0) {
    tasks.push((async () => {
      for (const item of startupItems) {
        try {
          const outcome = await toggleStartup(item, false)
          if (outcome?.success === true) result.startupDisabled++
          else result.success = false
        } catch {
          result.success = false
        }
      }
    })())
  }

  // 等待所有结果，部分失败也保留已经成功的操作记录。
  const settled = await Promise.allSettled(tasks)
  result.success = result.success && settled.every(r => r.status === 'fulfilled')
  lastOptimizeResult.value = result
  trackOptimization({ ...optimizationIdentity(), optType: 'quick_optimize', optDetail: JSON.stringify({ ...result, operationCount }), success: result.success, errorMessage: result.success ? undefined : '部分优化未完成，请查看单项记录' })

  // Clear selections after completion
  optimizeSelections.disk.categories = []
  optimizeSelections.memory.enabled = false
  optimizeSelections.startup.items = []

  // Return result (don't auto-refresh, let UI handle it)
  return result
}

// Helper functions for managing selections
function setDiskCategories(categories: string[]) {
  optimizeSelections.disk.categories = categories
}

function toggleDiskCategory(category: string, enabled: boolean) {
  if (enabled) {
    if (!optimizeSelections.disk.categories.includes(category)) {
      optimizeSelections.disk.categories.push(category)
    }
  } else {
    const idx = optimizeSelections.disk.categories.indexOf(category)
    if (idx >= 0) {
      optimizeSelections.disk.categories.splice(idx, 1)
    }
  }
}

function setMemoryOptimizeEnabled(enabled: boolean) {
  optimizeSelections.memory.enabled = enabled
}

function sameStartupItem(candidate: StartupItem, item: StartupItem) {
  return candidate.source === item.source && (item.source === 'StartupFolder'
    ? candidate.command === item.command
    : candidate.name === item.name)
}

function syncStartupSelections(scan?: ScanResult) {
  const items: StartupItem[] = scan?.status !== 'error' && Array.isArray(scan?.details?.items) ? scan.details.items : []
  optimizeSelections.startup.items = items.filter(item => item.enabled && isStartupItemSelected(item))
}

function toggleStartupItemSelection(item: StartupItem, selected: boolean) {
  const idx = optimizeSelections.startup.items.findIndex(candidate => sameStartupItem(candidate, item))
  if (selected) {
    if (item.enabled && idx < 0) {
      optimizeSelections.startup.items.push(item)
    }
  } else {
    if (idx >= 0) {
      optimizeSelections.startup.items.splice(idx, 1)
    }
  }
}

function isStartupItemSelected(item: StartupItem): boolean {
  return optimizeSelections.startup.items.some(candidate => sameStartupItem(candidate, item))
}

// 上报设备信息到 aigc-server（忽略错误，不阻断扫描流程）
async function reportDeviceInfo(systemInfo: SystemDetails) {
  try {
    const user = getUser()
    if (!user) return
    const payload = {
      userCode: user.fsUserId,
      userName: user?.name,
      hostname: systemInfo.hostname,
      ip: systemInfo.ip || '',
      manufacturer: systemInfo.manufacturer,
      model: systemInfo.model,
      serialNumber: systemInfo.serialNumber,
      manufactureDate: systemInfo.manufactureDate,
      osName: systemInfo.os.name,
      osVersion: systemInfo.os.version,
      osArch: systemInfo.os.architecture,
      osInstallDate: systemInfo.os.installDate,
      osLastBoot: systemInfo.os.lastBoot,
      cpuName: systemInfo.cpu.name,
      cpuCores: systemInfo.cpu.cores,
      memoryGb: systemInfo.memory.totalGB,
      storageGb: systemInfo.storage.totalGB,
      gpuName: systemInfo.gpu.name,
    }
    await fetchWithAuth('/api-aigc/device/info/report', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    deviceReported.value = true
  } catch (e) {
    deviceReported.value = false
  }
}

// Export composable
export function useOptimizer() {
  return {
    // State
    isScanning,
    isOperating,
    scanProgress,
    scanStatus,
    lastScanTime,
    results,
    error,
    lastOptimizeResult,
    deviceReported,

    // Optimization selections
    optimizeSelections,
    hasSelections,
    selectionCount,

    // Computed
    diskResult,
    memoryResult,
    startupResult,
    healthResult,
    systemResult,
    hasIssues,

    // Actions
    scanAll,
    cleanDisk,
    optimizeMemory,
    toggleStartup,
    refreshDisk,
    refreshMemory,
    refreshStartup,
    quickOptimize,
    setDiskCategories,
    toggleDiskCategory,
    setMemoryOptimizeEnabled,
    toggleStartupItemSelection,
    isStartupItemSelected,
  }
}
