<template>
  <transition name="sheet-fade">
    <div v-if="visible" class="sheet-root" @click.self="handleClose">
      <section class="sheet search-sheet">
        <div class="sheet-header">
          <div>
            <strong>搜索直播间</strong>
            <p>即时反馈，优先保持输入和跳转的丝滑。</p>
          </div>
          <button type="button" class="close-btn" @click="handleClose">关闭</button>
        </div>

        <div class="platform-strip">
          <button
            v-for="platform in platforms"
            :key="platform.id"
            type="button"
            class="platform-pill"
            :class="{ active: localPlatform === platform.id }"
            @click="setPlatform(platform.id)"
          >
            {{ platform.label }}
          </button>
        </div>

        <div class="search-box">
          <input
            ref="inputRef"
            v-model="query"
            type="search"
            :placeholder="placeholderText"
            enterkeyhint="search"
            @input="handleInput"
            @keydown.enter.prevent="runSearchNow"
          />
          <button type="button" class="submit-btn" @click="runSearchNow">
            <Search :size="17" />
          </button>
        </div>

        <div class="results">
          <div v-if="isLoading" class="state">搜索中...</div>
          <div v-else-if="errorMessage" class="state state--error">{{ errorMessage }}</div>
          <div v-else-if="!results.length && trimmedQuery" class="state">没有找到结果</div>
          <button
            v-for="result in results"
            :key="`${result.platform}:${result.roomId}`"
            type="button"
            class="result-item"
            @click="selectAnchor(result)"
          >
            <img v-if="result.avatar" :src="result.avatar" :alt="result.userName" class="avatar" />
            <div v-else class="avatar avatar--fallback">{{ result.userName.slice(0, 1) }}</div>
            <div class="meta">
              <strong>{{ result.userName }}</strong>
              <span>{{ result.roomTitle || `房间 ${result.roomId}` }}</span>
            </div>
            <span class="status-dot" :class="{ live: result.liveStatus }"></span>
          </button>
        </div>
      </section>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { Search } from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/core';
import type { Platform as UiPlatform } from '../layout/types';
import { Platform } from '../platforms/common/types';
import { useImageProxy } from '../components/FollowsList/useProxy';

type SearchResultItem = {
  platform: Platform;
  roomId: string;
  webId?: string | null;
  userName: string;
  roomTitle?: string | null;
  avatar: string | null;
  liveStatus: boolean;
  rawStatus?: number | null;
};

interface DouyinApiStreamInfo {
  title?: string | null;
  anchor_name?: string | null;
  avatar?: string | null;
  status?: number | null;
  error_message?: string | null;
  web_rid?: string | null;
}

interface DouyinSearchLiveItem {
  web_rid: string;
  room_id: string;
  title: string;
  nickname: string;
  avatar: string;
  is_live: boolean;
  status: number;
}

interface HuyaAnchorItem {
  room_id: string;
  avatar: string;
  user_name: string;
  live_status: boolean;
  title: string;
}

type BilibiliSearchItem = {
  room_id: string;
  title: string;
  avatar: string;
  anchor: string;
  is_live: boolean;
};

const props = defineProps<{
  visible: boolean;
  activePlatform: UiPlatform;
}>();

const emit = defineEmits<{
  (event: 'close'): void;
  (event: 'select-anchor', payload: { id: string; platform: Platform; nickname: string; avatarUrl: string | null }): void;
}>();

const platforms: Array<{ id: UiPlatform; label: string }> = [
  { id: 'douyu', label: '斗鱼' },
  { id: 'huya', label: '虎牙' },
  { id: 'douyin', label: '抖音' },
  { id: 'bilibili', label: 'B站' },
];

const inputRef = ref<HTMLInputElement | null>(null);
const sanitizeSearchPlatform = (platform: UiPlatform): UiPlatform => {
  if (platform === 'custom' || platform === 'custom-m3u8') {
    return 'douyu';
  }
  return platform;
};

const localPlatform = ref<UiPlatform>(sanitizeSearchPlatform(props.activePlatform));
const query = ref('');
const results = ref<SearchResultItem[]>([]);
const isLoading = ref(false);
const errorMessage = ref('');
const requestToken = ref(0);
let debounceTimer: number | null = null;

const { ensureProxyStarted, proxify } = useImageProxy();
const trimmedQuery = computed(() => query.value.trim());
const isLikelyDouyinDirectQuery = (value: string) => /^\d+$/.test(value) || /douyin\.com\//i.test(value);
const placeholderText = computed(() => (
  localPlatform.value === 'douyin'
    ? '抖音仅支持搜索房间号！'
    : '搜索主播、房间号'
));

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    localPlatform.value = sanitizeSearchPlatform(props.activePlatform);
    await nextTick();
    inputRef.value?.focus();
  },
);

watch(
  () => props.activePlatform,
  (platform) => {
    if (props.visible) {
      localPlatform.value = sanitizeSearchPlatform(platform);
    }
  },
);

const resetState = () => {
  results.value = [];
  errorMessage.value = '';
  isLoading.value = false;
  if (debounceTimer !== null) {
    window.clearTimeout(debounceTimer);
    debounceTimer = null;
  }
};

const handleClose = () => {
  query.value = '';
  resetState();
  emit('close');
};

const setPlatform = (platform: UiPlatform) => {
  localPlatform.value = platform;
  if (trimmedQuery.value) {
    void runSearchNow();
  }
};

const handleInput = () => {
  if (debounceTimer !== null) {
    window.clearTimeout(debounceTimer);
  }
  if (!trimmedQuery.value) {
    resetState();
    return;
  }
  isLoading.value = true;
  debounceTimer = window.setTimeout(() => {
    void runSearchNow();
  }, 260);
};

const runSearchNow = async () => {
  if (!trimmedQuery.value) {
    resetState();
    return;
  }

  const currentToken = requestToken.value + 1;
  requestToken.value = currentToken;
  errorMessage.value = '';
  isLoading.value = true;

  try {
    switch (localPlatform.value) {
      case 'douyin':
        await runDouyinSearch(trimmedQuery.value, currentToken);
        break;
      case 'huya':
        await runHuyaSearch(trimmedQuery.value, currentToken);
        break;
      case 'bilibili':
        await runBilibiliSearch(trimmedQuery.value, currentToken);
        break;
      default:
        await runDouyuSearch(trimmedQuery.value, currentToken);
        break;
    }
  } catch (error) {
    if (requestToken.value === currentToken) {
      errorMessage.value = '搜索失败，请重试。';
      isLoading.value = false;
    }
    console.error('[MobileSearchSheet] search failed:', error);
  }
};

const commitResults = (currentToken: number, nextResults: SearchResultItem[]) => {
  if (requestToken.value !== currentToken) return;
  results.value = nextResults;
  isLoading.value = false;
  errorMessage.value = '';
};

const runDouyuSearch = async (keyword: string, currentToken: number) => {
  const response = await invoke<string>('search_anchor', { keyword });
  const payload = JSON.parse(response);
  const items = Array.isArray(payload?.data?.relateUser) ? payload.data.relateUser : [];
  commitResults(
    currentToken,
    items
      .filter((item: any) => item.type === 1)
      .map((item: any) => ({
        platform: Platform.DOUYU,
        roomId: String(item.anchorInfo.rid),
        userName: item.anchorInfo.nickName || '斗鱼主播',
        roomTitle: item.anchorInfo.roomName || item.anchorInfo.description || null,
        avatar: item.anchorInfo.avatar || null,
        liveStatus: item.anchorInfo.isLive === 1 && item.anchorInfo.videoLoop !== 1,
      })),
  );
};

const runDouyinSearch = async (keyword: string, currentToken: number) => {
  if (isLikelyDouyinDirectQuery(keyword)) {
    const info = await invoke<DouyinApiStreamInfo>('fetch_douyin_streamer_info', {
      payload: { args: { room_id_str: keyword } },
    });
    const nextResults: SearchResultItem[] = info?.anchor_name
      ? [{
          platform: Platform.DOUYIN,
          roomId: info.web_rid || keyword,
          webId: info.web_rid || keyword,
          userName: info.anchor_name || '抖音主播',
          roomTitle: info.title || null,
          avatar: info.avatar || null,
          liveStatus: info.status === 2,
          rawStatus: info.status ?? null,
        }]
      : [];
    if (!nextResults.length && info?.error_message) {
      throw new Error(info.error_message);
    }
    commitResults(currentToken, nextResults);
    return;
  }

  const items = await invoke<DouyinSearchLiveItem[]>('search_douyin_live_rooms', {
    keyword,
    page: 1,
  });
  const nextResults: SearchResultItem[] = (Array.isArray(items) ? items : []).map((item) => ({
    platform: Platform.DOUYIN,
    roomId: item.web_rid || item.room_id,
    webId: item.web_rid || item.room_id,
    userName: item.nickname || '抖音主播',
    roomTitle: item.title || null,
    avatar: item.avatar || null,
    liveStatus: item.is_live,
    rawStatus: item.status ?? null,
  }));
  commitResults(currentToken, nextResults);
};

const runHuyaSearch = async (keyword: string, currentToken: number) => {
  const items = await invoke<HuyaAnchorItem[]>('search_huya_anchors', { keyword, page: 1 });
  await ensureProxyStarted();
  commitResults(
    currentToken,
    (Array.isArray(items) ? items : []).map((item) => ({
      platform: Platform.HUYA,
      roomId: item.room_id,
      userName: item.user_name || '虎牙主播',
      roomTitle: item.title || null,
      avatar: proxify(item.avatar || null),
      liveStatus: Boolean(item.live_status),
    })),
  );
};

const runBilibiliSearch = async (keyword: string, currentToken: number) => {
  const items = await invoke<BilibiliSearchItem[]>('search_bilibili_rooms', { keyword, page: 1 });
  await ensureProxyStarted();
  commitResults(
    currentToken,
    (Array.isArray(items) ? items : []).map((item) => ({
      platform: Platform.BILIBILI,
      roomId: item.room_id,
      webId: item.room_id,
      userName: item.anchor || 'B站主播',
      roomTitle: item.title || null,
      avatar: proxify(item.avatar || null),
      liveStatus: item.is_live,
    })),
  );
};

const selectAnchor = (result: SearchResultItem) => {
  emit('select-anchor', {
    id: result.webId || result.roomId,
    platform: result.platform,
    nickname: result.userName,
    avatarUrl: result.avatar,
  });
  handleClose();
};
</script>

<style scoped>
.sheet-root {
  position: fixed;
  inset: 0;
  z-index: 75;
  background: var(--mobile-sheet-backdrop);
  display: flex;
  align-items: flex-end;
}

.search-sheet {
  width: 100%;
  max-height: min(88vh, 960px);
  border-radius: 24px 24px 0 0;
  padding: 18px 16px calc(16px + env(safe-area-inset-bottom));
  background: var(--mobile-surface-strong);
  color: var(--mobile-text-primary);
  overflow: hidden;
}

.sheet-header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.sheet-header strong {
  font-size: 16px;
}

.sheet-header p {
  margin: 4px 0 0;
  color: var(--mobile-text-secondary);
  font-size: 12px;
}

.close-btn,
.submit-btn {
  border: none;
  border-radius: 12px;
  background: var(--mobile-icon-btn-bg);
  color: var(--mobile-text-primary);
}

.close-btn {
  min-width: 56px;
  min-height: 36px;
  font-weight: 700;
}

.platform-strip {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  padding: 14px 0 10px;
  scrollbar-width: none;
}

.platform-strip::-webkit-scrollbar {
  display: none;
}

.platform-pill {
  flex: 0 0 auto;
  min-height: 34px;
  padding: 0 14px;
  border-radius: 999px;
  border: 1px solid var(--mobile-border);
  background: var(--mobile-pill-bg);
  color: var(--mobile-text-secondary);
  font-weight: 700;
}

.platform-pill.active {
  background: var(--mobile-pill-active-bg);
  border-color: var(--mobile-pill-active-border);
  color: var(--mobile-pill-active-text);
}

.search-box {
  display: grid;
  grid-template-columns: 1fr 48px;
  gap: 10px;
}

.search-box input {
  min-width: 0;
  min-height: 48px;
  padding: 0 16px;
  border-radius: 14px;
  border: 1px solid var(--mobile-border);
  background: var(--mobile-surface-soft);
  color: var(--mobile-text-primary);
  font-size: 15px;
}

.search-box input::placeholder {
  color: var(--mobile-text-secondary);
}

.submit-btn {
  min-height: 48px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.results {
  margin-top: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: calc(min(88vh, 960px) - 176px);
  overflow: auto;
}

.state {
  padding: 24px 0;
  text-align: center;
  color: var(--mobile-text-secondary);
}

.state--error {
  color: #fda4af;
}

.result-item {
  width: 100%;
  border: 1px solid var(--mobile-border);
  border-radius: 16px;
  padding: 12px;
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) 12px;
  gap: 12px;
  align-items: center;
  background: var(--mobile-surface-muted);
  color: var(--mobile-text-primary);
  text-align: left;
}

.avatar {
  width: 44px;
  height: 44px;
  border-radius: 14px;
  object-fit: cover;
  background: var(--mobile-icon-btn-bg);
}

.avatar--fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-weight: 800;
}

.meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.meta strong,
.meta span {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.meta strong {
  font-size: 14px;
}

.meta span {
  color: var(--mobile-text-secondary);
  font-size: 12px;
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.72);
}

.status-dot.live {
  background: #22c55e;
}

.sheet-fade-enter-active,
.sheet-fade-leave-active {
  transition: opacity 180ms ease;
}

.sheet-fade-enter-active .search-sheet,
.sheet-fade-leave-active .search-sheet {
  transition: transform 180ms ease;
}

.sheet-fade-enter-from,
.sheet-fade-leave-to {
  opacity: 0;
}

.sheet-fade-enter-from .search-sheet,
.sheet-fade-leave-to .search-sheet {
  transform: translateY(18px);
}
</style>
