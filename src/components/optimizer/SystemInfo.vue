<script setup lang="ts">
import type { SystemDetails } from '@/stores/optimizer'

interface Props {
  details: SystemDetails
}

const props = defineProps<Props>()

const sections = [
  {
    title: '操作系统',
    items: [
      { label: '系统', value: () => props.details.os?.name },
      { label: '版本', value: () => props.details.os?.version },
      { label: '架构', value: () => props.details.os?.architecture },
      { label: '安装日期', value: () => props.details.os?.installDate },
      { label: '上次启动', value: () => props.details.os?.lastBoot },
    ],
  },
  {
    title: '处理器',
    items: [
      { label: 'CPU', value: () => props.details.cpu?.name },
      { label: '核心/线程', value: () => `${props.details.cpu?.cores}/${props.details.cpu?.threads}` },
      { label: '最大频率', value: () => props.details.cpu?.maxSpeed },
    ],
  },
  {
    title: '硬件',
    items: [
      { label: '制造商', value: () => props.details.manufacturer },
      { label: '型号', value: () => props.details.model },
      { label: '序列号', value: () => props.details.serialNumber },
      { label: '出厂时间', value: () => props.details.manufactureDate },
      { label: '主机名', value: () => props.details.hostname },
      { label: 'IP 地址', value: () => props.details.ip || '-' },
      { label: '内存', value: () => `${props.details.memory?.totalGB} GB` },
      { label: '存储', value: () => `${props.details.storage?.totalGB} GB` },
    ],
  },
  {
    title: '显卡',
    items: [
      { label: 'GPU', value: () => props.details.gpu?.name },
      { label: '分辨率', value: () => props.details.gpu?.resolution },
      { label: '驱动版本', value: () => props.details.gpu?.driverVersion },
    ],
  },
]
</script>

<template>
  <div class="space-y-3">
    <div
      v-for="section in sections"
      :key="section.title"
      class="space-y-1"
    >
      <div class="text-xs font-medium text-muted-foreground">{{ section.title }}</div>
      <div class="border rounded p-2 space-y-1">
        <div
          v-for="item in section.items"
          :key="item.label"
          class="flex justify-between text-xs"
        >
          <span class="text-muted-foreground">{{ item.label }}</span>
          <span class="text-right truncate max-w-[180px]" :title="item.value()">
            {{ item.value() || '-' }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
