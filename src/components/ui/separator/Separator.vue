<script setup lang="ts">
import { cn } from '@/lib/utils'

interface Props {
  orientation?: 'horizontal' | 'vertical'
  decorative?: boolean
}

withDefaults(defineProps<Props>(), {
  orientation: 'horizontal',
  decorative: true,
})
</script>

<template>
  <div
    :role="decorative ? 'none' : 'separator'"
    :aria-orientation="decorative ? undefined : orientation"
    :class="cn(
      'shrink-0 bg-border',
      orientation === 'horizontal' ? 'h-[1px] w-full' : 'h-full w-[1px]',
      $attrs.class ?? ''
    )"
  />
</template>
