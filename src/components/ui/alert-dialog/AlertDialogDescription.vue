<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { AlertDialogDescription } from 'radix-vue'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()

const descClass = computed(() => cn('text-sm text-muted-foreground', props.class))
</script>

<template>
  <AlertDialogDescription :class="descClass">
    <slot />
  </AlertDialogDescription>
</template>
