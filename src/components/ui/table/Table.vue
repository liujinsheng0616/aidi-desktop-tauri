<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { cn } from '@/lib/utils'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()

const tableClass = computed(() => cn('w-full caption-bottom text-sm', props.class))
</script>

<template>
  <div class="relative w-full overflow-auto">
    <table :class="tableClass">
      <slot />
    </table>
  </div>
</template>
