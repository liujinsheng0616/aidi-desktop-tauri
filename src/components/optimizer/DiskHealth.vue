<script setup lang="ts">
import { Progress } from '@/components/ui/progress'
import { Badge } from '@/components/ui/badge'
import type { HealthDetails } from '@/stores/optimizer'

defineProps<{ details: HealthDetails }>()

function getStatusVariant(status: string) {
  switch (status) {
    case 'good': return 'success'
    case 'warning': return 'warning'
    case 'danger': return 'danger'
    default: return 'outline'
  }
}

function getStatusText(status: string) {
  switch (status) {
    case 'good': return '良好'
    case 'warning': return '警告'
    case 'danger': return '严重'
    default: return status
  }
}

function getMediaTypeText(mediaType: string) {
  switch (mediaType) {
    case 'SSD': return 'SSD'
    case 'HDD': return '机械硬盘'
    case 'SCM': return 'SCM'
    case 'Unspecified': return '未知类型'
    default: return mediaType || '未知类型'
  }
}

function getHealthStatusText(healthStatus: string) {
  switch (healthStatus) {
    case 'Healthy': return '健康'
    case 'Warning': return '警告'
    case 'Unhealthy': return '不健康'
    case 'Unknown': return '未知'
    default: return healthStatus || '未知'
  }
}
</script>

<template>
  <div class="space-y-3">
    <div class="text-xs font-medium">磁盘分区</div>

    <div class="space-y-3">
      <div
        v-for="vol in details.volumes"
        :key="vol.drive"
        class="border rounded p-2 space-y-2"
      >
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium">{{ vol.drive }} {{ vol.label }}</span>
          <Badge :variant="getStatusVariant(vol.status)" class="text-xs">
            {{ getStatusText(vol.status) }}
          </Badge>
        </div>
        <Progress :model-value="vol.usedPercent" class="h-1.5" />
        <div class="flex justify-between text-xs text-muted-foreground">
          <span>已用 {{ vol.usedGB }} GB</span>
          <span>剩余 {{ vol.freeGB }} GB ({{ vol.sizeGB }} GB)</span>
        </div>
      </div>
    </div>

    <div v-if="details.physicalDisks?.length" class="border-t pt-3">
      <div class="text-xs font-medium mb-2">物理磁盘</div>
      <div class="space-y-2">
        <div
          v-for="disk in details.physicalDisks"
          :key="disk.name"
          class="flex items-center justify-between text-xs"
        >
          <span class="truncate max-w-[200px]" :title="disk.name">{{ disk.name }}</span>
          <span class="text-muted-foreground">
            {{ getMediaTypeText(disk.mediaType) }} | {{ disk.sizeGB }} GB | {{ getHealthStatusText(disk.healthStatus) }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
