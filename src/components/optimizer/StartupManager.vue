<script setup lang="ts">
import { ref, computed } from 'vue'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import type { StartupDetails, StartupItem } from '@/stores/optimizer'
import { useOptimizer } from '@/stores/optimizer'

interface Props {
  details: StartupDetails
}

const props = defineProps<Props>()

const { toggleStartup, refreshStartup, toggleStartupItemSelection, isStartupItemSelected, optimizeSelections } = useOptimizer()

const isOptimizing = ref(false)
const showConfirm = ref(false)

const isMac = navigator.platform.toLowerCase().includes('mac')

// Sources that require admin privileges (cannot be modified by user)
// macOS: GlobalLaunchAgents needs admin
// Windows: HKLM_Run, AllUsersStartup need admin
const adminOnlySources = isMac
  ? ['GlobalLaunchAgents']
  : ['HKLM_Run', 'AllUsersStartup']

// Filter items that can be modified (exclude admin-only sources)
const editableItems = computed(() =>
  props.details.items.filter(item => !adminOnlySources.includes(item.source))
)

// Only show enabled items that can be disabled for optimization
const optimizableItems = computed(() =>
  editableItems.value.filter(item => item.enabled)
)

// Check if an item is selected for optimization
function isSelected(item: StartupItem): boolean {
  return isStartupItemSelected(item)
}

// Toggle selection for optimization
function handleSelectForOptimize(item: StartupItem, checked: boolean) {
  toggleStartupItemSelection(item, checked)
}

// Selected count
const selectedCount = computed(() => optimizeSelections.startup.items.length)

const hasSelection = computed(() => selectedCount.value > 0)

// Optimize selected items (disable them)
async function handleOptimize() {
  if (!hasSelection.value) return

  isOptimizing.value = true
  try {
    const itemsToDisable = [...optimizeSelections.startup.items]
    for (const item of itemsToDisable) {
      await toggleStartup(item, false)
    }
    // Clear selections
    optimizeSelections.startup.items = []
    // Refresh data
    await refreshStartup()
  } catch (e) {
    console.error('Optimize failed:', e)
  } finally {
    isOptimizing.value = false
  }
}

function getSourceLabel(source: string) {
  // macOS sources
  if (isMac) {
    switch (source) {
      case 'LoginItems': return '登录项'
      case 'LaunchAgents': return '启动代理'
      case 'GlobalLaunchAgents': return '全局代理'
      default: return source
    }
  }
  // Windows sources
  switch (source) {
    case 'HKCU_Run': return '用户注册表'
    case 'HKLM_Run': return '系统注册表'
    case 'StartupFolder': return '启动文件夹'
    case 'AllUsersStartup': return '公共启动'
    default: return source
  }
}
</script>

<template>
  <div class="space-y-3">
    <!-- Optimizable items list -->
    <div class="space-y-1">
      <div
        v-for="item in optimizableItems"
        :key="item.name + item.source"
        class="startup-item flex items-center justify-between text-xs p-1.5 rounded transition-colors cursor-default"
      >
        <span class="flex items-center gap-2 flex-1 min-w-0">
          <span>🚀</span>
          <span class="truncate" :title="item.name">{{ item.name }}</span>
        </span>
        <div class="flex items-center gap-2">
          <span class="text-muted-foreground">{{ getSourceLabel(item.source) }}</span>
          <Switch
            :model-value="isSelected(item)"
            :disabled="isOptimizing"
            @update:model-value="handleSelectForOptimize(item, $event as boolean)"
            class="scale-75"
          />
        </div>
      </div>

      <div v-if="optimizableItems.length === 0" class="text-xs text-muted-foreground p-2 text-center">
        没有可优化的启动项
      </div>
    </div>

    <div class="flex items-center justify-between pt-2 border-t">
      <span class="text-sm font-medium">已选: {{ selectedCount }} 项</span>
      <Button size="sm" :disabled="isOptimizing || !hasSelection" @click="showConfirm = true">
        {{ isOptimizing ? '优化中...' : '优化所选' }}
      </Button>
    </div>

    <!-- Confirm dialog -->
    <Teleport to="body">
      <div v-if="showConfirm" class="confirm-overlay" @click.self="showConfirm = false">
        <div class="confirm-dialog">
          <div class="confirm-title">确认优化</div>
          <div class="confirm-desc">
            将禁用 <strong>{{ selectedCount }}</strong> 个启动项。<br>
            禁用后下次开机将不再自动启动这些程序。
          </div>
          <div class="confirm-actions">
            <Button variant="outline" size="sm" @click="showConfirm = false">取消</Button>
            <Button size="sm" @click="handleOptimize(); showConfirm = false">确认优化</Button>
          </div>
        </div>
      </div>
    </Teleport>

    <div class="text-xs text-muted-foreground">
      注：仅显示可修改的启动项，系统级启动项需管理员权限
    </div>
  </div>
</template>

<style scoped>
.startup-item:hover {
  background: var(--accent);
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
</style>
