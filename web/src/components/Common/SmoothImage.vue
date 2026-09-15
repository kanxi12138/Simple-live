<template>
  <div class="smooth-image-wrapper" :class="{ 'is-loading': !isLoaded, 'is-error': isError }">
    <div v-if="!isLoaded && !isError" class="skeleton image-placeholder"></div>
    <div v-if="isError" class="image-error-placeholder">
      <slot name="error">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="error-icon">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/>
        </svg>
      </slot>
    </div>
    <img
      :key="src"
      :src="src"
      :alt="alt"
      :loading="loading || 'eager'"
      decoding="async"
      :class="['smooth-img', imgClass, { 'fade-in': isLoaded }]"
      @load="handleLoad"
      @error="handleError"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';

const props = defineProps<{
  src: string;
  alt?: string;
  imgClass?: string;
  loading?: 'eager' | 'lazy';
}>();

const isLoaded = ref(false);
const isError = ref(false);

const handleLoad = (event: Event) => {
  // Ignore events from an image replaced while its request was still pending.
  if ((event.currentTarget as HTMLImageElement).getAttribute('src') !== props.src) return;
  isLoaded.value = true;
  isError.value = false;
};

const handleError = (event: Event) => {
  if ((event.currentTarget as HTMLImageElement).getAttribute('src') !== props.src) return;
  isLoaded.value = false;
  isError.value = true;
};

// Reset state when src changes
watch(() => props.src, () => {
  isLoaded.value = false;
  isError.value = false;
});
</script>

<style scoped>
.smooth-image-wrapper {
  position: relative;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--bg-tertiary);
  /* Do not set fixed width/height here, let the class from parent decide */
}

.image-placeholder {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 1;
  background: var(--bg-tertiary);
}

.image-error-placeholder {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  color: var(--secondary-text);
  background-color: var(--secondary-bg);
}

.smooth-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  opacity: 0;
  transition: opacity 0.35s ease;
}

.smooth-img.fade-in {
  opacity: 1;
}

.error-icon {
  opacity: 0.5;
}

@media (prefers-reduced-motion: reduce) {
  .smooth-img {
    transition: none;
  }
}
</style>
