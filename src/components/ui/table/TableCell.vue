<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { cn } from '@/lib/utils'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()

const cellClass = computed(() =>
  cn('p-2 align-middle [&:has([role=checkbox])]:pr-0', props.class)
)
</script>

<template>
  <td :class="cellClass">
    <slot />
  </td>
</template>
