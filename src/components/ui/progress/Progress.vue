<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { ProgressIndicator, ProgressRoot, type ProgressRootProps } from 'radix-vue'
import { cn } from '@/lib/utils'

interface Props extends ProgressRootProps {
  class?: HTMLAttributes['class']
  indicatorClass?: HTMLAttributes['class']
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: 0,
})

const progressClass = computed(() =>
  cn('relative h-2 w-full overflow-hidden rounded-full bg-secondary', props.class)
)

const indicatorClass = computed(() =>
  cn('h-full w-full flex-1 bg-primary transition-all', props.indicatorClass)
)
</script>

<template>
  <ProgressRoot v-bind="props" :class="progressClass">
    <ProgressIndicator
      :class="indicatorClass"
      :style="`transform: translateX(-${100 - (props.modelValue ?? 0)}%)`"
    />
  </ProgressRoot>
</template>
