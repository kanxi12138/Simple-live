<template>
  <div class="cate2-strip">
    <div class="cate2-list" ref="cate2ListRef">
      <button
        v-for="cate2 in cate2List"
        :key="cate2.href"
        type="button"
        class="cate2-chip"
        :class="{ active: selectedCate2Href === cate2.href }"
        @click="$emit('select', cate2)"
      >
        <span class="cate2-name" :title="cate2.title">{{ cate2.title }}</span>
        <span
          class="cate2-subscribe-btn"
          :class="{ 'is-active': isSubscribed(cate2) }"
          :aria-label="isSubscribed(cate2) ? `Unsubscribe ${cate2.title}` : `Subscribe ${cate2.title}`"
          @click.stop="$emit('subscribe', cate2)"
        >
          +
        </span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue'
import type { Category2 } from '../../../platforms/common/categoryTypes.ts'

const props = defineProps<{
  cate2List: Category2[]
  selectedCate2Href: string | null
  isExpanded?: boolean
  isSubscribed?: (cate2: Category2) => boolean
}>()

const emit = defineEmits<{
  (e: 'select', cate2: Category2): void
  (e: 'subscribe', cate2: Category2): void
  (e: 'toggle-expand'): void
  (e: 'height-changed'): void
}>()

const cate2ListRef = ref<HTMLElement | null>(null)

onMounted(() => {
  nextTick(() => emit('height-changed'))
})

const isSubscribed = (cate2: Category2) => props.isSubscribed?.(cate2) ?? false
</script>

<style scoped>
.cate2-strip {
  padding: 0 4px 1px;
}

.cate2-list {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 0 4px 1px;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.cate2-list::-webkit-scrollbar {
  width: 0;
  height: 0;
}

.cate2-chip {
  flex: 0 0 auto;
  min-height: 28px;
  padding: 0 9px 0 11px;
  border: 1px solid transparent;
  border-radius: 999px;
  background: rgba(74, 80, 88, 0.82);
  color: #d7dde6;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.06),
    inset 0 -1px 0 rgba(0, 0, 0, 0.2);
}

.cate2-chip.active {
  background: rgba(142, 152, 160, 0.3);
  color: #f8fbff;
  border-color: rgba(235, 241, 246, 0.2);
}

.cate2-name {
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
}

.cate2-subscribe-btn {
  width: 15px;
  height: 15px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: rgba(34, 197, 94, 0.22);
  color: #4ade80;
  font-size: 11px;
  line-height: 1;
  font-weight: 800;
}

.cate2-subscribe-btn.is-active {
  background: rgba(34, 197, 94, 0.94);
  color: #062814;
}

:root[data-theme="light"] .cate2-chip {
  background: rgba(255, 255, 255, 0.92);
  color: #334155;
  border-color: rgba(203, 213, 225, 0.78);
  box-shadow: 0 4px 10px rgba(15, 23, 42, 0.08);
}

:root[data-theme="light"] .cate2-chip.active {
  background: rgba(226, 232, 240, 0.96);
  color: #0f172a;
}

:root[data-theme="light"] .cate2-subscribe-btn {
  background: rgba(34, 197, 94, 0.12);
  color: #15803d;
}

:root[data-theme="light"] .cate2-subscribe-btn.is-active {
  background: #22c55e;
  color: #ffffff;
}
</style>
