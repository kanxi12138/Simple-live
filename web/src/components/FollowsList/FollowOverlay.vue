<template>
  <teleport to="body">
    <transition name="overlay-fade">
      <div v-if="show" class="follow-overlay-backdrop" @click.self="emit('close')">
        <transition name="overlay-pop">
          <div 
            ref="panelRef"
            class="follow-overlay-panel" 
            :style="{ top: `${panelTop}px`, left: `${panelLeft}px`, height: `${panelHeight}px` }"
          >
            <!-- 将关闭按钮移动到面板右上角 -->
            <button class="overlay-close-btn" title="关闭" @click="emit('close')">
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
            <div class="overlay-header" ref="headerRef">
              <div class="overlay-header-left">
                <!-- 移除标题：关注列表 -->
                <slot name="filters"></slot>
              </div>
              <div class="overlay-header-actions">
                <button 
                  class="overlay-text-btn manage-action" 
                  :class="{ active: props.isDeleteMode }"
                  @click="emit('toggle-remove')"
                >
                  <span>{{ props.isDeleteMode ? '完成' : '管理' }}</span>
                </button>
                <button 
                  class="overlay-text-btn refresh-action" 
                  :class="{ 'is-refreshing': isRefreshing, 'just-finished': justFinished }" 
                  :disabled="isRefreshing" 
                  @click="emit('refresh')"
                >
                  <span class="refresh-label">刷新</span>
                  <span class="refresh-spinner" aria-hidden="true"></span>
                </button>
                <!-- 原关闭按钮已移除到面板右上角 -->
              </div>
            </div>
            <div class="overlay-content" :style="{ height: `${Math.max(120, panelHeight - headerHeight)}px`, overflow: shouldScroll ? 'auto' : 'hidden' }">
              <div v-if="!items || items.length === 0" class="overlay-empty">
                <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round" class="feather feather-heart">
                  <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"></path>
                </svg>
                <p class="empty-title">暂无关注主播</p>
                <p class="empty-text">当前筛选下暂无关注主播</p>
              </div>
              <ul v-else class="overlay-streamers-list" ref="listEl">
                <li 
                  v-for="s in items" 
                  :key="s.id" 
                  class="overlay-streamer-item"
                  :class="{ 'remove-mode': props.isDeleteMode }"
                  @click="handleSelect(s)"
                >
                  <button 
                    v-if="props.isDeleteMode" 
                    class="overlay-remove-btn" 
                    title="删除"
                    @click.stop="emit('remove', s)"
                  >
                    ×
                  </button>
                  <StreamerItem 
                    :streamer="s" 
                    :getAvatarSrc="getAvatarSrc" 
                    :handleImgError="handleImgError"
                    :getLiveIndicatorClass="getLiveIndicatorClass"
                    :proxyBase="proxyBase"
                    :big="false"
                    :showPlatform="false"
                    @clickItem="() => handleSelect(s)"
                  />
                </li>
              </ul>
            </div>
          </div>
        </transition>
      </div>
    </transition>
  </teleport>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, nextTick } from 'vue';
import type { FollowedStreamer } from '../../platforms/common/types';
import StreamerItem from './StreamerItem.vue';

const props = defineProps<{ 
  show: boolean, 
  items: FollowedStreamer[], 
  getAvatarSrc: (s: FollowedStreamer) => string, 
  handleImgError: (ev: Event, s: FollowedStreamer) => void, 
  getLiveIndicatorClass: (s: FollowedStreamer) => string, 
  proxyBase?: string, 
  alignTop?: number, 
  alignLeft?: number,
  isRefreshing?: boolean,
  isDeleteMode?: boolean
}>();
const emit = defineEmits<{ 
  (e: 'close'): void, 
  (e: 'select', s: FollowedStreamer): void, 
  (e: 'refresh'): void,
  (e: 'toggle-remove'): void,
  (e: 'remove', s: FollowedStreamer): void
}>();

// 刷新完成提示：当 isRefreshing 从 true 变为 false 时，短暂展示完成动画
const justFinished = ref(false);
watch(() => props.isRefreshing, (newVal, oldVal) => {
  if (oldVal && !newVal) {
    justFinished.value = true;
    setTimeout(() => { justFinished.value = false; }, 800);
  }
});

watch(() => props.show, (val) => {
  if (val) {
    nextTick(() => computePanelMetrics());
  }
});

// 参考值与动态测量
const PANEL_MIN = 220;
const PANEL_MAX_MARGIN = 180; // 留出顶部/底部边距，增加底部空间
const DEFAULT_CARD_H = 76; // 估算：48头像 + 24内边距 + 2边框
const DEFAULT_CARD_W = 200;
const LIST_PAD_TOP = 6;
const LIST_PAD_BOTTOM = 6;
const panelHeight = ref<number>(400);
const panelTop = ref<number>(64);
const panelLeft = ref<number>(props.alignLeft ?? 240);
const headerHeight = ref<number>(56);
const headerRef = ref<HTMLElement | null>(null);
const listEl = ref<HTMLElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const shouldScroll = ref<boolean>(false);

function clamp(n: number, min: number, max: number) { return Math.max(min, Math.min(max, n)); }
function updatePanelLeft() {
  if (typeof window === 'undefined') return;
  nextTick(() => {
    const requestedLeft = props.alignLeft ?? 240;
    if (!panelRef.value) {
      panelLeft.value = Math.max(16, Math.min(requestedLeft, window.innerWidth - 320));
      return;
    }
    const panelWidth = panelRef.value.getBoundingClientRect().width;
    const maxLeft = Math.max(16, window.innerWidth - panelWidth - 16);
    panelLeft.value = Math.max(16, Math.min(requestedLeft, maxLeft));
  });
}
function computePanelMetrics() {
  nextTick(() => {
    // 测量 header 实际高度
    headerHeight.value = Math.ceil(headerRef.value?.getBoundingClientRect().height || 56);
    // 测量首个卡片高度
    let cardH = DEFAULT_CARD_H;
    let cardW = DEFAULT_CARD_W;
    const firstItem = listEl.value?.querySelector('.overlay-streamer-item') as HTMLElement | null;
    if (firstItem) {
      const firstRect = firstItem.getBoundingClientRect();
      cardH = Math.ceil(firstRect.height || DEFAULT_CARD_H);
      cardW = Math.ceil(firstRect.width || DEFAULT_CARD_W);
    }
    // 读取 grid gap（如果可用）
    let gapPx = 14;
    let listWidth = 0;
    if (listEl.value) {
      const cs = window.getComputedStyle(listEl.value);
      const gapStr = (cs as any).gap || cs.columnGap || cs.rowGap;
      const parsed = parseFloat(gapStr || '');
      if (!isNaN(parsed)) gapPx = Math.round(parsed);
      listWidth = Math.ceil(listEl.value.getBoundingClientRect().width || 0);
    }
    const count = Array.isArray(props.items) ? props.items.length : 0;
    let columns = count > 0 ? Math.floor((listWidth + gapPx) / (cardW + gapPx)) : 1;
    if (!Number.isFinite(columns) || columns < 1) {
      columns = Math.max(1, Math.min(count || 1, Math.ceil(Math.sqrt(count || 1))));
    } else {
      columns = Math.max(1, Math.min(count || 1, columns));
    }
    const rows = Math.max(1, columns > 0 ? Math.ceil(count / columns) : count);
    const contentHeight = rows * cardH + (rows - 1) * gapPx + LIST_PAD_TOP + LIST_PAD_BOTTOM;
    const desired = headerHeight.value + contentHeight + 8 + 10; // overlay-content padding: 上8 下10
    const maxH = (typeof window !== 'undefined') ? (window.innerHeight - PANEL_MAX_MARGIN) : desired;
    panelHeight.value = clamp(desired, PANEL_MIN, maxH);
    shouldScroll.value = desired > maxH;
    const vh = (typeof window !== 'undefined') ? window.innerHeight : panelHeight.value;
    panelTop.value = Math.max(16, Math.round((vh - panelHeight.value) / 2));
    updatePanelLeft();
  });
}

onMounted(() => {
  computePanelMetrics();
  resizeListener = () => computePanelMetrics();
  scrollListener = () => { if (props.show) computePanelMetrics(); };
  window.addEventListener('resize', resizeListener);
  window.addEventListener('scroll', scrollListener, { passive: true });
});
let resizeListener: ((this: Window, ev: UIEvent) => any) | null = null;
let scrollListener: ((this: Window, ev: Event) => any) | null = null;
onUnmounted(() => { 
  if (resizeListener) window.removeEventListener('resize', resizeListener); 
  if (scrollListener) window.removeEventListener('scroll', scrollListener);
});
watch(() => props.items, () => computePanelMetrics(), { deep: true });
watch(() => props.show, (v) => { if (v) computePanelMetrics(); });
watch(() => props.isDeleteMode, () => computePanelMetrics());
watch(() => props.alignLeft, () => updatePanelLeft());

const handleSelect = (s: FollowedStreamer) => {
  if (props.isDeleteMode) return;
  emit('select', s);
};
</script>

<style scoped>
.follow-overlay-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(8px);
  z-index: 1200;
}

.follow-overlay-panel {
  position: fixed;
  width: min(1180px, calc(100vw - 48px));
  border-radius: var(--radius-lg);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  color: var(--primary-text);
  border: 1px solid var(--glass-border);
  box-shadow: var(--glass-shadow);
  overflow: visible;
  transform: translateZ(0);
  z-index: 1201;
}

.overlay-header {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 16px;
  padding: 20px 32px;
  border-bottom: none;
}

.overlay-header-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}

.overlay-header-actions { 
  display: flex; 
  align-items: center; 
  gap: 16px;
}

.overlay-text-btn {
  background: var(--tertiary-bg);
  border: 1px solid var(--border-color);
  color: var(--primary-text);
  padding: 8px 18px;
  border-radius: 12px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 13px;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.overlay-text-btn:hover {
  background: var(--secondary-bg);
  border-color: var(--accent-color);
  color: var(--accent-color);
  transform: translateY(-1px);
  box-shadow: var(--card-shadow);
}

.overlay-text-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.manage-action.active {
  background: var(--accent-color);
  border-color: transparent;
  color: #fff;
}

.refresh-action .refresh-spinner {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  visibility: hidden;
}

.refresh-action.is-refreshing .refresh-spinner {
  visibility: visible;
  animation: spin 0.8s linear infinite;
}

@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }

.overlay-content {
  overflow: auto;
  padding: 12px 32px 92px; /* 增加底部内边距，避免内容贴底 */
}

.overlay-content::-webkit-scrollbar {
  width: 4px;
}

.overlay-content::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 10px;
}

.overlay-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 80px 40px;
  color: var(--secondary-text);
  gap: 20px;
}

.empty-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--primary-text);
}

.overlay-streamers-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 12px;
  list-style: none;
  margin: 0;
  padding: 0;
}

.overlay-streamer-item {
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color-light);
  background: var(--secondary-bg);
  cursor: pointer;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  overflow: hidden;
}

.overlay-streamer-item:hover {
  transform: translateY(-2px);
  border-color: var(--border-color);
  box-shadow: var(--card-shadow-hover);
}

/* Keep follow overlay using glass-style background across themes to match search results */

.overlay-remove-btn {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: #ef4444;
  color: white;
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10;
  font-weight: bold;
  box-shadow: 0 2px 8px rgba(239, 68, 68, 0.3);
  transition: all 0.2s ease;
  font-size: 16px;
}

.overlay-remove-btn:hover {
  transform: scale(1.1) rotate(90deg);
  background: #dc2626;
}

.overlay-fade-enter-active,
.overlay-fade-leave-active { transition: opacity 0.4s ease; }
.overlay-fade-enter-from,
.overlay-fade-leave-to { opacity: 0; }

.overlay-pop-enter-active,
.overlay-pop-leave-active { transition: all 0.5s cubic-bezier(0.16, 1, 0.3, 1); }
.overlay-pop-enter-from,
.overlay-pop-leave-to { transform: scale(0.9) translateY(20px); opacity: 0; }

.overlay-close-btn {
  position: absolute;
  top: -20px;
  right: -20px;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  border: 1px solid var(--glass-border);
  color: var(--primary-text);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  box-shadow: var(--glass-shadow);
  transition: all 0.4s cubic-bezier(0.34, 1.56, 0.64, 1);
  z-index: 1100;
}

.overlay-close-btn:hover {
  transform: scale(1.1) rotate(180deg);
  border-color: var(--accent-color);
  color: var(--accent-color);
}

:root[data-theme="dark"] .follow-overlay-backdrop {
  background: rgba(0, 0, 0, 0.28);
}

:root[data-theme="dark"] .follow-overlay-panel {
  background: rgba(24, 30, 26, 0.9);
  border-color: rgba(255, 255, 255, 0.14);
  box-shadow: 0 18px 44px rgba(0, 0, 0, 0.48);
}

:root[data-theme="dark"] .overlay-text-btn {
  background: #232c24;
  border-color: rgba(255, 255, 255, 0.14);
}

:root[data-theme="dark"] .overlay-text-btn:hover {
  background: #2a352c;
}

:root[data-theme="dark"] .overlay-streamer-item {
  background: #222b23;
  border-color: rgba(255, 255, 255, 0.12);
}
</style>
