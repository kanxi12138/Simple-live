<template>
  <header class="mobile-topbar" :class="`mobile-topbar--${theme}`">
    <div class="topbar-main">
      <div class="brand">
        <div class="brand-mark">简</div>
        <div class="brand-copy">
          <strong>简直播</strong>
          <span>{{ subtitle }}</span>
        </div>
      </div>

      <div class="actions">
        <button
          type="button"
          class="icon-btn"
          aria-label="搜索"
          @click="emit('open-search')"
        >
          <Search :size="18" />
        </button>
        <button
          type="button"
          class="icon-btn"
          aria-label="关注列表"
          @click="emit('open-follows')"
        >
          <Heart :size="18" />
        </button>
        <button
          type="button"
          class="icon-btn"
          aria-label="切换主题"
          @click="emit('theme-toggle')"
        >
          <Sun v-if="theme === 'dark'" :size="18" />
          <Moon v-else :size="18" />
        </button>
      </div>
    </div>

    <div class="platform-strip">
      <button
        v-for="platform in platforms"
        :key="platform.id"
        type="button"
        class="platform-pill"
        :class="{ active: activePlatform === platform.id }"
        @click="emit('platform-change', platform.id)"
      >
        {{ platform.label }}
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { Heart, Moon, Search, Sun } from 'lucide-vue-next';
import type { Platform as UiPlatform } from '../layout/types';

defineProps<{
  activePlatform: UiPlatform;
  theme: 'light' | 'dark';
  subtitle: string;
}>();

const emit = defineEmits<{
  (event: 'platform-change', platform: UiPlatform): void;
  (event: 'theme-toggle'): void;
  (event: 'open-search'): void;
  (event: 'open-follows'): void;
}>();

const platforms: Array<{ id: UiPlatform; label: string }> = [
  { id: 'custom', label: '订阅' },
  { id: 'douyu', label: '斗鱼' },
  { id: 'huya', label: '虎牙' },
  { id: 'douyin', label: '抖音' },
  { id: 'bilibili', label: 'B站' },
  { id: 'custom-m3u8', label: '自定义 M3U8' },
];
</script>

<style scoped>
.mobile-topbar {
  position: sticky;
  top: 0;
  z-index: 35;
  padding: max(14px, env(safe-area-inset-top)) 14px 12px;
  background: var(--mobile-topbar-bg);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
  border-bottom: 1px solid var(--mobile-topbar-border);
  box-shadow: 0 8px 24px rgba(69, 48, 30, 0.06);
}

.topbar-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.brand-mark {
  width: 40px;
  height: 40px;
  border-radius: 13px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-weight: 900;
  color: #fff7ef;
  background: linear-gradient(135deg, #e5774d, #bd4d29);
  box-shadow: 0 10px 22px rgba(216, 100, 58, 0.22);
}

.brand-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.brand-copy strong {
  color: var(--mobile-topbar-text);
  font-size: 15px;
  line-height: 1.2;
  letter-spacing: 0;
}

.brand-copy span {
  color: var(--mobile-text-secondary);
  font-size: 11px;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.actions {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.icon-btn {
  width: 44px;
  height: 44px;
  border: 1px solid var(--mobile-border);
  border-radius: 12px;
  background: var(--mobile-icon-btn-bg);
  color: var(--mobile-topbar-text);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.platform-strip {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  padding-top: 14px;
  scrollbar-width: none;
}

.platform-strip::-webkit-scrollbar {
  display: none;
}

.platform-pill {
  flex: 0 0 auto;
  min-width: 66px;
  min-height: 36px;
  padding: 0 15px;
  border: 1px solid var(--mobile-border);
  border-radius: 999px;
  background: var(--mobile-pill-bg);
  color: var(--mobile-text-secondary);
  font-size: 12px;
  font-weight: 700;
}

.platform-pill.active {
  background: var(--mobile-pill-active-bg);
  color: var(--mobile-pill-active-text);
  border-color: var(--mobile-pill-active-border);
  box-shadow: inset 0 0 0 1px rgba(216, 100, 58, 0.08);
}

.mobile-topbar--light .brand-mark {
  color: #fff7ef;
  background: linear-gradient(135deg, #e5774d, #bd4d29);
  box-shadow: 0 10px 22px rgba(216, 100, 58, 0.22);
}

.mobile-topbar--dark .brand-mark {
  background: linear-gradient(135deg, #ed8158, #c95f39);
  box-shadow: 0 10px 22px rgba(237, 129, 88, 0.24);
}
</style>
