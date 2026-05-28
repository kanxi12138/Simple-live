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
      v-show="isLoaded"
      :src="src"
      :alt="alt"
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
}>();

const isLoaded = ref(false);
const isError = ref(false);

const handleLoad = () => {
  isLoaded.value = true;
  isError.value = false;
};

const handleError = () => {
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
  background-color: var(--skeleton-bg);
  /* Do not set fixed width/height here, let the class from parent decide */
}

.image-placeholder {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 1;
  animation: shimmer 1.4s ease-in-out infinite;
  background: linear-gradient(
    90deg,
    rgba(255, 255, 255, 0.04) 25%,
    rgba(255, 255, 255, 0.12) 37%,
    rgba(255, 255, 255, 0.04) 63%
  );
  background-size: 400% 100%;
}

.image-error-placeholder {
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

@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}
</style>
