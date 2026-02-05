<script setup lang="ts">
import { computed } from 'vue'
import { ChevronDown } from 'lucide-vue-next'
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '@/components/ui/collapsible'
import { Badge } from '@/components/ui/badge'
import type { Status } from '@/stores/optimizer'

interface Props {
  icon: string
  title: string
  status: Status
  summary: string
  open?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
})

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const statusVariant = computed(() => {
  switch (props.status) {
    case 'good': return 'success'
    case 'warning': return 'warning'
    case 'danger': return 'danger'
    case 'error': return 'destructive'
    case 'scanning': return 'secondary'
    default: return 'outline'
  }
})

const statusText = computed(() => {
  switch (props.status) {
    case 'good': return '良好'
    case 'warning': return '警告'
    case 'danger': return '严重'
    case 'error': return '错误'
    case 'scanning': return '扫描中'
    default: return '查看'
  }
})
</script>

<template>
  <Collapsible
    :open="open"
    @update:open="emit('update:open', $event)"
    class="border rounded-lg overflow-hidden"
  >
    <CollapsibleTrigger class="w-full">
      <div class="flex items-center gap-3 p-3 hover:bg-muted/50 transition-colors cursor-pointer">
        <span class="text-xl">{{ icon }}</span>
        <span class="flex-1 text-left font-medium text-sm">{{ title }}</span>
        <Badge :variant="statusVariant" class="text-xs">
          {{ statusText }}
        </Badge>
        <ChevronDown
          :size="16"
          class="text-muted-foreground transition-transform"
          :class="{ 'rotate-180': open }"
        />
      </div>
    </CollapsibleTrigger>
    <CollapsibleContent>
      <div class="px-3 pb-3 pt-1 border-t bg-muted/30">
        <p class="text-xs text-muted-foreground mb-2">{{ summary }}</p>
        <slot />
      </div>
    </CollapsibleContent>
  </Collapsible>
</template>
