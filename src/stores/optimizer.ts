import { ref, reactive, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { platform } from '@tauri-apps/plugin-os'
import { getUser, fetchWithAuth } from './auth'

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

// Platform-specific scan steps
// macOS: 磁盘清理、启动项管理、系统信息
// Windows: 全部5项
const scanSteps = computed(() => isMac.value
  ? [
      { key: 'disk', label: '磁盘清理' },
      { key: 'startup', label: '启动项' },
      { key: 'system', label: '系统信息' },
    ]
  : [
      { key: 'disk', label: '磁盘清理' },
      { key: 'memory', label: '内存状态' },
      { key: 'startup', label: '启动项' },
      { key: 'health', label: '磁盘健康' },
      { key: 'system', label: '系统信息' },
    ]
)

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
  isScanning.value = true
  scanProgress.value = 5  // Start at 5% immediately to show activity
  scanStatus.value = '正在扫描...'
  error.value = null
  deviceReported.value = false

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
        return { dimension: step.key as any, status: 'error' as Status, summary: e.message, details: {} }
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
    lastScanTime.value = new Date().toLocaleString()

    // 上报设备信息（不阻断扫描流程）
    const systemRes = scanResults.find(r => r.dimension === 'system')
    if (systemRes && systemRes.status !== 'error') {
      reportDeviceInfo(systemRes.details as SystemDetails)
    }
  } catch (e: any) {
    error.value = e.message
  } finally {
    isScanning.value = false
  }
}

async function cleanDisk(categories: string[]) {
  const categoriesJson = JSON.stringify(categories)
  return await invoke('optimizer_disk_clean', { categoriesJson })
}

async function optimizeMemory() {
  return await invoke('optimizer_memory_optimize')
}

async function toggleStartup(item: StartupItem, enabled: boolean) {
  const itemJson = JSON.stringify({ ...item, enabled })
  return await invoke('optimizer_startup_toggle', { itemJson })
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
  if (idx >= 0) {
    results.value[idx] = result
  }
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
        .then(r => { result.diskCleaned = (r as any)?.cleaned || 0 })
    )
  }

  // Optimize memory if enabled
  if (optimizeSelections.memory.enabled) {
    tasks.push(
      optimizeMemory()
        .then(r => { result.memoryFreed = (r as any)?.freedBytes || 0 })
    )
  }

  // Disable selected startup items
  if (optimizeSelections.startup.items.length > 0) {
    const startupCount = optimizeSelections.startup.items.length
    for (const item of optimizeSelections.startup.items) {
      tasks.push(toggleStartup(item, false))
    }
    result.startupDisabled = startupCount
  }

  // Execute all tasks in parallel
  await Promise.all(tasks)

  // Store result for UI feedback
  lastOptimizeResult.value = result

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

function toggleStartupItemSelection(item: StartupItem, selected: boolean) {
  const idx = optimizeSelections.startup.items.findIndex(
    i => i.name === item.name && i.source === item.source
  )
  if (selected) {
    if (idx < 0) {
      optimizeSelections.startup.items.push(item)
    }
  } else {
    if (idx >= 0) {
      optimizeSelections.startup.items.splice(idx, 1)
    }
  }
}

function isStartupItemSelected(item: StartupItem): boolean {
  return optimizeSelections.startup.items.some(
    i => i.name === item.name && i.source === item.source
  )
}

// 上报设备信息到 aigc-server（忽略错误，不阻断扫描流程）
async function reportDeviceInfo(systemInfo: SystemDetails) {
  try {
    const user = getUser()
    if (!user) return
    const payload = {
      userCode: user.fsUserId,
      userName: user.name,
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
    console.warn('Device info report failed:', e)
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
