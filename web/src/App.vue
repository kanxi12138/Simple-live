<template>
  <div class="mobile-app-shell">
    <MobileTopbar
      v-if="!shouldHideChrome && !isPlayerRoute"
      :active-platform="activePlatform"
      :theme="theme"
      :subtitle="subtitle"
      @platform-change="handlePlatformChange"
      @theme-toggle="toggleTheme"
      @open-search="isSearchSheetOpen = true"
      @open-follows="isFollowsSheetOpen = true"
    />

    <header v-else-if="!shouldHideChrome && isPlayerRoute" class="player-topbar">
      <button type="button" class="player-back-btn" @click="router.back()">返回</button>
      <span>{{ subtitle }}</span>
    </header>

    <main class="mobile-app-main" :class="{ 'mobile-app-main--player': isPlayerRoute }">
      <div ref="routeShellRef" class="mobile-route-shell">
        <div
          class="pull-refresh-indicator"
          :class="{
            'pull-refresh-indicator--visible': shouldShowRefreshIndicator,
            'pull-refresh-indicator--armed': isRefreshArmed,
            'pull-refresh-indicator--refreshing': isRefreshing,
            'pull-refresh-indicator--bottom': pullEdge === 'bottom',
          }"
          :style="{
            opacity: shouldShowRefreshIndicator ? 1 : 0,
            transform: `translateY(${pullEdge === 'bottom' ? -indicatorOffset : indicatorOffset}px) scale(${Math.min(1, 0.82 + Math.abs(displayedPullDistance) / 220)})`,
          }"
        >
          <span class="pull-refresh-spinner"></span>
          <span class="pull-refresh-text">{{ refreshHintText }}</span>
        </div>
        <div
          class="mobile-route-shell-content"
          :class="{ 'mobile-route-shell-content--refreshing': isRefreshing }"
          :style="{ transform: `translateY(${displayedPullDistance}px)` }"
        >
        <router-view
          v-slot="{ Component, route }"
          @follow="handleFollowStore"
          @unfollow="handleUnfollowStore"
          @fullscreen-change="handleFullscreenChange"
        >
          <keep-alive :include="['CustomHomeView', 'DouyuHomeView', 'DouyinHomeView', 'HuyaHomeView', 'BilibiliHomeView', 'CustomM3u8HomeView']">
            <component :is="Component" ref="activeRouteComponentRef" :key="route.fullPath" />
          </keep-alive>
        </router-view>
        </div>
      </div>
    </main>

    <MobileBottomNav
      v-if="!shouldHideChrome && !isPlayerRoute"
      :active-tab="activeTab"
      @select="handleBottomTabSelect"
    />

    <MobileSearchSheet
      :visible="isSearchSheetOpen"
      :active-platform="activePlatform"
      @close="isSearchSheetOpen = false"
      @select-anchor="handleSelectAnchorFromSearch"
    />

    <MobileFollowsSheet
      :visible="isFollowsSheetOpen"
      :followed-anchors="followedStreamers"
      @close="isFollowsSheetOpen = false"
      @select-anchor="handleSelectAnchorFromDrawer"
      @unfollow="handleUnfollowStore"
      @reorder-list="handleReorderListStore"
    />

    <MobileSettingsSheet
      :visible="isSettingsSheetOpen"
      :exporting="isExportingConfig"
      :importing="isImportingConfig"
      :app-version="appVersion"
      :status="configStatus"
      :checking-update="isCheckingUpdate"
      :downloading-update="isDownloadingUpdate"
      :update-message="updateMessage"
      :show-update-confirm="isUpdateConfirmVisible"
      :update-confirm-message="updateConfirmMessage"
      :show-import-dialog="showImportDialog"
      @close="isSettingsSheetOpen = false"
      @export-config="handleExportConfig"
      @import-config="handleImportConfig"
      @open-github="openGithub"
      @check-update="handleCheckUpdate"
      @confirm-update="handleConfirmUpdate"
      @cancel-update="handleCancelUpdate"
      @confirm-import="handleConfirmImport"
      @cancel-import="handleCancelImport"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { getVersion } from '@tauri-apps/api/app';
import MobileBottomNav, { type MobileTab } from './mobile/MobileBottomNav.vue';
import MobileFollowsSheet from './mobile/MobileFollowsSheet.vue';
import MobileSearchSheet from './mobile/MobileSearchSheet.vue';
import MobileSettingsSheet from './mobile/MobileSettingsSheet.vue';
import MobileTopbar from './mobile/MobileTopbar.vue';
import { createPortableConfigPayload, parsePortableConfigPayload, replacePortableConfigEntries } from './services/configTransfer';
import { RELEASES_PAGE, type LatestReleaseInfo } from './services/updateChecker';
import { useThemeStore } from './stores/theme';
import { useFollowStore } from './store/followStore';
import { useCustomM3u8Store } from './store/customM3u8Store';
import type { Platform as UiPlatform } from './layout/types';
import { Platform } from './platforms/common/types';
import type { FollowedStreamer } from './platforms/common/types';
import { copyTextToClipboard, openExternal } from './runtime/host';
import { canUseInAppUpdate, downloadLatestReleaseApk, ensureInstallPermission, fallbackToReleasesPage, fetchLatestReleaseInfo, installDownloadedApk } from './runtime/updateHost';
import './styles/global.css';

const router = useRouter();
const route = useRoute();
const followStore = useFollowStore();
const customM3u8Store = useCustomM3u8Store();
const themeStore = useThemeStore();
customM3u8Store.ensureLoaded();

const isSearchSheetOpen = ref(false);
const isFollowsSheetOpen = ref(false);
const isSettingsSheetOpen = ref(false);
const isPlayerFullscreen = ref(false);
const lastPlayerRoute = ref<{ name: string; params: Record<string, string> } | null>(null);
const routeShellRef = ref<HTMLElement | null>(null);
const activeRouteComponentRef = ref<{
  refreshPageContent?: () => Promise<boolean>;
  appendBottomRefreshContent?: () => Promise<boolean>;
  canTriggerBottomRefresh?: () => boolean;
  scrollToTopAnchor?: () => void;
} | null>(null);
const pullDistance = ref(0);
const isPulling = ref(false);
const isRefreshArmed = ref(false);
const isRefreshing = ref(false);
const pullEdge = ref<'top' | 'bottom'>('top');

const isExportingConfig = ref(false);
const isImportingConfig = ref(false);
const showImportDialog = ref(false);
const configStatus = ref<{ tone: 'info' | 'success' | 'error'; text: string } | null>(null);
const appVersion = ref('');
const isCheckingUpdate = ref(false);
const isDownloadingUpdate = ref(false);
const updateMessage = ref('');
const pendingUpdateInfo = ref<LatestReleaseInfo | null>(null);

const theme = computed(() => themeStore.getEffectiveTheme());
const displayedPullDistance = computed(() => {
  if (isRefreshing.value) {
    return pullEdge.value === 'bottom'
      ? Math.min(pullDistance.value, -76)
      : Math.max(pullDistance.value, 76);
  }
  return pullDistance.value;
});
const indicatorOffset = computed(() => Math.min(Math.abs(displayedPullDistance.value) * 0.72, 72));
const shouldShowRefreshIndicator = computed(() => (
  Math.abs(displayedPullDistance.value) > 0 ||
  isRefreshing.value
));
const refreshHintText = computed(() => {
  if (isRefreshing.value) return '正在刷新';
  if (pullEdge.value === 'bottom') {
    if (isRefreshArmed.value) return '松手加载更多';
    return '继续上滑加载更多直播间';
  }
  if (isRefreshArmed.value) return '松手刷新';
  return '继续下拉刷新';
});
const isUpdateConfirmVisible = computed(() => pendingUpdateInfo.value !== null);
const updateConfirmMessage = computed(() => {
  const releaseInfo = pendingUpdateInfo.value;
  if (!releaseInfo) {
    return '';
  }
  return `发现新版本 v${releaseInfo.latestVersion}，是否立即更新？`;
});

const routePlatform = computed<UiPlatform>(() => {
  const name = route.name as string | undefined;
  const path = route.path;
  if (name === 'CustomM3u8Home' || name === 'customM3u8Player' || path.startsWith('/custom-m3u8')) return 'custom-m3u8';
  if (name === 'CustomHome' || path.startsWith('/custom')) return 'custom';
  if (name === 'douyinPlayer' || name === 'DouyinHome' || path.startsWith('/douyin')) return 'douyin';
  if (name === 'huyaPlayer' || name === 'HuyaHome' || path.startsWith('/huya')) return 'huya';
  if (name === 'bilibiliPlayer' || name === 'BilibiliHome' || path.startsWith('/bilibili')) return 'bilibili';
  return 'douyu';
});

const activePlatform = computed<UiPlatform>(() => routePlatform.value);

const followedStreamers = computed<FollowedStreamer[]>(() => followStore.getFollowedStreamers);

const isPlayerRoute = computed(() => {
  const name = route.name as string | undefined;
  return ['douyuPlayer', 'douyinPlayer', 'huyaPlayer', 'bilibiliPlayer', 'customM3u8Player'].includes(name || '');
});
const isHomeRoute = computed(() => {
  const name = route.name as string | undefined;
  return ['DouyuHome', 'DouyinHome', 'HuyaHome', 'BilibiliHome'].includes(name || '');
});
const canUsePullToRefresh = computed(() => isHomeRoute.value && !isPlayerRoute.value);

const shouldHideChrome = computed(() => isPlayerRoute.value && isPlayerFullscreen.value);

const subtitle = computed(() => {
  switch (String(activePlatform.value)) {
    case 'douyin':
      return '抖音直播';
    case 'huya':
      return '虎牙直播';
    case 'bilibili':
      return 'B站直播';
    case 'kuaishou':
      return '快手直播，补齐 Android 端入口';
    case 'neteasecc':
      return '网易 CC，分类与播放统一接入';
    case 'custom-m3u8':
      return '自定义 M3U8 源，独立管理与播放';
    case 'custom':
      return '订阅聚合，回到上次位置';
    default:
      return '斗鱼直播';
  }
});

const activeTab = computed<MobileTab>(() => {
  if (isSettingsSheetOpen.value) return 'settings';
  if (isFollowsSheetOpen.value) return 'follows';
  if (isSearchSheetOpen.value) return 'search';
  if (isPlayerRoute.value) return 'player';
  return 'browse';
});

const platformToHomeRoute = (platform: UiPlatform) => {
  switch (platform) {
    case 'custom':
      return { name: 'CustomHome' };
    case 'custom-m3u8':
      return { name: 'CustomM3u8Home' };
    case 'douyin':
      return { name: 'DouyinHome' };
    case 'huya':
      return { name: 'HuyaHome' };
    case 'bilibili':
      return { name: 'BilibiliHome' };
    default:
      return { name: 'DouyuHome' };
  }
};

const handlePlatformChange = (platform: UiPlatform) => {
  router.push(platformToHomeRoute(platform));
};

const pushPlayerRoute = (platform: Platform, roomId: string) => {
  let target: { name: string; params: Record<string, string> };
  if (platform === Platform.DOUYIN) {
    target = { name: 'douyinPlayer', params: { roomId } };
  } else if (platform === Platform.HUYA) {
    target = { name: 'huyaPlayer', params: { roomId } };
  } else if (platform === Platform.BILIBILI) {
    target = { name: 'bilibiliPlayer', params: { roomId } };
  } else if (platform === Platform.CUSTOM_M3U8) {
    target = { name: 'customM3u8Player', params: { encodedId: encodeURIComponent(roomId) } };
  } else {
    target = { name: 'douyuPlayer', params: { roomId } };
  }
  lastPlayerRoute.value = target;
  router.push(target);
};

const handleSelectAnchor = (streamer: FollowedStreamer) => {
  pushPlayerRoute(streamer.platform, streamer.id);
};

const handleSelectAnchorFromSearch = (payload: { id: string; platform: Platform; nickname: string; avatarUrl: string | null }) => {
  handleSelectAnchor({
    id: payload.id,
    platform: payload.platform,
    nickname: payload.nickname,
    avatarUrl: payload.avatarUrl ?? '',
    currentRoomId: payload.id,
    liveStatus: 'UNKNOWN',
  });
};

const handleSelectAnchorFromDrawer = (streamer: FollowedStreamer) => {
  isFollowsSheetOpen.value = false;
  handleSelectAnchor(streamer);
};

const handleFollowStore = (streamer: FollowedStreamer) => {
  followStore.followStreamer(streamer);
};

const handleUnfollowStore = (payload: { platform: Platform; id: string } | string) => {
  if (typeof payload === 'string') {
    followStore.unfollowStreamer(Platform.DOUYU, payload);
  } else {
    followStore.unfollowStreamer(payload.platform, payload.id);
  }
};

const handleReorderListStore = (reorderedList: FollowedStreamer[]) => {
  followStore.updateOrder(reorderedList);
};

const handleFullscreenChange = (isFullscreen: boolean) => {
  isPlayerFullscreen.value = isFullscreen;
};

const PULL_REFRESH_THRESHOLD = 92;
const PULL_REFRESH_MAX = 148;
const PULL_AXIS_LOCK = 8;
const BOTTOM_EDGE_EPSILON = 16;
const refreshTouchState = {
  startX: 0,
  startY: 0,
  tracking: false,
  engaged: false,
  edge: 'top' as 'top' | 'bottom',
};

const dampPullDistance = (distance: number) => {
  if (distance <= 0) return 0;
  const softened = distance * 0.62;
  const tail = Math.max(0, distance - 72) * 0.22;
  return Math.min(PULL_REFRESH_MAX, softened - tail);
};

const resetPullState = () => {
  pullDistance.value = 0;
  isPulling.value = false;
  isRefreshArmed.value = false;
  refreshTouchState.tracking = false;
  refreshTouchState.engaged = false;
  refreshTouchState.edge = 'top';
  pullEdge.value = 'top';
  refreshTouchState.startX = 0;
  refreshTouchState.startY = 0;
};

const triggerCurrentPageRefresh = async (edge: 'top' | 'bottom') => {
  if (!canUsePullToRefresh.value) return false;
  if (edge === 'bottom') {
    return await activeRouteComponentRef.value?.appendBottomRefreshContent?.() ?? false;
  }
  return await activeRouteComponentRef.value?.refreshPageContent?.() ?? false;
};

const canTriggerBottomRefresh = () => (
  canUsePullToRefresh.value &&
  (activeRouteComponentRef.value?.canTriggerBottomRefresh?.() ?? false)
);

const isAtBottomEdge = (shell: HTMLElement) => (
  shell.scrollTop + shell.clientHeight >= shell.scrollHeight - BOTTOM_EDGE_EPSILON
);

const handlePullTouchStart = (event: TouchEvent) => {
  if (!canUsePullToRefresh.value || isRefreshing.value) return;
  const shell = routeShellRef.value;
  const touch = event.touches[0];
  if (!shell || !touch) return;

  const atTop = shell.scrollTop <= 0;
  const atBottom = isAtBottomEdge(shell);
  if (!atTop && !(atBottom && canTriggerBottomRefresh())) return;

  refreshTouchState.tracking = true;
  refreshTouchState.engaged = false;
  refreshTouchState.edge = atTop ? 'top' : 'bottom';
  pullEdge.value = refreshTouchState.edge;
  refreshTouchState.startX = touch.clientX;
  refreshTouchState.startY = touch.clientY;
};

const handlePullTouchMove = (event: TouchEvent) => {
  if (!refreshTouchState.tracking || isRefreshing.value) return;
  const shell = routeShellRef.value;
  const touch = event.touches[0];
  if (!shell || !touch) return;

  const deltaX = touch.clientX - refreshTouchState.startX;
  const deltaY = touch.clientY - refreshTouchState.startY;

  if (!refreshTouchState.engaged) {
    if (Math.abs(deltaY) < PULL_AXIS_LOCK) return;
    const invalidTopGesture = refreshTouchState.edge === 'top' && (
      deltaY <= 0 || Math.abs(deltaY) <= Math.abs(deltaX) || shell.scrollTop > 0
    );
    const invalidBottomGesture = refreshTouchState.edge === 'bottom' && (
      deltaY >= 0 || Math.abs(deltaY) <= Math.abs(deltaX) || !isAtBottomEdge(shell) || !canTriggerBottomRefresh()
    );
    if (invalidTopGesture || invalidBottomGesture) {
      resetPullState();
      return;
    }
    refreshTouchState.engaged = true;
  }

  event.preventDefault();
  const nextDistance = refreshTouchState.edge === 'top'
    ? dampPullDistance(deltaY)
    : dampPullDistance(-deltaY);
  pullDistance.value = refreshTouchState.edge === 'top' ? nextDistance : -nextDistance;
  isPulling.value = nextDistance > 0;
  isRefreshArmed.value = nextDistance >= PULL_REFRESH_THRESHOLD;
};

const finishPullRefresh = () => new Promise<void>((resolve) => {
  window.setTimeout(() => {
    resetPullState();
    resolve();
  }, 140);
});

const handlePullTouchEnd = async () => {
  if (!refreshTouchState.tracking) return;
  const shouldRefresh = refreshTouchState.engaged && isRefreshArmed.value && !isRefreshing.value;
  const refreshEdge = pullEdge.value;
  refreshTouchState.tracking = false;
  refreshTouchState.engaged = false;

  if (!shouldRefresh) {
    resetPullState();
    return;
  }

  isRefreshing.value = true;
  pullDistance.value = pullEdge.value === 'bottom'
    ? Math.min(pullDistance.value, -76)
    : Math.max(pullDistance.value, 76);
  try {
    await triggerCurrentPageRefresh(refreshEdge);
  } finally {
    isRefreshing.value = false;
    await finishPullRefresh();
  }
};

const toggleTheme = () => {
  themeStore.toggleTheme();
};

const handleBottomTabSelect = (tab: MobileTab) => {
  if (tab === 'browse') {
    if (isPlayerRoute.value) {
      router.push(platformToHomeRoute(activePlatform.value));
    }
    isSearchSheetOpen.value = false;
    isFollowsSheetOpen.value = false;
    isSettingsSheetOpen.value = false;
    return;
  }

  if (tab === 'search') {
    isSettingsSheetOpen.value = false;
    isFollowsSheetOpen.value = false;
    isSearchSheetOpen.value = true;
    return;
  }

  if (tab === 'follows') {
    isSettingsSheetOpen.value = false;
    isSearchSheetOpen.value = false;
    isFollowsSheetOpen.value = true;
    return;
  }

  if (tab === 'settings') {
    isSearchSheetOpen.value = false;
    isFollowsSheetOpen.value = false;
    isSettingsSheetOpen.value = true;
    return;
  }

  if (lastPlayerRoute.value) {
    router.push(lastPlayerRoute.value);
  }
};

const handleExportConfig = async () => {
  if (isExportingConfig.value || typeof window === 'undefined' || !window.localStorage) return;

  isExportingConfig.value = true;
  configStatus.value = { tone: 'info', text: '正在生成配置文件...' };
  try {
    const payload = createPortableConfigPayload(window.localStorage, {
      client: 'android',
      appVersion: appVersion.value || null,
    });
    await copyTextToClipboard(JSON.stringify(payload));
    configStatus.value = { tone: 'success', text: '已导出所有配置至粘贴板！' };
  } catch (error) {
    console.error('[App] Export config failed:', error);
    configStatus.value = { tone: 'error', text: '配置导出失败，请稍后重试。' };
  } finally {
    isExportingConfig.value = false;
  }
};

const handleImportConfig = () => {
  if (isImportingConfig.value) return;
  showImportDialog.value = true;
};

const handleConfirmImport = (rawText: string) => {
  if (!rawText.trim()) {
    configStatus.value = { tone: 'error', text: '导入失败！' };
    return;
  }
  try {
    const payload = parsePortableConfigPayload(rawText.trim());
    replacePortableConfigEntries(window.localStorage, payload.entries);
    configStatus.value = { tone: 'success', text: '导入成功！' };
    showImportDialog.value = false;
    window.setTimeout(() => window.location.reload(), 360);
  } catch (error) {
    console.error('[App] Import config failed:', error);
    configStatus.value = { tone: 'error', text: '导入失败！' };
  }
};

const handleCancelImport = () => {
  showImportDialog.value = false;
};

const openGithub = async () => {
  await openExternal('https://github.com/kanxi12138/Simple-live');
};

const handleCheckUpdate = async () => {
  if (isCheckingUpdate.value || isDownloadingUpdate.value) return;
  isCheckingUpdate.value = true;
  pendingUpdateInfo.value = null;
  updateMessage.value = '';
  try {
    const result = await fetchLatestReleaseInfo();
    appVersion.value = result.currentVersion || appVersion.value;
    if (!result.hasUpdate) {
      updateMessage.value = '当前已经是最新版本 ' + (result.currentVersion || appVersion.value || '4.9.0') + '。';
      return;
    }
    if (!result.apkAsset) {
      updateMessage.value = '发现新版本，但未找到可安装的 APK。';
      return;
    }
    if (!canUseInAppUpdate()) {
      updateMessage.value = '当前环境不支持内置安装，正在打开 Releases 页面。';
      await fallbackToReleasesPage(result.htmlUrl || RELEASES_PAGE);
      return;
    }
    pendingUpdateInfo.value = result;
    updateMessage.value = '发现新版本 v' + result.latestVersion + '。';
  } catch (error: unknown) {
    console.error('[App] Update check failed:', error);
    updateMessage.value = '检查更新失败，请稍后重试。';
  } finally {
    isCheckingUpdate.value = false;
  }
};

const handleCancelUpdate = () => {
  pendingUpdateInfo.value = null;
  if (!isDownloadingUpdate.value) {
    updateMessage.value = '已取消更新。';
  }
};

const handleConfirmUpdate = async () => {
  if (isDownloadingUpdate.value) {
    return;
  }

  const releaseInfo = pendingUpdateInfo.value;
  if (!releaseInfo) {
    return;
  }

  pendingUpdateInfo.value = null;
  if (!ensureInstallPermission()) {
    updateMessage.value = '请允许本应用安装未知来源应用后重试。';
    return;
  }

  isDownloadingUpdate.value = true;
  updateMessage.value = '正在下载更新...';

  try {
    const downloadResult = await downloadLatestReleaseApk();
    const installStarted = installDownloadedApk(downloadResult.filePath);
    if (!installStarted) {
      throw new Error('Failed to open the Android package installer.');
    }
    updateMessage.value = '下载完成，正在打开安装程序...';
  } catch (error: unknown) {
    console.error('[App] Update download failed:', error);
    updateMessage.value = '更新失败，请稍后重试。';
  } finally {
    isDownloadingUpdate.value = false;
  }
};

onMounted(async () => {
  try {
    appVersion.value = await getVersion();
  } catch (error) {
    console.warn('[App] Failed to resolve app version:', error);
  }

  const shell = routeShellRef.value;
  if (shell) {
    shell.addEventListener('touchstart', handlePullTouchStart, { passive: true });
    shell.addEventListener('touchmove', handlePullTouchMove, { passive: false });
    shell.addEventListener('touchend', handlePullTouchEnd, { passive: true });
    shell.addEventListener('touchcancel', handlePullTouchEnd, { passive: true });
  }
});

onBeforeUnmount(() => {
  const shell = routeShellRef.value;
  if (shell) {
    shell.removeEventListener('touchstart', handlePullTouchStart);
    shell.removeEventListener('touchmove', handlePullTouchMove);
    shell.removeEventListener('touchend', handlePullTouchEnd);
    shell.removeEventListener('touchcancel', handlePullTouchEnd);
  }
});
</script>

<style scoped>
.mobile-app-shell {
  height: 100%;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  background:
    radial-gradient(circle at top, var(--mobile-shell-glow, rgba(37, 99, 235, 0.16)), transparent 24%),
    var(--mobile-shell-bg, var(--bg-primary));
}

.mobile-app-main {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0 0 calc(var(--mobile-bottom-nav-height) + env(safe-area-inset-bottom));
}

.mobile-app-main--player {
  padding-bottom: 0;
}

.mobile-route-shell {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding-bottom: 12px;
  position: relative;
  overflow-y: auto;
  overflow-x: hidden;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior-y: contain;
  overflow-anchor: none;
}

.mobile-route-shell-content {
  display: flex;
  flex: 1;
  min-height: 100%;
  flex-direction: column;
  will-change: transform;
  transition: transform 0.22s cubic-bezier(0.22, 1, 0.36, 1);
  overflow-anchor: none;
}

.mobile-route-shell-content--refreshing {
  transition-duration: 0.18s;
}

.pull-refresh-indicator {
  position: absolute;
  top: max(10px, env(safe-area-inset-top));
  left: 50%;
  z-index: 30;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  padding: 0 14px;
  border-radius: 999px;
  background: var(--mobile-surface-strong, rgba(255, 255, 255, 0.96));
  color: var(--mobile-text-primary, var(--text-primary));
  border: 1px solid var(--mobile-border, rgba(148, 163, 184, 0.22));
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.16);
  transform-origin: center top;
  pointer-events: none;
  transition:
    opacity 0.16s ease,
    transform 0.22s cubic-bezier(0.22, 1, 0.36, 1),
    background-color 0.2s ease,
    border-color 0.2s ease;
}

.pull-refresh-indicator--bottom {
  top: auto;
  bottom: calc(84px + env(safe-area-inset-bottom));
}

.pull-refresh-indicator--armed {
  background: var(--mobile-pill-active-bg, rgba(34, 197, 94, 0.16));
  border-color: var(--mobile-pill-active-border, rgba(34, 197, 94, 0.34));
}

.pull-refresh-indicator--refreshing {
  background: var(--mobile-surface-strong, rgba(255, 255, 255, 0.98));
}

.pull-refresh-spinner {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid currentColor;
  border-top-color: transparent;
  opacity: 0.72;
}

.pull-refresh-indicator--armed .pull-refresh-spinner,
.pull-refresh-indicator--refreshing .pull-refresh-spinner {
  opacity: 1;
}

.pull-refresh-indicator--refreshing .pull-refresh-spinner {
  animation: pull-refresh-spin 0.88s linear infinite;
}

.pull-refresh-text {
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
}

.player-topbar {
  position: sticky;
  top: 0;
  z-index: 35;
  min-height: 52px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: max(10px, env(safe-area-inset-top)) 12px 10px;
  background: var(--mobile-topbar-bg, rgba(2, 6, 23, 0.86));
  color: var(--mobile-topbar-text, var(--text-primary));
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border-bottom: 1px solid var(--mobile-topbar-border, rgba(148, 163, 184, 0.12));
}

.player-back-btn {
  min-width: 56px;
  min-height: 34px;
  border: none;
  border-radius: 10px;
  background: var(--mobile-icon-btn-bg, rgba(148, 163, 184, 0.16));
  color: var(--mobile-topbar-text, var(--text-primary));
  font-weight: 700;
}

@keyframes pull-refresh-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
