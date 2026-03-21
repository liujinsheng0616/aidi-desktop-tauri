<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
import { Card, CardContent, CardFooter } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import StatusCard from './StatusCard.vue'
import DiskClean from './DiskClean.vue'
import MemoryStatus from './MemoryStatus.vue'
import StartupManager from './StartupManager.vue'
import DiskHealth from './DiskHealth.vue'
import SystemInfo from './SystemInfo.vue'
import { useOptimizer } from '@/stores/optimizer'
import { listen } from '@tauri-apps/api/event'

// Platform detection
const nav = navigator as any
const isWin = nav.userAgentData
  ? nav.userAgentData.platform === 'Windows'
  : navigator.platform.startsWith('Win')

const {
  isScanning,
  scanProgress,
  scanStatus,
  lastScanTime,
  diskResult,
  memoryResult,
  startupResult,
  healthResult,
  systemResult,
  hasSelections,
  selectionCount,
  scanAll,
  quickOptimize,
  deviceReported,
} = useOptimizer()

const openCards = ref<Record<string, boolean>>({
  disk: false,
  memory: false,
  startup: false,
  health: false,
  system: false,
})

const isQuickOptimizing = ref(false)
const toastMessage = ref('')
const toastType = ref<'info' | 'success'>('info')
const showOptimizeSuccess = ref(false)
const optimizeResultSummary = ref('')

function showToast(msg: string, type: 'info' | 'success' = 'info') {
  toastMessage.value = msg
  toastType.value = type
  setTimeout(() => toastMessage.value = '', 3000)
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB'
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB'
}

function applyThemeFromMode(mode: string) {
  let isDark = mode === 'dark'
  if (mode === 'system') {
    isDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  }
  document.documentElement.classList.toggle('dark', isDark)
}

function applyTheme() {
  const saved = localStorage.getItem('aidi-settings')
  let mode = 'system'
  if (saved) {
    const s = JSON.parse(saved)
    mode = s.themeMode || 'system'
  }
  applyThemeFromMode(mode)
}

function onSettingsUpdated(settings: any) {
  if (settings?.themeMode) {
    applyThemeFromMode(settings.themeMode)
  }
}

async function handleQuickOptimize() {
  if (!hasSelections.value) {
    showToast('请先勾选要优化的项目')
    return
  }

  isQuickOptimizing.value = true
  try {
    const result = await quickOptimize()

    // Build success message
    const parts: string[] = []
    if (result.diskCleaned && result.diskCleaned > 0) {
      parts.push(`清理 ${formatBytes(result.diskCleaned)}`)
    }
    if (result.memoryFreed && result.memoryFreed > 0) {
      parts.push(`释放内存 ${formatBytes(result.memoryFreed)}`)
    }
    if (result.startupDisabled && result.startupDisabled > 0) {
      parts.push(`禁用 ${result.startupDisabled} 个启动项`)
    }

    optimizeResultSummary.value = parts.length > 0 ? parts.join('，') : '优化完成'
    showOptimizeSuccess.value = true

    // Show success state for 2 seconds before rescanning
    await new Promise(resolve => setTimeout(resolve, 2000))
    showOptimizeSuccess.value = false

    // Now rescan
    await scanAll()
  } catch (e) {
    showToast('优化失败，请重试')
  } finally {
    isQuickOptimizing.value = false
  }
}

let unlistenSettings: (() => void) | null = null

onMounted(async () => {
  applyTheme()
  scanAll()
  unlistenSettings = await listen<any>('settings-updated', (event) => {
    onSettingsUpdated(event.payload)
  })
})

onUnmounted(() => {
  if (unlistenSettings) unlistenSettings()
})
</script>

<template>
  <div class="optimizer-wrapper">
    <Card class="optimizer-card border-0 shadow-xl !p-0 !gap-0">
      <!-- Scrollable Content -->
      <CardContent class="scrollable-content p-3 space-y-2">
        <!-- Refresh button row -->
        <div class="flex justify-end -mt-1 mb-1">
          <Button variant="ghost" size="icon-sm" :disabled="isScanning" @click="scanAll">
            <RefreshCw :size="14" :class="{ 'animate-spin': isScanning }" />
          </Button>
        </div>
        <!-- Optimize success animation -->
        <div v-if="showOptimizeSuccess" class="text-center py-8">
          <div class="text-5xl mb-4 success-bounce">✅</div>
          <div class="text-base font-semibold text-green-600 dark:text-green-400 mb-2">优化完成！</div>
          <div class="text-sm text-muted-foreground">{{ optimizeResultSummary }}</div>
        </div>

        <!-- Scanning progress -->
        <div v-else-if="isScanning" class="text-center py-6">
          <div class="text-4xl mb-4">🔍</div>
          <div class="text-sm font-medium mb-1">{{ scanStatus }}</div>
          <div class="text-xs text-muted-foreground mb-3">{{ scanProgress }}%</div>
          <Progress :model-value="scanProgress" class="h-2 w-48 mx-auto" />
        </div>

        <!-- Results -->
        <template v-else>
          <!-- Device report success message -->
          <div v-if="deviceReported" class="text-center py-2 px-3 mb-3 bg-green-50 dark:bg-green-950 border border-green-200 dark:border-green-800 rounded-lg">
            <div class="flex items-center justify-center gap-2 text-sm text-green-700 dark:text-green-300">
              <span>✅</span>
              <span>设备信息已上报</span>
            </div>
          </div>

          <!-- Disk -->
          <StatusCard
            v-if="diskResult"
            icon="💾"
            title="磁盘清理"
            :status="diskResult.status"
            :summary="diskResult.summary"
            v-model:open="openCards.disk"
          >
            <DiskClean v-if="diskResult.details" :details="diskResult.details" />
          </StatusCard>

          <!-- Memory (Windows only) -->
          <StatusCard
            v-if="isWin && memoryResult"
            icon="🧠"
            title="内存状态"
            :status="memoryResult.status"
            :summary="memoryResult.summary"
            v-model:open="openCards.memory"
          >
            <MemoryStatus v-if="memoryResult.details" :details="memoryResult.details" />
          </StatusCard>

          <!-- Startup -->
          <StatusCard
            v-if="startupResult"
            icon="🚀"
            title="启动项管理"
            :status="startupResult.status"
            :summary="startupResult.summary"
            v-model:open="openCards.startup"
          >
            <StartupManager v-if="startupResult.details" :details="startupResult.details" />
          </StatusCard>

          <!-- Disk Health (Windows only) -->
          <StatusCard
            v-if="isWin && healthResult"
            icon="💿"
            title="磁盘健康"
            :status="healthResult.status"
            :summary="healthResult.summary"
            v-model:open="openCards.health"
          >
            <DiskHealth v-if="healthResult.details" :details="healthResult.details" />
          </StatusCard>

          <!-- System Info -->
          <StatusCard
            v-if="systemResult"
            icon="💻"
            title="系统信息"
            :status="systemResult.status"
            :summary="systemResult.summary"
            v-model:open="openCards.system"
          >
            <SystemInfo v-if="systemResult.details" :details="systemResult.details" />
          </StatusCard>

          <!-- Quick optimize button -->
          <Button
            class="w-full mt-2"
            :disabled="isQuickOptimizing"
            @click="handleQuickOptimize"
          >
            <template v-if="isQuickOptimizing">优化中...</template>
            <template v-else-if="selectionCount > 0">一键优化 ({{ selectionCount }} 项)</template>
            <template v-else>一键优化</template>
          </Button>

          <!-- Toast message -->
          <div v-if="toastMessage" :class="['toast-message', toastType === 'success' ? 'toast-success' : '']">
            {{ toastMessage }}
          </div>
        </template>
      </CardContent>

      <CardFooter v-if="lastScanTime" class="p-2 pt-0 justify-center">
        <span class="text-xs text-muted-foreground">
          上次扫描: {{ lastScanTime }}
        </span>
      </CardFooter>
    </Card>
  </div>
</template>

<style scoped>
.optimizer-wrapper {
  padding: 10px;
  height: 100vh;
  box-sizing: border-box;
}

.optimizer-card {
  border-radius: 12px;
  background: var(--background);
  height: 100%;
  display: flex;
  flex-direction: column;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  overflow: hidden;
}

.scrollable-content {
  flex: 1;
  overflow-y: auto;
}

/* Custom scrollbar */
.scrollable-content::-webkit-scrollbar {
  width: 6px;
}

.scrollable-content::-webkit-scrollbar-track {
  background: transparent;
}

.scrollable-content::-webkit-scrollbar-thumb {
  background: color-mix(in oklch, var(--muted-foreground) 30%, transparent);
  border-radius: 3px;
}

.scrollable-content::-webkit-scrollbar-thumb:hover {
  background: color-mix(in oklch, var(--muted-foreground) 50%, transparent);
}

.toast-message {
  margin-top: 8px;
  padding: 8px 12px;
  background: var(--muted);
  border: 1px solid var(--border);
  border-radius: 6px;
  font-size: 12px;
  text-align: center;
  color: var(--muted-foreground);
  animation: toast-fade-in 0.2s ease-out;
}

@keyframes toast-fade-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.toast-success {
  background: hsl(142 76% 36% / 0.15);
  border-color: hsl(142 76% 36% / 0.3);
  color: hsl(142 76% 30%);
}

.dark .toast-success {
  background: hsl(142 76% 36% / 0.2);
  border-color: hsl(142 76% 36% / 0.4);
  color: hsl(142 76% 60%);
}

.success-bounce {
  animation: success-bounce 0.6s ease-out;
}

@keyframes success-bounce {
  0% {
    opacity: 0;
    transform: scale(0.3);
  }
  50% {
    transform: scale(1.1);
  }
  70% {
    transform: scale(0.95);
  }
  100% {
    opacity: 1;
    transform: scale(1);
  }
}
</style>
