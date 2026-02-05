<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { AlertDialogTitle } from 'radix-vue'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()

const titleClass = computed(() => cn('text-lg font-semibold', props.class))
</script>

<template>
  <AlertDialogTitle :class="titleClass">
    <slot />
  </AlertDialogTitle>
</template>
