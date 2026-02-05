<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { AlertDialogAction } from 'radix-vue'
import { cn } from '@/lib/utils'
import { buttonVariants } from '@/components/ui/button'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()

const actionClass = computed(() => cn(buttonVariants(), props.class))
</script>

<template>
  <AlertDialogAction :class="actionClass">
    <slot />
  </AlertDialogAction>
</template>
