<template>
  <div class="cate1-list-container">
    <ul class="cate1-list">
      <li
        v-for="cate1 in cate1List"
        :key="cate1.href"
        class="cate1-item"
        :class="{ selected: cate1.href === selectedCate1Href }"
        @click="$emit('select', cate1)"
      >
        <span class="cate1-name">{{ cate1.title }}</span>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import type { Category1 } from '../../../platforms/common/categoryTypes.ts'

defineProps<{
  cate1List: Category1[]
  selectedCate1Href: string | null
}>()
</script>

<style scoped>
.cate1-list-container {
  overflow-x: auto;
  flex-shrink: 0;
  padding: 0 0 2px;
  border: none;
  box-shadow: none;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.cate1-list {
  list-style: none;
  margin: 0;
  padding: 0 8px 1px;
  display: inline-flex;
  gap: 12px;
  flex-wrap: nowrap;
  background: transparent;
  border: none;
  box-shadow: none;
  position: relative;
}

.cate1-item {
  height: 28px;
  padding: 0 2px;
  display: inline-flex;
  align-items: center;
  cursor: pointer;
  transition: color 0.2s ease, transform 0.2s ease;
  font-size: 12px;
  font-weight: 500;
  border-radius: 8px;
  white-space: nowrap;
  color: var(--secondary-text);
  background: transparent;
  border: 1px solid transparent;
  box-shadow: none;
  position: relative;
}

.cate1-item:hover {
  color: var(--primary-text);
}

.cate1-item.selected {
  color: var(--text-primary);
  font-weight: 700;
  transform: translateY(-1px);
}

.cate1-item::after {
  content: '';
  position: absolute;
  left: 22%;
  right: 22%;
  bottom: -4px;
  height: 3px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.65);
  opacity: 0;
  transform: scaleX(0.6);
  transition: opacity 0.2s ease, transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.cate1-item.selected::after {
  opacity: 1;
  transform: scaleX(1);
}

:root[data-theme="light"] .cate1-item::after {
  background: rgba(0, 0, 0, 0.45);
}

:root[data-theme="dark"] .cate1-item {
  color: #cbd5d1;
}

:root[data-theme="light"] .cate1-item {
  color: #5f6563;
}

:root[data-theme="light"] .cate1-item.selected {
  color: #1f2937;
}

:root[data-theme="dark"] .cate1-item.selected {
  color: #f6fbf7;
}

.cate1-name {
  display: inline-block;
  letter-spacing: 0.2px;
}

.cate1-list-container::-webkit-scrollbar {
  width: 0;
  height: 0;
}

</style>
