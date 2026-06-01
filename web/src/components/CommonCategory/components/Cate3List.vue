<template>
  <div v-if="!isLoading && (cate3List.length > 0 || hasAllOption)" class="cate3-list">
    <div
      class="cate3-item"
      :class="{ active: selectedCate3Id === null || selectedCate3Id === 'all' }"
      @click="selectAll"
    >
      全部
    </div>
    <div
      v-for="cate3 in cate3List"
      :key="cate3.id"
      class="cate3-item"
      :class="{ active: selectedCate3Id === cate3.id }"
      @click="$emit('select', cate3)"
    >
      <span class="cate3-name">{{ cate3.name }}</span>
      <button
        type="button"
        class="cate3-subscribe-btn"
        :class="{ 'is-active': isSubscribed(cate3) }"
        :aria-label="isSubscribed(cate3) ? `取消订阅 ${cate3.name}` : `订阅 ${cate3.name}`"
        @click.stop="$emit('subscribe', cate3)"
      >
        +
      </button>
    </div>
  </div>
  <div v-if="isLoading" class="loading-cate3">正在加载三级分类...</div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

type Category3 = {
  id: string;
  name: string;
};

const props = defineProps<{
  cate3List: Category3[];
  selectedCate3Id: string | null;
  isLoading: boolean;
  isSubscribed?: (cate3: Category3) => boolean;
}>();

const emit = defineEmits<{
  (e: 'select', cate3: Category3): void;
  (e: 'subscribe', cate3: Category3): void;
}>();

const hasAllOption = computed(() => {
  return props.cate3List && props.cate3List.length > 0;
});

const selectAll = () => {
  const allCategory: Category3 = {
    id: 'all',
    name: '全部',
  };
  emit('select', allCategory);
};

const isSubscribed = (cate3: Category3) => props.isSubscribed?.(cate3) ?? false;
</script>

<style scoped>
.cate3-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 6px 8px 0 8px;
  padding-bottom: 8px;
}

.cate3-item {
  height: 28px;
  padding: 0 8px 0 12px;
  border-radius: 999px;
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 11.5px;
  font-weight: 700;
  background: var(--bg-tertiary);
  border: none;
  color: var(--text-secondary);
  box-shadow: var(--shadow-low);
}

.cate3-name {
  white-space: nowrap;
}

.cate3-subscribe-btn {
  width: 18px;
  min-width: 18px;
  height: 18px;
  border: none;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  background: var(--mobile-pill-active-bg);
  color: var(--accent);
  font-size: 13px;
  line-height: 1;
  font-weight: 700;
  cursor: pointer;
  transition: transform 0.18s ease, background 0.18s ease, color 0.18s ease;
}

.cate3-subscribe-btn:hover {
  transform: scale(1.05);
  background: var(--mobile-pill-active-bg);
}

.cate3-subscribe-btn.is-active {
  background: var(--mobile-pill-active-bg);
  color: var(--accent);
}

.loading-cate3 {
  margin: 10px 12px 0;
  font-size: 12px;
  color: var(--cate3-loading-text-dark, rgba(255, 255, 255, 0.5));
}

:root[data-theme="light"] .loading-cate3 {
  color: rgba(15, 23, 42, 0.52);
}
</style>
