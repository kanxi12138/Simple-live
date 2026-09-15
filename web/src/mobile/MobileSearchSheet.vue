<template>
  <transition name="sheet-fade">
    <div v-if="visible" class="sheet-root" @click.self="handleClose">
      <section class="sheet search-sheet">
        <div class="sheet-header">
          <div>
            <strong>搜索直播间</strong>
            <p>{{ localPlatform === 'douyin' ? '输入直播关键字或抖音号' : '选择平台，输入关键词或数字房间号。' }}</p>
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
          <button type="button" class="submit-btn" aria-label="搜索直播间" :disabled="!trimmedQuery" @click="runSearchNow">
            <Search :size="17" />
          </button>
        </div>

        <div v-if="localPlatform === 'douyin' && douyinLoginBridge" class="login-row">
          <span>关键词搜索需要抖音登录，房间号可直接查询。</span>
          <button type="button" class="close-btn" @click="openDouyinLogin">抖音登录</button>
        </div>

        <div class="results" :aria-busy="isLoading">
          <div v-if="!trimmedQuery" class="state">输入关键词或数字房间号，查找想看的直播。</div>
          <div v-if="trimmedQuery && isLoading" class="state" role="status">搜索中...</div>
          <div v-if="trimmedQuery && errorMessage" class="state state--error">{{ errorMessage }}</div>
          <div v-if="hasSearched && !isLoading && !errorMessage && !results.length && trimmedQuery" class="state">没有找到结果</div>
          <button
            v-for="result in results"
            :key="`${result.platform}:${result.accountId || result.secUid || result.roomId}`"
            type="button"
            class="result-item"
            :disabled="result.platform === Platform.DOUYIN && (!result.liveStatus || !result.roomId)"
            @click="selectAnchor(result)"
          >
            <img loading="lazy" decoding="async" v-if="result.avatar" :src="result.avatar" :alt="result.userName" class="avatar" />
            <div v-else class="avatar avatar--fallback">{{ result.userName.slice(0, 1) }}</div>
            <div class="meta">
              <strong>{{ result.userName }}</strong>
              <template v-if="result.platform === Platform.DOUYIN">
                <span>{{ result.kind }} · {{ result.douyinId ? `抖音号：${result.douyinId}` : '抖音号未提供' }}</span>
                <span>{{ followerLabel(result.followers) }} · {{ statusLabel(result) }}</span>
                <span v-if="result.roomTitle">{{ result.roomTitle }}</span>
              </template>
              <span v-else>{{ result.roomTitle || `房间 ${result.roomId}` }}</span>
            </div>
            <span class="status-dot" :class="{ live: result.liveStatus }"></span>
          </button>
        </div>
      </section>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { Search } from 'lucide-vue-next';
import { invoke } from '../services/platformInvoke';
import type { Platform as UiPlatform } from '../layout/types';
import { Platform } from '../platforms/common/types';
import { useImageProxy } from '../components/FollowsList/useProxy';
import { mapAccount, mergeDouyinResults, followerLabel, statusLabel } from '../platforms/douyin/searchResults';
import type { SearchResultItem, DouyinAccountItem } from '../platforms/douyin/searchResults';

interface DouyinApiStreamInfo {
  title?: string | null;
  anchor_name?: string | null;
  avatar?: string | null;
  status?: number | null;
  error_message?: string | null;
  web_rid?: string | null;
}

interface DouyinSearchItem {
  account_id?: string | null;
  sec_uid?: string | null;
  douyin_id?: string | null;
  follower_count?: number | null;
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

interface DouyuSearchItem {
  room_id: string;
  user_name: string;
  room_title?: string | null;
  avatar?: string | null;
  live_status: boolean;
}

const props = defineProps<{
  visible: boolean;
  activePlatform: UiPlatform;
}>();

const emit = defineEmits<{
  (event: 'close'): void;
  (event: 'select-anchor', payload: {
    id: string;
    platform: Platform;
    nickname: string;
    avatarUrl: string | null;
  }): void;
}>();

const platforms: Array<{ id: UiPlatform; label: string }> = [
  { id: 'douyu', label: '斗鱼' },
  { id: 'huya', label: '虎牙' },
  { id: 'douyin', label: '抖音' },
  { id: 'bilibili', label: 'B站' },
];

const inputRef = ref<HTMLInputElement | null>(null);

const sanitizeSearchPlatform = (platform: UiPlatform): UiPlatform => {
  if (platform === 'custom') {
    return 'douyu';
  }
  return platform;
};

const localPlatform = ref<UiPlatform>(sanitizeSearchPlatform(props.activePlatform));
const query = ref('');
const results = ref<SearchResultItem[]>([]);
const isLoading = ref(false);
const hasSearched = ref(false);
const errorMessage = ref('');
const douyinLoginBridge = (window as unknown as {
  DTVDouyinLogin?: { getCookie: () => string; openLogin: () => void };
}).DTVDouyinLogin;

/** Opens official Douyin login; reports bridge failures in the search panel. */
const openDouyinLogin = () => {
  try {
    douyinLoginBridge?.openLogin();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
};
const requestToken = ref(0);
let debounceTimer: number | null = null;

const { ensureProxyStarted, proxify } = useImageProxy();
const trimmedQuery = computed(() => query.value.trim());

const placeholderText = computed(() => {
  if (localPlatform.value === 'douyu') return '搜索斗鱼主播名称/房间号';
  if (localPlatform.value === 'huya') return '搜索虎牙主播名称/房间号';
  if (localPlatform.value === 'douyin') return '输入直播关键字或抖音号';
  if (localPlatform.value === 'bilibili') return '搜索B站主播名称/房间号';
  return '搜索主播/房间';
});

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) {
      resetState();
      return;
    }
    localPlatform.value = sanitizeSearchPlatform(props.activePlatform);
    await nextTick();
    inputRef.value?.focus();
  },
);

watch(
  () => props.activePlatform,
  (platform) => {
    if (props.visible) {
      resetState();
      localPlatform.value = sanitizeSearchPlatform(platform);
    }
  },
);

const resetState = () => {
  hasSearched.value = false;
  requestToken.value += 1;
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
  resetState();
  localPlatform.value = platform;
  if (trimmedQuery.value) {
    void runSearchNow();
  }
};

const handleInput = () => {
  hasSearched.value = false;
  requestToken.value += 1;
  results.value = [];
  errorMessage.value = '';
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
  }, 400);
};

const runSearchNow = async () => {
  if (debounceTimer !== null) {
    window.clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  if (!trimmedQuery.value) {
    resetState();
    return;
  }

  const currentToken = requestToken.value + 1;
  requestToken.value = currentToken;
  results.value = [];
  hasSearched.value = false;
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
      results.value = [];
      errorMessage.value = error instanceof Error ? error.message : String(error || '搜索失败，请重试。');
      isLoading.value = false;
    }
    console.error('Diagnostic: MobileSearchSheet.vue:303 (details omitted)');
  }
};

const commitResults = (currentToken: number, nextResults: SearchResultItem[]) => {
  if (requestToken.value !== currentToken) {
    return;
  }
  hasSearched.value = true;
  results.value = nextResults;
  isLoading.value = false;
  errorMessage.value = '';
};

const runDouyuSearch = async (keyword: string, currentToken: number) => {
  const items = await invoke<DouyuSearchItem[]>('search_anchor', { keyword });
  commitResults(
    currentToken,
    (Array.isArray(items) ? items : []).map((item) => ({
      platform: Platform.DOUYU,
      roomId: String(item.room_id),
      userName: item.user_name || '斗鱼主播',
      roomTitle: item.room_title || null,
      avatar: item.avatar || null,
      liveStatus: item.live_status,
    })),
  );
};

const fetchDouyinRooms = async (keyword: string, cookie: string | null): Promise<SearchResultItem[]> => {
  if (!/^\d+$/.test(keyword)) {
    const items = await invoke<DouyinSearchItem[]>('search_douyin_live_rooms', { keyword, page: 1, cookie });
    return items.map((item) => ({
      platform: Platform.DOUYIN,
      roomId: item.web_rid || item.room_id,
      webId: item.web_rid || null,
      userName: item.nickname,
      roomTitle: item.title,
      avatar: item.avatar || null,
      liveStatus: item.is_live,
      rawStatus: item.status,
      accountId: item.account_id,
      secUid: item.sec_uid,
      douyinId: item.douyin_id,
      followers: item.follower_count,
      roomAliases: [item.web_rid, item.room_id].filter(Boolean),
      kind: '直播间',
    }));
  }
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
        kind: '直播间',
      }]
    : [];
  if (!nextResults.length && info?.error_message) {
    throw new Error(info.error_message);
  }

  return nextResults;
};

/** Runs both sources independently, retaining partial results and invalidating late replies. */
const runDouyinSearch = async (keyword: string, currentToken: number) => {
  const cookie = douyinLoginBridge?.getCookie() || null;
  const sourceResults: SearchResultItem[][] = [[], []];
  const sourceErrors = ['', ''];
  let remaining = 2;
  const requests = [
    async () => (await invoke<DouyinAccountItem[]>('search_douyin_accounts', { keyword, page: 1, cookie })).map(mapAccount),
    async () => fetchDouyinRooms(keyword, cookie),
  ];
  await Promise.all(requests.map(async (request, index) => {
    try {
      sourceResults[index] = await request();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      sourceErrors[index] = `${index === 0 ? '账号' : '直播间'}：${message}`;
    } finally {
      remaining -= 1;
      if (requestToken.value === currentToken) {
        results.value = mergeDouyinResults(sourceResults[0], sourceResults[1]);
        errorMessage.value = sourceErrors.filter(Boolean).join('；');
        isLoading.value = remaining > 0;
        hasSearched.value = remaining === 0;
      }
    }
  }));
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
  const items = await invoke<BilibiliSearchItem[]>('search_bilibili_rooms', { keyword, page: 1, cookie: localStorage.getItem('bilibili_cookie') || null });
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
  if (result.platform === Platform.DOUYIN && (!result.liveStatus || !result.roomId)) return;
  emit('select-anchor', {
    id: result.webId || result.roomId,
    platform: result.platform,
    nickname: result.userName,
    avatarUrl: result.avatar,
  });
  handleClose();
};

/** Retries the active keyword using fresh native cookies after the login view closes. */
const handleDouyinLoginClosed = () => {
  if (props.visible && localPlatform.value === 'douyin' && trimmedQuery.value) {
    void runSearchNow();
  }
};

onMounted(() => window.addEventListener('dtv-douyin-login-closed', handleDouyinLoginClosed));
onUnmounted(() => {
  resetState();
  window.removeEventListener('dtv-douyin-login-closed', handleDouyinLoginClosed);
});
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
  border: 1px solid var(--mobile-border);
  border-bottom: none;
  box-shadow: var(--mobile-sheet-shadow);
}

.login-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 12px;
  color: var(--mobile-text-secondary);
  font-size: 12px;
}

.login-row button {
  flex-shrink: 0;
}

.sheet-header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.sheet-header strong {
  font-size: 18px;
  line-height: 1.2;
}

.sheet-header p {
  margin: 4px 0 0;
  color: var(--mobile-text-secondary);
  font-size: 13px;
}

.close-btn,
.submit-btn {
  border: 1px solid var(--mobile-border);
  border-radius: 12px;
  background: var(--mobile-icon-btn-bg);
  color: var(--mobile-text-primary);
}

.close-btn {
  min-width: 56px;
  min-height: 44px;
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
  font-size: 13px;
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
  border-radius: 15px;
  border: 1px solid var(--mobile-border);
  background: var(--mobile-surface-muted);
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
  color: var(--error-color);
}

.result-item {
  width: 100%;
  border: 1px solid var(--mobile-border);
  border-radius: 14px;
  padding: 12px;
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) 12px;
  gap: 12px;
  align-items: center;
  background: var(--mobile-surface);
  color: var(--mobile-text-primary);
  text-align: left;
}

.avatar {
  width: 44px;
  height: 44px;
  border-radius: 13px;
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
  font-size: 13px;
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.72);
}

.status-dot.live {
  background: var(--status-live);
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

.result-item:disabled {
  opacity: 1;
  cursor: default;
}

.sheet-header strong {
  line-height: 1.4;
}

.sheet-header p {
  line-height: 1.6;
}
</style>
