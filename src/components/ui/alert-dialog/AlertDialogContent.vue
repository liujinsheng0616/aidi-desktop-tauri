<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { AlertDialogContent, AlertDialogOverlay } from 'radix-vue'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()

const contentClass = computed(() =>
  cn(
    'fixed left-1/2 top-1/2 z-50 grid w-full max-w-lg -translate-x-1/2 -translate-y-1/2 gap-4 border bg-background p-6 shadow-lg duration-200 sm:rounded-lg',
    props.class
  )
)
</script>

<template>
  <Teleport to="body">
    <AlertDialogOverlay class="fixed inset-0 z-50 bg-black/80" />
    <AlertDialogContent :class="contentClass">
      <slot />
    </AlertDialogContent>
  </Teleport>
</template>
