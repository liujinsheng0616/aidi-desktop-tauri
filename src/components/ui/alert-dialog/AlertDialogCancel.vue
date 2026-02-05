<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { AlertDialogCancel } from 'radix-vue'
import { cn } from '@/lib/utils'
import { buttonVariants } from '@/components/ui/button'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()

const cancelClass = computed(() => cn(buttonVariants({ variant: 'outline' }), 'mt-2 sm:mt-0', props.class))
</script>

<template>
  <AlertDialogCancel :class="cancelClass">
    <slot />
  </AlertDialogCancel>
</template>
