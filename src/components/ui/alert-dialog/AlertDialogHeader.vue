<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { cn } from '@/lib/utils'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()

const headerClass = computed(() =>
  cn('flex flex-col gap-2 text-center sm:text-left', props.class)
)
</script>

<template>
  <div :class="headerClass">
    <slot />
  </div>
</template>
