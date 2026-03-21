<script setup lang="ts">
import { ref, computed } from 'vue'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Switch } from '@/components/ui/switch'
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/table'
import type { MemoryDetails } from '@/stores/optimizer'
import { useOptimizer } from '@/stores/optimizer'

defineProps<{ details: MemoryDetails }>()

const { optimizeMemory, refreshMemory, optimizeSelections, setMemoryOptimizeEnabled } = useOptimizer()

// Computed property for memory optimization selection
const includeInQuickOptimize = computed({
  get: () => optimizeSelections.memory.enabled,
  set: (val: boolean) => setMemoryOptimizeEnabled(val)
})

const isOptimizing = ref(false)
const optimizeResult = ref<{ freedMB: number } | null>(null)

async function handleOptimize() {
  isOptimizing.value = true
  optimizeResult.value = null

  try {
    const result = await optimizeMemory()
    optimizeResult.value = result as any
    await refreshMemory()
  } catch (e) {
    // 优化失败，忽略
  } finally {
    isOptimizing.value = false
  }
}
</script>

<template>
  <div class="space-y-3">
    <div class="space-y-2">
      <div class="flex justify-between text-xs">
        <span>已用: {{ details.usedGB }} GB</span>
        <span>可用: {{ details.freeGB }} GB</span>
      </div>
      <Progress :model-value="details.usedPercent" class="h-2" />
      <div class="text-xs text-muted-foreground text-center">
        {{ details.usedPercent }}% 已使用 (共 {{ details.totalGB }} GB)
      </div>
    </div>

    <div class="border rounded">
      <div class="text-xs font-medium p-2 bg-muted/50 border-b">高内存占用进程</div>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead class="text-xs h-8">进程名</TableHead>
            <TableHead class="text-xs h-8 text-right">内存</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="proc in details.topProcesses?.slice(0, 5)" :key="proc.pid">
            <TableCell class="text-xs py-1">{{ proc.name }}</TableCell>
            <TableCell class="text-xs py-1 text-right">{{ proc.memoryMB }} MB</TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>

    <div class="flex items-center justify-between pt-2 border-t">
      <div class="flex items-center gap-2">
        <Switch
          v-model="includeInQuickOptimize"
          :disabled="isOptimizing"
          class="scale-75"
        />
        <span class="text-xs text-muted-foreground">加入一键优化</span>
      </div>
      <Button size="sm" :disabled="isOptimizing" @click="handleOptimize">
        {{ isOptimizing ? '优化中...' : '释放内存' }}
      </Button>
    </div>

    <div v-if="optimizeResult" class="text-xs text-green-600 bg-green-50 p-2 rounded">
      已释放 {{ optimizeResult.freedMB }} MB 内存
    </div>
  </div>
</template>
