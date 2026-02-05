<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import type { DiskDetails } from '@/stores/optimizer'
import { useOptimizer } from '@/stores/optimizer'

interface Props {
  details: DiskDetails
}

const props = defineProps<Props>()

const { cleanDisk, refreshDisk, optimizeSelections, toggleDiskCategory } = useOptimizer()

interface CleanDetail {
  category: string
  path: string
  size: number
  status: 'success' | 'failed' | 'partial'
  reason: string
}

interface CleanResult {
  cleaned: number
  cleanedMB: number
  successCount: number
  failedCount: number
  details: CleanDetail[]
}

const isCleaning = ref(false)
const cleanResult = ref<CleanResult | null>(null)
const showConfirm = ref(false)
const showDetails = ref(false)

// Pagination for details
const PAGE_SIZE = 20
const visibleCount = ref(PAGE_SIZE)

const isMac = navigator.platform.toLowerCase().includes('mac')
const isWin = navigator.platform.toLowerCase().includes('win')

// Categories that require admin privileges on Windows
const ELEVATED_CATEGORIES = ['systemTemp', 'prefetch']

// Note: Tauri doesn't need macOS Full Disk Access for these operations
// The native Rust implementation handles permissions differently than Electron

// Check if a category requires admin on Windows
function needsAdminOnWindows(key: string): boolean {
  return isWin && ELEVATED_CATEGORIES.includes(key)
}

// Tooltip state
const activeTooltip = ref<{ key: string; tip: string; x: number; y: number } | null>(null)

function showTooltip(event: MouseEvent, cat: { key: string; tip: string }) {
  const target = event.currentTarget as HTMLElement
  const rect = target.getBoundingClientRect()
  activeTooltip.value = {
    key: cat.key,
    tip: cat.tip,
    x: rect.left,
    y: rect.top - 8
  }
}

function hideTooltip() {
  activeTooltip.value = null
}

// Use computed property to sync with store selections
const selectedCategories = computed(() => {
  const result: Record<string, boolean> = {
    temp: false,
    systemTemp: false,
    prefetch: false,
    recycleBin: false,
    browserCache: false,
  }
  for (const cat of optimizeSelections.disk.categories) {
    result[cat] = true
  }
  return result
})

function toggleCategory(key: string, checked: boolean) {
  toggleDiskCategory(key, checked)
}

const categories = computed(() => isMac ? [
  { key: 'temp', label: '临时文件', icon: '📁', tip: '系统和应用产生的临时文件，清理后自动重建' },
  { key: 'systemTemp', label: '应用缓存', icon: '🗂️', tip: '各应用的缓存数据（Xcode、Docker、npm等），清理后首次启动稍慢' },
  { key: 'prefetch', label: '系统日志', icon: '📋', tip: '系统和应用的日志文件，仅清理30天以上的旧日志' },
  { key: 'recycleBin', label: '废纸篓', icon: '🗑️', tip: '已删除的文件，清理后无法恢复' },
  { key: 'browserCache', label: '浏览器缓存', icon: '🌐', tip: '网页缓存，不影响历史记录、密码和书签' },
] : [
  { key: 'temp', label: '用户临时文件', icon: '📁', tip: '用户临时目录中的文件，清理后自动重建' },
  { key: 'systemTemp', label: '系统临时文件', icon: '🗂️', tip: 'Windows系统临时目录，清理后自动重建' },
  { key: 'prefetch', label: '预读取文件', icon: '⚡', tip: '程序预加载缓存，清理后系统会自动重建' },
  { key: 'recycleBin', label: '回收站', icon: '🗑️', tip: '已删除的文件，清理后无法恢复' },
  { key: 'browserCache', label: '浏览器缓存', icon: '🌐', tip: '网页缓存，不影响历史记录、密码和书签' },
])

const selectedSize = computed(() => {
  let size = 0
  for (const cat of categories.value) {
    if (selectedCategories.value[cat.key]) {
      size += props.details.categories[cat.key as keyof typeof props.details.categories]?.size || 0
    }
  }
  return size
})

const selectedSizeFormatted = computed(() => formatSize(selectedSize.value))

const hasSelection = computed(() => {
  return Object.values(selectedCategories.value).some(v => v) && selectedSize.value > 0
})

// Paginated details
const visibleDetails = computed(() => {
  if (!cleanResult.value?.details) return []
  return cleanResult.value.details.slice(0, visibleCount.value)
})

const hasMoreDetails = computed(() => {
  if (!cleanResult.value?.details) return false
  return cleanResult.value.details.length > visibleCount.value
})

const remainingCount = computed(() => {
  if (!cleanResult.value?.details) return 0
  return cleanResult.value.details.length - visibleCount.value
})

function loadMoreDetails() {
  visibleCount.value += PAGE_SIZE
}

function resetDetailsPagination() {
  visibleCount.value = PAGE_SIZE
}

function formatSize(bytes: number) {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(2) + ' MB'
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB'
}

async function handleClean() {
  isCleaning.value = true
  cleanResult.value = null
  resetDetailsPagination()

  // Save selected keys before cleaning
  const selectedKeys = Object.entries(selectedCategories.value)
    .filter(([_, selected]) => selected)
    .map(([key]) => key)

  try {
    const result = await cleanDisk(selectedKeys as any)
    cleanResult.value = result as any

    // Refresh disk data after cleaning completes
    await refreshDisk()
  } catch (e) {
    console.error('Clean failed:', e)
  } finally {
    isCleaning.value = false
  }

  // Reset selected categories after cleaning is fully complete
  for (const key of selectedKeys) {
    toggleDiskCategory(key, false)
  }
}
</script>

<template>
  <div class="space-y-3">
    <div class="space-y-1">
      <div
        v-for="cat in categories"
        :key="cat.key"
        class="category-item flex items-center justify-between text-xs p-1.5 rounded transition-colors cursor-default"
        @mouseenter="showTooltip($event, cat)"
        @mouseleave="hideTooltip"
      >
        <span class="flex items-center gap-2">
          <span>{{ cat.icon }}</span>
          <span>{{ cat.label }}</span>
          <span v-if="needsAdminOnWindows(cat.key)" class="admin-badge" title="需要管理员权限">
            Admin
          </span>
        </span>
        <div class="flex items-center gap-2">
          <span class="text-muted-foreground">
            {{ formatSize((details.categories as any)[cat.key]?.size || 0) }}
          </span>
          <Switch
            :model-value="selectedCategories[cat.key]"
            :disabled="isCleaning"
            @update:model-value="toggleCategory(cat.key, $event as boolean)"
            class="scale-75"
          />
        </div>
      </div>
    </div>

    <div class="flex items-center justify-between pt-2 border-t">
      <span class="text-sm font-medium">已选: {{ selectedSizeFormatted }}</span>
      <Button size="sm" :disabled="isCleaning || !hasSelection" @click="showConfirm = true">
        {{ isCleaning ? '清理中...' : '清理所选' }}
      </Button>
    </div>

    <!-- 确认弹窗 -->
    <Teleport to="body">
      <div v-if="showConfirm" class="confirm-overlay" @click.self="showConfirm = false">
        <div class="confirm-dialog">
          <div class="confirm-title">确认清理</div>
          <div class="confirm-desc">
            将清理 <strong>{{ selectedSizeFormatted }}</strong> 的文件。<br>
            此操作无法撤销，请确认后继续。
          </div>
          <div class="confirm-actions">
            <Button variant="outline" size="sm" @click="showConfirm = false">取消</Button>
            <Button variant="destructive" size="sm" @click="handleClean(); showConfirm = false">确认清理</Button>
          </div>
        </div>
      </div>
    </Teleport>

    <div v-if="isCleaning" class="text-xs text-muted-foreground p-2 rounded bg-muted/50">
      清理中...
    </div>
    <div v-else-if="cleanResult" class="clean-result space-y-2">
      <div class="flex items-center justify-between text-xs p-2 rounded bg-green-50 dark:bg-green-900/20">
        <span class="text-green-600 dark:text-green-400">
          已清理 {{ formatSize(cleanResult.cleaned || 0) }} ({{ cleanResult.successCount }} 个文件)
          <span v-if="cleanResult.failedCount > 0" class="text-red-500 dark:text-red-400 ml-2">
            {{ cleanResult.failedCount }} 个失败
          </span>
        </span>
        <button
          v-if="cleanResult.details && cleanResult.details.length > 0"
          class="text-primary hover:underline"
          @click="showDetails = !showDetails"
        >
          {{ showDetails ? '收起' : '查看明细' }}
        </button>
      </div>

      <div v-if="showDetails && cleanResult.details" class="details-list text-xs border rounded max-h-48 overflow-y-auto">
        <div
          v-for="(detail, idx) in visibleDetails"
          :key="idx"
          class="detail-item flex items-center justify-between p-1.5 border-b last:border-b-0"
          :class="detail.status === 'failed' ? 'bg-red-50 dark:bg-red-900/10' : ''"
        >
          <div class="flex-1 min-w-0">
            <div class="truncate text-muted-foreground" :title="detail.path">
              {{ detail.path.split('/').pop() }}
            </div>
            <div v-if="detail.status === 'failed'" class="text-red-500 text-[10px]">
              {{ detail.reason }}
            </div>
          </div>
          <div class="flex items-center gap-2 ml-2 shrink-0">
            <span class="text-muted-foreground">{{ formatSize(detail.size) }}</span>
            <span v-if="detail.status === 'success'" class="text-green-500">OK</span>
            <span v-else class="text-red-500">失败</span>
          </div>
        </div>
        <!-- Load more button -->
        <div v-if="hasMoreDetails" class="load-more-container">
          <button class="load-more-btn" @click="loadMoreDetails">
            加载更多 ({{ remainingCount }} 条)
          </button>
        </div>
      </div>
    </div>

    <!-- Fixed position tooltip -->
    <Teleport to="body">
      <div
        v-if="activeTooltip"
        class="fixed-tooltip"
        :style="{
          left: activeTooltip.x + 'px',
          top: activeTooltip.y + 'px',
        }"
      >
        {{ activeTooltip.tip }}
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.category-item:hover {
  background: var(--accent);
}

.admin-badge {
  font-size: 9px;
  padding: 1px 4px;
  background: #f59e0b;
  color: #fff;
  border-radius: 3px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.fixed-tooltip {
  position: fixed;
  transform: translateY(-100%);
  padding: 6px 10px;
  background: #1a1a1a;
  color: #fff;
  border: 1px solid #333;
  border-radius: 6px;
  font-size: 11px;
  white-space: nowrap;
  z-index: 99999;
  box-shadow: 0 2px 10px rgba(0,0,0,0.3);
  pointer-events: none;
}

:root:not(.dark) .fixed-tooltip {
  background: #fff;
  color: #333;
  border-color: #ddd;
  box-shadow: 0 2px 10px rgba(0,0,0,0.15);
}

.fixed-tooltip::after {
  content: '';
  position: absolute;
  top: 100%;
  left: 12px;
  border: 5px solid transparent;
  border-top-color: #1a1a1a;
}

:root:not(.dark) .fixed-tooltip::after {
  border-top-color: #fff;
}

.confirm-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.confirm-dialog {
  background: #1c1c1e;
  border: 1px solid #3a3a3c;
  border-radius: 12px;
  padding: 20px;
  width: 300px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

:root:not(.dark) .confirm-dialog {
  background: #ffffff;
  border-color: #e5e5e5;
}

.confirm-title {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 12px;
}

.confirm-desc {
  font-size: 13px;
  color: var(--muted-foreground);
  line-height: 1.5;
  margin-bottom: 20px;
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.details-list {
  background: var(--background);
}

.detail-item:hover {
  background: var(--accent);
}

.load-more-container {
  padding: 8px;
  text-align: center;
  border-top: 1px solid var(--border);
}

.load-more-btn {
  padding: 4px 12px;
  font-size: 11px;
  color: var(--primary);
  background: transparent;
  border: 1px solid var(--border);
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
}

.load-more-btn:hover {
  background: var(--accent);
  border-color: var(--primary);
}
</style>
