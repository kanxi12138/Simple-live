<template>
  <nav class="mobile-bottom-nav">
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      class="nav-item"
      :class="{ active: activeTab === item.id }"
      @click="emit('select', item.id)"
    >
      <component :is="item.icon" :size="18" />
      <span>{{ item.label }}</span>
    </button>
  </nav>
</template>

<script setup lang="ts">
import { Heart, MonitorPlay, Radar, Search, Settings2 } from 'lucide-vue-next';

export type MobileTab = 'browse' | 'search' | 'follows' | 'player' | 'settings';

defineProps<{
  activeTab: MobileTab;
}>();

const emit = defineEmits<{
  (event: 'select', tab: MobileTab): void;
}>();

const items = [
  { id: 'browse', label: '发现', icon: Radar },
  { id: 'search', label: '搜索', icon: Search },
  { id: 'follows', label: '关注', icon: Heart },
  { id: 'player', label: '播放', icon: MonitorPlay },
  { id: 'settings', label: '设置', icon: Settings2 },
] as const;
</script>

<style scoped>
.mobile-bottom-nav {
  position: sticky;
  bottom: 0;
  z-index: 45;
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 8px;
  padding: 10px 12px calc(10px + env(safe-area-inset-bottom));
  background: var(--mobile-topbar-bg);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
  border-top: 1px solid var(--mobile-topbar-border);
}

.nav-item {
  min-height: 54px;
  border: none;
  border-radius: 14px;
  background: transparent;
  color: var(--mobile-text-secondary);
  display: inline-flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  font-size: 11px;
  font-weight: 700;
  transition: background-color 140ms ease, color 140ms ease, transform 140ms ease;
}

.nav-item.active {
  color: var(--mobile-pill-active-text);
  background: var(--mobile-pill-active-bg);
}

.nav-item:active {
  transform: scale(0.98);
}
</style>
