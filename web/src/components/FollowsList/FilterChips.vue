<template>
  <div class="filter-group">
    <button 
      class="filter-chip" 
      :class="{ active: activeFilter === 'ALL' }" 
      @click="$emit('update:activeFilter', 'ALL')"
    >全部</button>
    <button 
      v-for="p in visiblePlatforms" 
      :key="p" 
      class="filter-chip" 
      :class="{ active: activeFilter === p }" 
      @click="$emit('update:activeFilter', p)"
    >{{ platformLabel(p) }}</button>
  </div>
</template>

<script setup lang="ts">
import { Platform } from '../../platforms/common/types';

defineProps<{
  visiblePlatforms: Platform[],
  activeFilter: 'ALL' | Platform,
}>();

const platformLabel = (p: Platform): string => {
  switch (p) {
    case Platform.DOUYU: return '斗鱼';
    case Platform.DOUYIN: return '抖音';
    case Platform.HUYA: return '虎牙';
    case Platform.BILIBILI: return 'B站';
    default: return '未知';
  }
};
</script>

<style scoped>
.filter-group { 
  display: flex; 
  align-items: center; 
  gap: 10px; 
  margin: 0;
  padding: 0 6px;
}
.filter-chip {
  padding: 6px 14px;
  border-radius: 12px;
  border: 1px solid var(--glass-border);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  color: var(--secondary-text);
  cursor: pointer;
  transition: all 0.3s cubic-bezier(0.23, 1, 0.32, 1);
  font-weight: 700;
  font-size: 12px;
  letter-spacing: 0.02em;
}
.filter-chip:hover {
  background: var(--hover-bg);
  color: var(--accent-color);
  transform: translateY(-1px);
  border-color: var(--accent-color);
}
.filter-chip.active {
  background: #f1f5f9;
  border-color: #cbd5e1;
  color: #111827;
  box-shadow: 0 6px 14px rgba(15, 23, 42, 0.08);
  transform: translateY(-1px);
}

:root[data-theme="dark"] .filter-chip.active {
  background: #2a2f33;
  border-color: #3b4248;
  color: #f1f5f9;
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.45);
}
</style>
