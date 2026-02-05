<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import { cn } from '@/lib/utils'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()

const bodyClass = computed(() => cn('[&_tr:last-child]:border-0', props.class))
</script>

<template>
  <tbody :class="bodyClass">
    <slot />
  </tbody>
</template>
