<template>
  <transition name="sheet-fade">
    <div v-if="visible" class="sheet-root" @click.self="emit('close')">
      <section class="sheet">
        <div class="sheet-header">
          <div>
            <strong>关注列表</strong>
            <p>点击进入直播间，长按调整关注顺序。</p>
          </div>
          <button type="button" class="close-btn" @click="emit('close')">关闭</button>
        </div>
        <div class="sheet-body">
          <FollowList
            :followed-anchors="followedAnchors"
            @select-anchor="emit('select-anchor', $event)"
            @unfollow="emit('unfollow', $event)"
            @reorder-list="emit('reorder-list', $event)"
          />
        </div>
      </section>
    </div>
  </transition>
</template>

<script setup lang="ts">
import FollowList from '../components/FollowsList/index.vue';
import type { FollowedStreamer, Platform } from '../platforms/common/types';

defineProps<{
  visible: boolean;
  followedAnchors: FollowedStreamer[];
}>();

const emit = defineEmits<{
  (event: 'close'): void;
  (event: 'select-anchor', streamer: FollowedStreamer): void;
  (event: 'unfollow', payload: { platform: Platform; id: string } | string): void;
  (event: 'reorder-list', newList: FollowedStreamer[]): void;
}>();
</script>

<style scoped>
.sheet-root {
  position: fixed;
  inset: 0;
  z-index: 70;
  background: var(--mobile-sheet-backdrop);
  display: flex;
  align-items: flex-end;
}

.sheet {
  width: 100%;
  max-height: min(84vh, 920px);
  border-radius: 24px 24px 0 0;
  background: var(--mobile-surface);
  color: var(--mobile-text-primary);
  overflow: hidden;
  border: 1px solid var(--mobile-border);
  border-bottom: none;
  box-shadow: var(--mobile-sheet-shadow);
}

.sheet-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 18px 16px 12px;
  border-bottom: 1px solid var(--mobile-border);
}

.sheet-header strong {
  display: block;
  font-size: 18px;
  line-height: 1.2;
}

.sheet-header p {
  margin: 4px 0 0;
  font-size: 13px;
  color: var(--mobile-text-secondary);
}

.close-btn {
  min-width: 56px;
  min-height: 40px;
  border: 1px solid var(--mobile-border);
  border-radius: 12px;
  background: var(--mobile-icon-btn-bg);
  color: var(--mobile-text-primary);
  font-weight: 700;
}

.sheet-body {
  overflow: auto;
  padding: 10px 10px 14px;
  max-height: calc(min(84vh, 920px) - 80px);
}

.sheet-fade-enter-active,
.sheet-fade-leave-active {
  transition: opacity 180ms ease;
}

.sheet-fade-enter-active .sheet,
.sheet-fade-leave-active .sheet {
  transition: transform 180ms ease;
}

.sheet-fade-enter-from,
.sheet-fade-leave-to {
  opacity: 0;
}

.sheet-fade-enter-from .sheet,
.sheet-fade-leave-to .sheet {
  transform: translateY(18px);
}

.close-btn:focus-visible,
button:focus-visible,
input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 3px;
}

button {
  -webkit-tap-highlight-color: transparent;
  transition: background-color 160ms ease, border-color 160ms ease;
}

button:active:not(:disabled) {
  background-color: var(--mobile-pill-active-bg);
}

button:disabled {
  opacity: 0.5;
  cursor: default;
}

.sheet-header strong {
  line-height: 1.4;
}

.sheet-header p {
  line-height: 1.6;
}
</style>
