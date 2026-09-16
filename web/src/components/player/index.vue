<template>
  <div class="player-page" :class="{ 'web-fs': isInWebFullscreen || isInNativePlayerFullscreen }">
    <Transition name="player-floating-action">
      <button v-if="!isInWebFullscreen" @click="handleClosePlayerClick" class="player-close-btn" title="关闭播放器">
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    </Transition>

    <div class="player-layout">
      <div class="main-content">
        <div v-if="!roomId" class="empty-player">
          <div class="empty-icon">
            <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
               <circle cx="12" cy="12" r="10"></circle>
               <line x1="12" y1="16" x2="12" y2="12"></line>
               <line x1="12" y1="8" x2="12.01" y2="8"></line>
            </svg>
          </div>
          <h3>未选择直播间</h3>
          <p>请从首页选择一个直播间开始观看。</p>
        </div>
        <div v-else-if="isLoadingStream" class="loading-player">
          <LoadingDots />
        </div>
        <div
          v-else-if="isOfflineError"
          class="player-container player-container--solo player-container--offline"
        >
          <Transition name="player-panel-fade">
            <StreamerInfo
              v-if="props.roomId && props.platform"
              :room-id="props.roomId"
              :platform="props.platform"
              :title="playerTitle"
              :anchor-name="playerAnchorName"
              :avatar="playerAvatar"
              :is-live="false"
              :is-followed="props.isFollowed"
              @follow="$emit('follow', $event)"
              @unfollow="$emit('unfollow', $event)"
              @details="handleStreamerDetails"
              class="streamer-info-offline"
            />
          </Transition>
          <div class="video-container video-container--offline">
            <div class="offline-placeholder">
              <span class="offline-placeholder__label">当前主播未开播！</span>
            </div>
          </div>
        </div>
        <div v-else-if="streamError && !isOfflineError" class="error-player">
          <div class="error-icon">
             <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <circle cx="12" cy="12" r="10"></circle>
              <line x1="12" y1="8" x2="12" y2="12"></line>
              <line x1="12" y1="16" x2="12.01" y2="16"></line>
            </svg>
          </div>
          <h3>加载失败</h3>
          <p>{{ streamError }}</p>
          <button @click="retryInitialization" class="retry-btn">再试一次</button>
        </div>
        <div v-else class="player-container" :class="{ 'player-container--solo': !isDanmuVisible }">
          <Transition name="player-panel-fade">
            <StreamerInfo
              v-if="props.roomId && !isInWebFullscreen"
              :room-id="props.roomId"
              :platform="props.platform"
              :title="playerTitle"
              :anchor-name="playerAnchorName"
              :avatar="playerAvatar"
              :is-followed="props.isFollowed"
              :is-live="playerIsLive"
              @follow="$emit('follow', $event)"
              @unfollow="$emit('unfollow', $event)"
              @details="handleStreamerDetails"
              class="streamer-info"
              :class="{'hidden-panel': isInWebFullscreen}"
            />
          </Transition>
          <div class="video-container">
            <div ref="playerContainerRef" class="video-player"></div>
          </div>
        </div>
      </div>

      <Transition name="danmu-panel-slide">
        <DanmuList
          v-if="roomId && !isLoadingStream && !streamError && isDanmuVisible && !isFullScreen"
          :room-id="props.roomId"
          :messages="danmakuMessages"
          class="danmu-panel"
          :class="{'hidden-panel': isFullScreen}"
          ref="danmuListRef"
        >
          <template #actions>
            <Transition name="player-panel-actions">
              <div
                v-if="canToggleDanmuPanel && !isFullScreen"
                class="danmu-panel-actions"
              >
                <button
                  type="button"
                  class="danmu-filter-toggle-btn"
                  title="屏蔽关键词"
                  @click="toggleDanmuFilterPanel"
                >
                  <Funnel :size="18" />
                </button>
              </div>
            </Transition>
          </template>
        </DanmuList>
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import { diagnosticDetails } from '../../services/diagnostics';
import { queueStreamProxy, stopStreamProxy as stopDouyuProxy } from '../../platforms/common/playbackProxy';
import type { PlaybackConfig, PlaybackLine } from '../../platforms/common/playback';
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, shallowRef } from 'vue';
import Player from 'xgplayer';
import FlvPlugin from 'xgplayer-flv';
import HlsPlugin from 'xgplayer-hls.js';
import { POSITIONS } from 'xgplayer/es/plugin/plugin.js';
import { Funnel } from 'lucide-vue-next';
import 'xgplayer/dist/index.min.css';

import './player.css';

import { Platform as StreamingPlatform } from '../../platforms/common/types';
import type { DanmakuMessage, DanmuOverlayInstance } from './types';
import {
  applyDanmuFontFamilyForOS,
  ICONS,
  loadDanmuPreferences,
  loadStoredVolume,
  persistDanmuPreferences,
  sanitizeDanmuArea,
  sanitizeDanmuDensity,
  sanitizeDanmuOpacity,
  type DanmuUserSettings,
} from './constants';
import {
  DanmuSettingsControl,
  DanmuToggleControl,
  LineControl,
  QualityControl,
  RefreshControl,
  VolumeControl,
} from './plugins';
import { arrangeControlClusters } from './controlLayout';
import { applyDanmuOverlayPreferences, createDanmuOverlay, ensureDanmuOverlayHost, syncDanmuEnabledState } from './danmuOverlay';
import { registerPlayerWatchers, type PlayerProps } from './watchers';
import { startCurrentDanmakuListener as startDanmakuListener, stopCurrentDanmakuListener as stopDanmakuListener } from './danmakuManager';
import { getLineLabel, persistLinePreference, resolveStoredLine } from './lineOptions';

// Platform-specific player helpers
import { getDouyuStreamConfig, isDouyuOfflineMessage } from '../../platforms/douyu/playerHelper';
import { fetchAndPrepareDouyinStreamConfig } from '../../platforms/douyin/playerHelper';
import { getHuyaStreamConfig } from '../../platforms/huya/playerHelper';
import { getBilibiliStreamConfig } from '../../platforms/bilibili/playerHelper';

import StreamerInfo from '../StreamerInfo/index.vue';
import DanmuList from '../DanmuList/index.vue';
import LoadingDots from '../Common/LoadingDots.vue';

import { invoke } from '../../services/platformInvoke';
import { useImageProxy } from '../FollowsList/useProxy';

// Ensure image proxy helpers are available in this component
const { ensureProxyStarted, proxify } = useImageProxy();

const props = defineProps<PlayerProps>();

const emit = defineEmits<{
  (e: 'follow', streamer: any): void;
  (e: 'unfollow', roomId: string): void;
  (e: 'close-player'): void;
  (e: 'fullscreen-change', isFullscreen: boolean): void;
  (e: 'request-refresh-details'): void;
  (e: 'request-player-reload'): void;
}>();

const isClosing = ref(false);
const supportsDanmuForPlatform = (platform: StreamingPlatform | null | undefined) => {
  if (platform == null) {
    return false;
  }
  return [
    StreamingPlatform.DOUYU,
    StreamingPlatform.DOUYIN,
    StreamingPlatform.HUYA,
    StreamingPlatform.BILIBILI,
  ].includes(platform);
};
const supportsQualityForPlatform = (platform: StreamingPlatform | null | undefined) => {
  if (platform == null) {
    return false;
  }
  return [
    StreamingPlatform.DOUYU,
    StreamingPlatform.DOUYIN,
    StreamingPlatform.HUYA,
    StreamingPlatform.BILIBILI,
  ].includes(platform);
};
const supportsDanmu = computed(() => supportsDanmuForPlatform(props.platform));
const supportsQuality = computed(() => supportsQualityForPlatform(props.platform));
const canToggleDanmuPanel = computed(() => supportsDanmu.value && !!props.roomId && !isLoadingStream.value && !streamError.value);
const isDanmuVisible = computed(() => canToggleDanmuPanel.value);

const toggleDanmuFilterPanel = () => {
  danmuListRef.value?.toggleFilterPanel?.();
};

const playerContainerRef = ref<HTMLDivElement | null>(null);
const danmuListRef = ref<{ toggleFilterPanel?: () => void } | null>(null);
const playerInstance = shallowRef<Player | null>(null);
const refreshControlPlugin = shallowRef<RefreshControl | null>(null);
const qualityControlPlugin = shallowRef<QualityControl | null>(null);
const lineControlPlugin = shallowRef<LineControl | null>(null);
const danmuTogglePlugin = shallowRef<DanmuToggleControl | null>(null);
const danmuSettingsPlugin = shallowRef<DanmuSettingsControl | null>(null);
const volumeControlPlugin = shallowRef<VolumeControl | null>(null);
const danmuInstance = shallowRef<DanmuOverlayInstance | null>(null);
const danmakuMessages = ref<DanmakuMessage[]>([]);
const isDanmakuListenerActive = ref(false); // Tracks if a danmaku listener is supposed to be running
const unlistenDanmakuFn = ref<(() => void) | null>(null);

const isLoadingStream = ref(true);
const streamError = ref<string | null>(null);
const isOfflineError = ref(false); // Added to track '涓绘挱鏈紑鎾? state

// Reactive state for streamer info, initialized by props, potentially updated by internal fetches (for Douyin)
const playerTitle = ref(props.title);
const playerAnchorName = ref(props.anchorName);
const playerAvatar = ref(props.avatar);
const playerIsLive = ref(props.isLive);

const isInNativePlayerFullscreen = ref(false); // New: Tracks Artplayer element's native fullscreen
const isInWebFullscreen = ref(false);
const isFullScreen = ref(false); // True if EITHER native player OR web fullscreen is active

const isDanmuEnabled = ref(true);
const danmuSettings = reactive<DanmuUserSettings>({
  fontSize: '20px',
  duration: 10000,
  area: 0.5,
  mode: 'scroll',
  opacity: 1,
  density: 'medium',
});

const storedDanmuPreferences = loadDanmuPreferences();
if (storedDanmuPreferences) {
  isDanmuEnabled.value = storedDanmuPreferences.enabled;
  Object.assign(danmuSettings, storedDanmuPreferences.settings);
}

// OS specific states
const osName = ref<string>('');

// 画质切换相关
const qualityOptions = ref<string[]>([]);

const resolveStoredQuality = (platform?: StreamingPlatform | null): string => {
  if (!platform) {
    return '原画';
  }
  if (typeof window === 'undefined') {
    return '原画';
  }
  try {
    const saved = window.localStorage.getItem(`${platform}_preferred_quality`);
    if (saved) {
      return saved;
    }
  } catch (error) {
    console.warn('Diagnostic: index.vue:284 (details omitted)');
  }
  return '原画';
};

const currentQuality = ref<string>(resolveStoredQuality(props.platform));
const isQualitySwitching = ref(false);
const isRefreshingStream = ref(false);
const isLineSwitching = ref(false);

interface OrientationBridge {
  enterLandscapeFullscreen: () => void;
  enterPortraitFullscreen: () => void;
  exitFullscreen: () => void;
}

type DtvWindow = Window & typeof globalThis & {
  DTVOrientation?: OrientationBridge;
  __DTV_HANDLE_ANDROID_BACK__?: () => boolean;
};


const currentLine = ref<string | null>(resolveStoredLine(props.platform));
const availableLines = ref<PlaybackLine[]>([]);
const lineOptions = computed(() => availableLines.value);
const getCurrentLineLabel = (key?: string | null) => getLineLabel(lineOptions.value, key);
let playerInitRunId = 0;
const isAndroidRuntime = typeof navigator !== 'undefined' && /android/i.test(navigator.userAgent || '');
const FULLSCREEN_HISTORY_FLAG = '__dtv_player_fullscreen__';
const FULLSCREEN_CONTROL_SELECTOR = [
  '.xgplayer-cssfullscreen',
  '.xgplayer-fullscreen',
  '.xg-get-cssfull',
  '.xg-exit-cssfull',
  '.xg-get-fullscreen',
  '.xg-exit-fullscreen',
].join(', ');
let fullscreenHistoryActive = false;
let ignoreNextFullscreenPop = false;
let fullscreenControlCleanup: (() => void) | null = null;
let videoOrientationCleanup: (() => void) | null = null;
let pendingFullscreenRestore = false;
let preserveFullscreenDuringReload = false;

const handleStreamerDetails = (payload: { title: string; nickname: string; avatarUrl: string | null; isLive: boolean | null }) => {
  if (payload.title) {
    playerTitle.value = payload.title;
  }
  if (payload.nickname) {
    playerAnchorName.value = payload.nickname;
  }
  playerAvatar.value = payload.avatarUrl ?? playerAvatar.value ?? null;
  if (typeof payload.isLive === 'boolean') {
    playerIsLive.value = payload.isLive;
  }
};

function resetFullscreenState() {
  isInNativePlayerFullscreen.value = false;
  isInWebFullscreen.value = false;
  isFullScreen.value = false;
  syncPlayerOrientation(false);
  fullscreenHistoryActive = false;
  ignoreNextFullscreenPop = false;
  try {
    document.documentElement.classList.remove('web-fs-active');
  } catch (error) {
    console.warn('Diagnostic: index.vue:351 (details omitted)');
  }
}

function updateFullscreenFlag() {
  if (isClosing.value) {
    return;
  }
  if (
    preserveFullscreenDuringReload
    && !isInNativePlayerFullscreen.value
    && !isInWebFullscreen.value
  ) {
    isFullScreen.value = true;
    syncPlayerOrientation(true);
    emit('fullscreen-change', true);
    return;
  }
  isFullScreen.value = isInNativePlayerFullscreen.value || isInWebFullscreen.value;
  syncPlayerOrientation(isFullScreen.value);
  emit('fullscreen-change', isFullScreen.value);
}

function getDtvWindow(): DtvWindow | null {
  if (typeof window === 'undefined') {
    return null;
  }
  return window as DtvWindow;
}

function getOrientationBridge(): OrientationBridge | null {
  const bridge = getDtvWindow()?.DTVOrientation;
  if (!bridge) {
    return null;
  }
  return bridge;
}

function setNativeOrientation(mode: 'landscape' | 'portrait') {
  if (!isAndroidRuntime) {
    return;
  }
  try {
    const bridge = getOrientationBridge();
    if (!bridge) {
      return;
    }
    if (mode === 'landscape') {
      bridge.enterLandscapeFullscreen();
      return;
    }
    bridge.enterPortraitFullscreen();
  } catch (error) {
    console.warn('Diagnostic: index.vue:404 (details omitted)');
  }
}

function exitNativeFullscreen() {
  if (!isAndroidRuntime) {
    return;
  }
  try {
    getOrientationBridge()?.exitFullscreen();
  } catch (error) {
    console.warn('Diagnostic: index.vue:415 (details omitted)');
  }
}

function getPlayerVideoElement(player: Player | null): HTMLVideoElement | null {
  if (player?.media instanceof HTMLVideoElement) {
    return player.media;
  }
  const videoElement = player?.root?.querySelector('video');
  return videoElement instanceof HTMLVideoElement ? videoElement : null;
}

function resolveVideoOrientation(): 'landscape' | 'portrait' {
  const media = getPlayerVideoElement(playerInstance.value);
  if (!media || media.videoWidth <= 0 || media.videoHeight <= 0) {
    return 'landscape';
  }
  return media.videoHeight > media.videoWidth ? 'portrait' : 'landscape';
}

function bindVideoOrientation(player: Player) {
  videoOrientationCleanup?.();
  videoOrientationCleanup = null;
  const media = getPlayerVideoElement(player);
  if (!media) {
    return;
  }
  const handleDimensionsChanged = () => {
    if (isFullScreen.value || preserveFullscreenDuringReload) {
      setNativeOrientation(resolveVideoOrientation());
    }
  };
  media.addEventListener('loadedmetadata', handleDimensionsChanged);
  media.addEventListener('resize', handleDimensionsChanged);
  videoOrientationCleanup = () => {
    media.removeEventListener('loadedmetadata', handleDimensionsChanged);
    media.removeEventListener('resize', handleDimensionsChanged);
  };
}

function ensureFullscreenHistoryEntry() {
  if (typeof window === 'undefined' || fullscreenHistoryActive) {
    return;
  }
  try {
    const currentState = typeof window.history.state === 'object' && window.history.state !== null
      ? window.history.state
      : {};
    window.history.pushState(
      {
        ...currentState,
        [FULLSCREEN_HISTORY_FLAG]: true,
      },
      '',
      window.location.href,
    );
    fullscreenHistoryActive = true;
  } catch (error) {
    console.warn('Diagnostic: index.vue:473 (details omitted)');
  }
}

function clearFullscreenHistoryEntry() {
  if (typeof window === 'undefined' || !fullscreenHistoryActive) {
    return;
  }
  fullscreenHistoryActive = false;
  try {
    ignoreNextFullscreenPop = true;
    window.history.back();
  } catch (error) {
    ignoreNextFullscreenPop = false;
    console.warn('Diagnostic: index.vue:487 (details omitted)');
  }
}

function syncPlayerOrientation(isFullscreen: boolean) {
  if (isFullscreen) {
    setNativeOrientation(resolveVideoOrientation());
    ensureFullscreenHistoryEntry();
    return;
  }
  exitNativeFullscreen();
  clearFullscreenHistoryEntry();
}

function exitPlayerFullscreenFromSystemBack() {
  const player = playerInstance.value as (Player & {
    getCssFullscreen?: () => void;
    exitCssFullscreen?: () => void;
    exitFullscreen?: () => void;
    isCssfullScreen?: boolean;
  }) | null;
  const isCurrentlyFullscreen = Boolean(
    player?.isCssfullScreen || isInWebFullscreen.value || isInNativePlayerFullscreen.value,
  );
  if (!player || !isCurrentlyFullscreen) {
    exitNativeFullscreen();
    return;
  }
  togglePlayerFullscreen(false);
}

function togglePlayerFullscreen(forceFullscreen?: boolean) {
  const player = playerInstance.value as (Player & {
    getCssFullscreen?: () => void;
    exitCssFullscreen?: () => void;
    exitFullscreen?: () => void;
    isCssfullScreen?: boolean;
  }) | null;
  if (!player) {
    if (forceFullscreen === false) {
      exitNativeFullscreen();
    }
    return;
  }

  const isCurrentlyFullscreen = Boolean(
    player.isCssfullScreen || isInWebFullscreen.value || isInNativePlayerFullscreen.value,
  );
  const shouldEnterFullscreen = typeof forceFullscreen === 'boolean'
    ? forceFullscreen
    : !isCurrentlyFullscreen;

  try {
    if (shouldEnterFullscreen) {
      setNativeOrientation(resolveVideoOrientation());
      player.getCssFullscreen?.();
    } else if (isInWebFullscreen.value && typeof player.exitCssFullscreen === 'function') {
      exitNativeFullscreen();
      player.exitCssFullscreen();
    } else if (typeof player.exitFullscreen === 'function') {
      exitNativeFullscreen();
      player.exitFullscreen();
    } else {
      resetFullscreenState();
    }
  } catch (error) {
    console.warn('Diagnostic: index.vue:553 (details omitted)');
    if (!shouldEnterFullscreen) {
      exitNativeFullscreen();
    }
  }
}

function bindFullscreenControlFallback(player: Player & {
  getCssFullscreen?: () => void;
  exitCssFullscreen?: () => void;
  isCssfullScreen?: boolean;
}) {
  if (fullscreenControlCleanup) {
    fullscreenControlCleanup();
    fullscreenControlCleanup = null;
  }

  const root = player.root as HTMLElement | null;
  if (!root || typeof window === 'undefined') {
    return;
  }

  let lastHandledAt = 0;
  const handleFullscreenIntent = (event: Event) => {
    const target = event.target instanceof Element
      ? event.target.closest(FULLSCREEN_CONTROL_SELECTOR)
      : null;
    if (!target) {
      return;
    }

    const now = Date.now();
    if (now - lastHandledAt < 240) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation?.();
      return;
    }
    lastHandledAt = now;

    if (isAndroidRuntime) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation?.();
      const shouldEnterFullscreen = !Boolean(player.isCssfullScreen || isInWebFullscreen.value || isInNativePlayerFullscreen.value);
      window.setTimeout(() => {
        togglePlayerFullscreen(shouldEnterFullscreen);
      }, 0);
      return;
    }

    const wasFullscreen = Boolean(player.isCssfullScreen || isInWebFullscreen.value);
    window.setTimeout(() => {
      const isFullscreenNow = Boolean(player.isCssfullScreen || isInWebFullscreen.value);
      if (!wasFullscreen && !isFullscreenNow) {
        player.getCssFullscreen?.();
      } else if (wasFullscreen && isFullscreenNow) {
        player.exitCssFullscreen?.();
      }
    }, 0);
  };

  root.addEventListener('click', handleFullscreenIntent, true);
  root.addEventListener('pointerup', handleFullscreenIntent, true);
  root.addEventListener('touchend', handleFullscreenIntent, true);
  fullscreenControlCleanup = () => {
    root.removeEventListener('click', handleFullscreenIntent, true);
    root.removeEventListener('pointerup', handleFullscreenIntent, true);
    root.removeEventListener('touchend', handleFullscreenIntent, true);
  };
}

function destroyPlayerInstance(options?: { preserveFullscreen?: boolean }) {
  const preserveFullscreen = options?.preserveFullscreen === true;
  if (preserveFullscreen) {
    pendingFullscreenRestore = true;
    preserveFullscreenDuringReload = true;
    setNativeOrientation(resolveVideoOrientation());
    try {
      document.documentElement.classList.add('web-fs-active');
    } catch (error) {
      console.warn('Diagnostic: index.vue:634 (details omitted)');
    }
  }
  if (fullscreenControlCleanup) {
    fullscreenControlCleanup();
    fullscreenControlCleanup = null;
  }
  videoOrientationCleanup?.();
  videoOrientationCleanup = null;
  const player = playerInstance.value;
  if (player) {
    try {
      player.destroy();
    } catch (error) {
      console.error('Diagnostic: index.vue:648 (details omitted)');
    }
    const overlayHost = player.root?.querySelector('.player-danmu-overlay') as HTMLElement | null;
    overlayHost?.remove();
  }
  playerInstance.value = null;

  const danmu = danmuInstance.value;
  if (danmu) {
    try {
      danmu.stop?.();
    } catch (error) {
      console.error('Diagnostic: index.vue:660 (details omitted)');
    }
    danmuInstance.value = null;
  }

  refreshControlPlugin.value = null;
  qualityControlPlugin.value = null;
  lineControlPlugin.value = null;
  danmuTogglePlugin.value = null;
  danmuSettingsPlugin.value = null;
  volumeControlPlugin.value = null;

  if (preserveFullscreen) {
    return;
  }

  pendingFullscreenRestore = false;
  preserveFullscreenDuringReload = false;
  resetFullscreenState();
}

const beginPlayerInitRun = () => {
  playerInitRunId += 1;
  return playerInitRunId;
};

const isActivePlayerInitRun = (runId: number) => runId === playerInitRunId && !isClosing.value;

async function mountXgPlayer(
  streamUrl: string,
  platformCode: StreamingPlatform,
  roomId: string,
  streamType?: string | null,
  initRunId?: number,
) {
  await nextTick();

  if (typeof initRunId === 'number' && !isActivePlayerInitRun(initRunId)) {
    return;
  }

  if (!playerContainerRef.value) {
    streamError.value = '播放器容器初始化失败。';
    return;
  }

  playerContainerRef.value.innerHTML = '';

  const playbackType = streamType === 'hls' ? 'hls' : 'flv';
  const isHlsPlayback = playbackType === 'hls';

  const playerOptions: Record<string, any> = {
    el: playerContainerRef.value,
    url: streamUrl,
    isLive: true,
    autoplay: true,
    playsinline: true,
    lang: 'zh-cn',
    width: '100%',
    height: '100%',
    videoFillMode: 'contain',
    closeVideoClick: true,
    closeVideoTouch: true,
    keyShortcut: true,
    volume: false as unknown as number,
    pip: {
      position: POSITIONS.CONTROLS_RIGHT,
      index: 3,
      showIcon: true,
    },
    cssFullscreen: {
      index: 2,
    },
    playbackRate: false,
    controls: {
      mode: 'normal',
    },
    icons: {
      play: ICONS.play,
      pause: ICONS.pause,
      fullscreen: ICONS.maximize2,
      exitFullscreen: ICONS.minimize2,
      cssFullscreen: ICONS.fullscreen,
      exitCssFullscreen: ICONS.minimize2,
      pipIcon: ICONS.pictureInPicture2,
      pipIconExit: ICONS.pictureInPicture2,
    },
  };

  if (isHlsPlayback) {
    playerOptions.plugins = [HlsPlugin];
    playerOptions.useHlsPlugin = true;
    playerOptions.hls = {
      isLive: true,
      retryCount: 3,
      retryDelay: 2000,
      enableWorker: !isAndroidRuntime,
      withCredentials: false,
      lowLatencyMode: false,
      fetchOptions: {
        credentials: 'omit',
        mode: 'cors',
        cache: 'no-store',
      } satisfies RequestInit,
      xhrSetup: (xhr: XMLHttpRequest) => {
        try {
          xhr.withCredentials = false;
        } catch (headerError) {
          console.warn('Diagnostic: index.vue:768 (details omitted)');
        }
      },
    };
  } else {
    playerOptions.plugins = [FlvPlugin];
    playerOptions.flv = {
      isLive: true,
      cors: true,
      autoCleanupSourceBuffer: true,
      enableWorker: !isAndroidRuntime,
      stashInitialSize: 128,
      lazyLoad: !isAndroidRuntime,
      lazyLoadMaxDuration: 30,
      deferLoadAfterSourceOpen: !isAndroidRuntime,
    };
  }

  const player = new Player(playerOptions);
  bindFullscreenControlFallback(player as Player & {
    getCssFullscreen?: () => void;
    exitCssFullscreen?: () => void;
    isCssfullScreen?: boolean;
  });

  if (typeof initRunId === 'number' && !isActivePlayerInitRun(initRunId)) {
    try {
      player.destroy();
    } catch (error) {
      console.error('Diagnostic: index.vue:797 (details omitted)');
    }
    return;
  }

  playerInstance.value = player;
  bindVideoOrientation(player);
  const storedPlayerVolume = loadStoredVolume();
  if (storedPlayerVolume !== null) {
    player.volume = storedPlayerVolume;
    player.muted = storedPlayerVolume === 0 ? true : player.muted;
  }

  const lineOptionsForPlatform = lineOptions.value.map((option) => ({ ...option }));

  refreshControlPlugin.value = player.registerPlugin(RefreshControl, {
    position: POSITIONS.CONTROLS_LEFT,
    index: 2,
    onClick: () => {
      void reloadCurrentStream('refresh');
    },
  }) as RefreshControl;

  volumeControlPlugin.value = player.registerPlugin(VolumeControl, {
    position: POSITIONS.CONTROLS_LEFT,
    index: 3,
  }) as VolumeControl;

  danmuTogglePlugin.value = player.registerPlugin(DanmuToggleControl, {
    position: POSITIONS.CONTROLS_RIGHT,
    index: 4,
    getState: () => isDanmuEnabled.value,
    onToggle: (enabled: boolean) => {
      isDanmuEnabled.value = enabled;
    },
  }) as DanmuToggleControl;

  danmuSettingsPlugin.value = player.registerPlugin(DanmuSettingsControl, {
    position: POSITIONS.CONTROLS_RIGHT,
    index: 4.2,
    getSettings: () => ({
      fontSize: danmuSettings.fontSize,
      duration: danmuSettings.duration,
      area: danmuSettings.area,
      mode: danmuSettings.mode,
      opacity: danmuSettings.opacity,
      density: danmuSettings.density,
    }),
    onChange: (partial: Partial<DanmuUserSettings>) => {
      if (partial.fontSize) {
        danmuSettings.fontSize = partial.fontSize;
      }
      if (typeof partial.duration === 'number') {
        danmuSettings.duration = partial.duration;
      }
      if (typeof partial.area === 'number') {
        danmuSettings.area = sanitizeDanmuArea(partial.area);
      }
      if (partial.mode) {
        danmuSettings.mode = partial.mode;
      }
      if (typeof partial.opacity === 'number') {
        danmuSettings.opacity = sanitizeDanmuOpacity(partial.opacity);
      }
      if (partial.density) {
        danmuSettings.density = partial.density;
      }
    },
  }) as DanmuSettingsControl;

  qualityControlPlugin.value = player.registerPlugin(QualityControl, {
    position: POSITIONS.CONTROLS_RIGHT,
    index: 5,
    disable: !supportsQualityForPlatform(platformCode),
    options: [...qualityOptions.value],
    getCurrent: () => currentQuality.value,
    onSelect: async (option: string) => {
      if (option === currentQuality.value) {
        return;
      }
      await switchQuality(option);
    },
  }) as QualityControl;
  qualityControlPlugin.value?.setOptions([...qualityOptions.value]);
  qualityControlPlugin.value?.updateLabel(currentQuality.value);

  lineControlPlugin.value = player.registerPlugin(LineControl, {
    position: POSITIONS.CONTROLS_RIGHT,
    index: 5.2,
    disable: lineOptionsForPlatform.length === 0,
    options: lineOptionsForPlatform,
    getCurrentKey: () => currentLine.value ?? '',
    getCurrentLabel: () => getCurrentLineLabel(currentLine.value),
    onSelect: async (optionKey: string) => {
      if (optionKey === currentLine.value) {
        return;
      }
      await switchLine(optionKey);
    },
  }) as LineControl;
  lineControlPlugin.value?.setOptions(lineOptionsForPlatform);
  lineControlPlugin.value?.updateLabel(getCurrentLineLabel(currentLine.value));


  arrangeControlClusters(player);

  let overlayInstance = createDanmuOverlay(player, danmuSettings, isDanmuEnabled.value);
  danmuInstance.value = overlayInstance;
  let danmakuBootstrapTimer: number | null = null;
  let danmakuBootstrapTriggered = false;

  const ensureDanmakuListenerStarted = async () => {
    if (danmakuBootstrapTriggered || isDanmakuListenerActive.value) {
        return;
      }
      if (!roomId || !supportsDanmuForPlatform(platformCode)) {
        return;
      }
      danmakuBootstrapTriggered = true;
    try {
      await startCurrentDanmakuListener(platformCode, roomId, overlayInstance);
    } catch (error) {
      danmakuBootstrapTriggered = false;
      throw error;
    }
  };

  player.on('ready', async () => {
    bindFullscreenControlFallback(player as Player & {
      getCssFullscreen?: () => void;
      exitCssFullscreen?: () => void;
      isCssfullScreen?: boolean;
    });
    arrangeControlClusters(player);
    ensureDanmuOverlayHost(player);
    overlayInstance = overlayInstance ?? createDanmuOverlay(player, danmuSettings, isDanmuEnabled.value);
    danmuInstance.value = overlayInstance;
    try {
      if (danmakuBootstrapTimer !== null) {
        window.clearTimeout(danmakuBootstrapTimer);
        danmakuBootstrapTimer = null;
      }
      await ensureDanmakuListenerStarted();
    } catch (error) {
      console.error('Diagnostic: index.vue:941 (details omitted)');
    }
    overlayInstance?.play?.();
    if (pendingFullscreenRestore) {
      pendingFullscreenRestore = false;
      window.setTimeout(() => {
        togglePlayerFullscreen(true);
      }, 120);
      window.setTimeout(() => {
        if (!isInWebFullscreen.value && !isInNativePlayerFullscreen.value) {
          togglePlayerFullscreen(true);
        }
      }, 360);
    }
    updateFullscreenFlag();
  });

  player.on('play', () => {
    overlayInstance?.play?.();
  });

  player.on('pause', () => {
    overlayInstance?.pause?.();
  });

  player.on('destroy', () => {
    if (danmakuBootstrapTimer !== null) {
      window.clearTimeout(danmakuBootstrapTimer);
      danmakuBootstrapTimer = null;
    }
    overlayInstance?.stop?.();
    overlayInstance = null;
    danmuInstance.value = null;
  });

  danmakuBootstrapTimer = window.setTimeout(() => {
    void ensureDanmakuListenerStarted().catch(() => {
      console.error('Diagnostic: index.vue:978 (details omitted)');
    });
  }, 1500);

  player.on('ended', () => {
    void stopDouyuProxy();
    availableLines.value = [];
  });
  player.on('error', () => {
    console.error('Diagnostic: index.vue:983 (details omitted)');
    streamError.value = '播放中断，请稍后手动重试。';
    void stopDouyuProxy();
  });

  player.on('enterFullscreen', () => {
    preserveFullscreenDuringReload = false;
    isInNativePlayerFullscreen.value = true;
    ensureDanmuOverlayHost(player);
    overlayInstance = overlayInstance ?? createDanmuOverlay(player, danmuSettings, isDanmuEnabled.value);
    danmuInstance.value = overlayInstance;
    overlayInstance?.play?.();
    updateFullscreenFlag();
  });

  player.on('exitFullscreen', () => {
    isInNativePlayerFullscreen.value = false;
    ensureDanmuOverlayHost(player);
    overlayInstance = overlayInstance ?? createDanmuOverlay(player, danmuSettings, isDanmuEnabled.value);
    danmuInstance.value = overlayInstance;
    updateFullscreenFlag();
  });

  player.on('enterFullscreenWeb', () => {
    preserveFullscreenDuringReload = false;
    isInWebFullscreen.value = true;
    try {
      document.documentElement.classList.add('web-fs-active');
    } catch (error) {
      console.warn('Diagnostic: index.vue:1011 (details omitted)');
    }
    ensureDanmuOverlayHost(player);
    overlayInstance = overlayInstance ?? createDanmuOverlay(player, danmuSettings, isDanmuEnabled.value);
    danmuInstance.value = overlayInstance;
    overlayInstance?.play?.();
    arrangeControlClusters(player);
    updateFullscreenFlag();
  });

  player.on('exitFullscreenWeb', () => {
    isInWebFullscreen.value = false;
    try {
      if (!isClosing.value && !preserveFullscreenDuringReload) {
        document.documentElement.classList.remove('web-fs-active');
      }
    } catch (error) {
      console.warn('Diagnostic: index.vue:1028 (details omitted)');
    }
    ensureDanmuOverlayHost(player);
    overlayInstance = overlayInstance ?? createDanmuOverlay(player, danmuSettings, isDanmuEnabled.value);
    danmuInstance.value = overlayInstance;
    arrangeControlClusters(player);
    updateFullscreenFlag();
  });

  player.on('cssFullscreen_change', (isCssFullscreen: boolean) => {
    if (isCssFullscreen) {
      preserveFullscreenDuringReload = false;
    }
    isInWebFullscreen.value = isCssFullscreen;
    try {
      if (isCssFullscreen) {
        document.documentElement.classList.add('web-fs-active');
      } else if (!isClosing.value && !preserveFullscreenDuringReload) {
        document.documentElement.classList.remove('web-fs-active');
      }
    } catch (error) {
      console.warn('Diagnostic: index.vue:1049 (details omitted)');
    }
    ensureDanmuOverlayHost(player);
    overlayInstance = overlayInstance ?? createDanmuOverlay(player, danmuSettings, isDanmuEnabled.value);
    danmuInstance.value = overlayInstance;
    if (isCssFullscreen) {
      overlayInstance?.play?.();
    }
    arrangeControlClusters(player);
    updateFullscreenFlag();
  });
}

function handleClosePlayerClick() {
  isClosing.value = true;
  emit('close-player');
}


async function initializePlayerAndStream(
  pRoomId: string, 
  pPlatform: StreamingPlatform,
  _pStreamUrlProp?: string | null, 
  isRefresh: boolean = false,
  oldRoomIdForCleanup?: string | null,
  oldPlatformForCleanup?: StreamingPlatform | null
): Promise<boolean> {
  const initRunId = beginPlayerInitRun();
  isLoadingStream.value = true;
  streamError.value = null;
  isOfflineError.value = false;

  // Detect OS and adjust danmu font family per platform
  osName.value = await applyDanmuFontFamilyForOS();
  if (!isActivePlayerInitRun(initRunId)) {
    return false;
  }

  if (!isRefresh) {
    danmakuMessages.value = [];
  }

  if (oldRoomIdForCleanup && oldPlatformForCleanup !== undefined && oldPlatformForCleanup !== null) {
    await stopCurrentDanmakuListener(oldPlatformForCleanup, oldRoomIdForCleanup);
  } else {
    await stopCurrentDanmakuListener(pPlatform, pRoomId);
  }
  await stopDouyuProxy();

  if (!isActivePlayerInitRun(initRunId)) {
    return false;
  }

  destroyPlayerInstance({
    preserveFullscreen: isRefresh && (isFullScreen.value || isInWebFullscreen.value || isInNativePlayerFullscreen.value),
  });

  if (isOfflineStreamError(props.initialError)) {
    streamError.value = props.initialError ?? null;
    isOfflineError.value = true;
    playerTitle.value = props.title;
    playerAnchorName.value = props.anchorName;
    playerAvatar.value = props.avatar;
    playerIsLive.value = false;
    destroyPlayerInstance();
    isLoadingStream.value = false;
    return false;
  }

  const effectiveLine = currentLine.value;

  try {
    let streamConfig: PlaybackConfig;

    if (pPlatform === StreamingPlatform.DOUYU) {
      if (playerIsLive.value === false) {
        streamError.value = streamError.value || '主播未开播。';
        isOfflineError.value = true;
        isLoadingStream.value = false;
        return false;
      }
      streamConfig = await getDouyuStreamConfig(pRoomId, currentQuality.value, effectiveLine);
    } else if (pPlatform === StreamingPlatform.DOUYIN) {
      const douyinConfig = await fetchAndPrepareDouyinStreamConfig(pRoomId, currentQuality.value, effectiveLine);
      if (!isActivePlayerInitRun(initRunId)) {
        return false;
      }
      playerTitle.value = douyinConfig.title;
      playerAnchorName.value = douyinConfig.anchorName;
      playerAvatar.value = douyinConfig.avatar;
      playerIsLive.value = douyinConfig.isLive;

      if (douyinConfig.initialError || !douyinConfig.isLive || !douyinConfig.streamUrl) {
        streamError.value = douyinConfig.initialError || '主播未开播或无法获取直播流。';
        isOfflineError.value = isOfflineStreamError(streamError.value);
        playerIsLive.value = false;
        isLoadingStream.value = false;
        console.warn('Diagnostic: index.vue:1146 (details omitted)');
        return false;
      }

      streamConfig = douyinConfig;
    } else if (pPlatform === StreamingPlatform.HUYA) {
      streamConfig = await getHuyaStreamConfig(pRoomId, currentQuality.value, effectiveLine);
    } else if (pPlatform === StreamingPlatform.BILIBILI) {
      streamConfig = await getBilibiliStreamConfig(pRoomId, currentQuality.value, props.cookie || undefined, effectiveLine);
    } else {
      throw new Error(`涓嶆敮鎸佺殑骞冲彴: ${pPlatform}`);
    }

    if (!isActivePlayerInitRun(initRunId)) return false;

    const proxyUrl = await queueStreamProxy(async () => {
      if (!isActivePlayerInitRun(initRunId)) return null;
      await invoke('set_stream_url_cmd', {
        url: streamConfig.streamUrl, headers: streamConfig.headers ?? {}, format: streamConfig.streamType,
      });
      if (!isActivePlayerInitRun(initRunId)) return null;
      return invoke<string>('start_proxy');
    });
    if (!proxyUrl || !isActivePlayerInitRun(initRunId)) return false;
    qualityOptions.value = streamConfig.qualities ?? [];
    availableLines.value = streamConfig.lines ?? [];
    currentQuality.value = streamConfig.selectedQuality ?? currentQuality.value;
    currentLine.value = streamConfig.selectedLine ?? null;
    isLoadingStream.value = false;
    await mountXgPlayer(proxyUrl, pPlatform, pRoomId, streamConfig.streamType, initRunId);
    if (!isActivePlayerInitRun(initRunId)) return false;
    return true;
  } catch (error: any) {
    if (!isActivePlayerInitRun(initRunId)) return false;
    console.error('播放初始化失败', diagnosticDetails(error));
    destroyPlayerInstance();

    const errorMessage = error instanceof Error ? error.message
      : typeof error === 'string' && error.trim() ? error : '加载直播流失败，请稍后再试。';

    if (isOfflineStreamError(errorMessage)) {
      streamError.value = errorMessage;
      isOfflineError.value = true;

      try {
        if (pPlatform === StreamingPlatform.HUYA) {
          const result: any = await invoke('get_huya_unified_cmd', { roomId: pRoomId, quality: currentQuality.value, line: effectiveLine ?? null });
          await ensureProxyStarted();
          playerTitle.value = result?.title ?? props.title;
          playerAnchorName.value = result?.nick ?? props.anchorName;
          playerAvatar.value = proxify((result?.avatar ?? props.avatar ?? '') as string);
        } else if (pPlatform === StreamingPlatform.BILIBILI) {
          const payload = { args: { room_id_str: pRoomId } };
          const savedCookie = (typeof localStorage !== 'undefined') ? (localStorage.getItem('bilibili_cookie') || null) : null;
          const res: any = await invoke('fetch_bilibili_streamer_info', { payload, cookie: savedCookie });
          await ensureProxyStarted();
          playerTitle.value = res?.title ?? props.title;
          playerAnchorName.value = res?.anchor_name ?? props.anchorName;
          playerAvatar.value = proxify((res?.avatar ?? props.avatar ?? '') as string);
        }
      } catch (infoError) {
        console.warn('离线房间信息加载失败', diagnosticDetails(infoError));
      }
    } else {
      streamError.value = errorMessage;
      isOfflineError.value = false;
    }

    isLoadingStream.value = false;
    return false;
  }
}
const danmakuManagerContext = {
  danmakuMessages,
  isDanmuEnabled,
  danmuSettings,
  isFullScreen,
  isDanmakuListenerActive,
  unlistenDanmakuFn,
  props,
};

const startCurrentDanmakuListener = async (platform: StreamingPlatform, roomId: string, danmuOverlay: DanmuOverlayInstance | null) => {
  await startDanmakuListener(danmakuManagerContext, platform, roomId, () => danmuInstance.value ?? danmuOverlay);
};

const stopCurrentDanmakuListener = async (platform?: StreamingPlatform, roomId?: string | null | undefined) => {
  await stopDanmakuListener(danmakuManagerContext, platform, roomId);
};

const retryInitialization = async () => {
  await reloadCurrentStream('refresh');
};

const isOfflineStreamError = (message: string | null | undefined) => {
  const text = String(message || '');
  return text.includes('主播未开播') || isDouyuOfflineMessage(text);
};

// 画质切换函数
const switchQuality = async (quality: string) => {
  if (isQualitySwitching.value) {
    return;
  }
  if (!supportsQuality.value) {
    return;
  }
  if (!qualityOptions.value.includes(quality)) {
    return;
  }
  if (!props.roomId || props.platform == null) {
    emit('request-player-reload');
    return;
  }
  if (quality === currentQuality.value) {
    return;
  }

  isQualitySwitching.value = true;
  const previousQuality = currentQuality.value;

  try {
    currentQuality.value = quality;
    const reloaded = await reloadCurrentStream('quality');
    if (!reloaded) {
      throw new Error(`Failed to reload stream for quality ${quality}`);
    }
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(`${props.platform}_preferred_quality`, quality);
    }
    console.log('Diagnostic: index.vue:1276 (details omitted)');
  } catch (error) {
    console.error('Diagnostic: index.vue:1278 (details omitted)');
    currentQuality.value = previousQuality;
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(`${props.platform}_preferred_quality`, previousQuality);
    }
  } finally {
    isQualitySwitching.value = false;
  }
};

const switchLine = async (lineKey: string) => {
  if (isLineSwitching.value) {
    return;
  }
  const options = lineOptions.value;
  if (!options.length) {
    return;
  }
  if (!options.some((option) => option.key === lineKey)) {
    return;
  }
  if (!props.roomId || props.platform == null) {
    emit('request-player-reload');
    return;
  }
  if (currentLine.value === lineKey) {
    return;
  }

  isLineSwitching.value = true;
  const previousLine = currentLine.value;

  try {
    currentLine.value = lineKey;
    const reloaded = await reloadCurrentStream('line');
    if (!reloaded) {
      throw new Error(`Failed to reload stream for line ${lineKey}`);
    }
    persistLinePreference(props.platform, lineKey);
    console.log('Diagnostic: index.vue:1317 (details omitted)');
  } catch (error) {
    console.error('Diagnostic: index.vue:1319 (details omitted)');
    currentLine.value = previousLine ?? null;
    if (typeof window !== 'undefined' && props.platform) {
      if (previousLine) {
        persistLinePreference(props.platform, previousLine);
      } else {
        window.localStorage.removeItem(`${props.platform}_preferred_line`);
      }
    } else if (previousLine) {
      persistLinePreference(props.platform, previousLine);
    }
  } finally {
    isLineSwitching.value = false;
  }
};

// 初始化画质偏好
const initializeQualityPreference = () => {
  currentQuality.value = resolveStoredQuality(props.platform);
};

async function reloadCurrentStream(trigger: 'refresh' | 'quality' | 'line' = 'refresh'): Promise<boolean> {
  if (isLoadingStream.value) {
    return false;
  }
  if (!props.roomId || props.platform == null) {
    emit('request-player-reload');
    return false;
  }
  const isRefreshAction = trigger === 'refresh';
  let reloaded = false;
  if (isRefreshAction) {
    isRefreshingStream.value = true;
  }
  try {
    reloaded = await initializePlayerAndStream(
      props.roomId,
      props.platform,
      props.streamUrl ?? null,
      true,
      props.roomId,
      props.platform,
    );
  } finally {
    if (isRefreshAction) {
      isRefreshingStream.value = false;
    }
  }
  if (trigger === 'quality') {
    qualityControlPlugin.value?.updateLabel(currentQuality.value);
  }
  if (trigger === 'line') {
    lineControlPlugin.value?.updateLabel(getCurrentLineLabel(currentLine.value));
  }
  return reloaded;
}

const getDanmuSettingsSnapshot = (): DanmuUserSettings => ({
  fontSize: danmuSettings.fontSize,
  duration: danmuSettings.duration,
  area: sanitizeDanmuArea(danmuSettings.area),
  mode: danmuSettings.mode,
  opacity: sanitizeDanmuOpacity(danmuSettings.opacity),
  density: sanitizeDanmuDensity(danmuSettings.density),
});

const persistCurrentDanmuPreferences = () => {
  persistDanmuPreferences({
    enabled: isDanmuEnabled.value,
    settings: getDanmuSettingsSnapshot(),
  });
};

registerPlayerWatchers({
  refreshControlPlugin,
  isRefreshingStream,
  qualityControlPlugin,
  isQualitySwitching,
  lineControlPlugin,
  isLineSwitching,
  lineOptions,
  currentLine,
  getLineLabel: getCurrentLineLabel,
  props,
  resolveStoredLine,
  isDanmuEnabled,
  danmuTogglePlugin,
  danmuInstance,
  danmuSettingsPlugin,
  danmuSettings,
  applyDanmuOverlayPreferences,
  syncDanmuEnabledState,
  persistCurrentDanmuPreferences,
  currentQuality,
  initializeQualityPreference,
  initializePlayerAndStream,
  stopCurrentDanmakuListener,
  stopDouyuProxy,
  destroyPlayerInstance,
  isLoadingStream,
  danmakuMessages,
  streamError,
  isOfflineError,
  playerTitle,
  playerAnchorName,
  playerAvatar,
  playerIsLive,
  playerRoot: () => playerInstance.value?.root as HTMLElement | null,
});


const handleFullscreenPopState = () => {
  if (ignoreNextFullscreenPop) {
    ignoreNextFullscreenPop = false;
    return;
  }
  if (!fullscreenHistoryActive) {
    return;
  }
  fullscreenHistoryActive = false;
  if (isFullScreen.value || isInWebFullscreen.value || isInNativePlayerFullscreen.value) {
    exitPlayerFullscreenFromSystemBack();
  } else {
    exitNativeFullscreen();
  }
};

if (typeof window !== 'undefined') {
  const dtvWindow = window as DtvWindow;
  dtvWindow.__DTV_HANDLE_ANDROID_BACK__ = () => {
    if (!isFullScreen.value && !isInWebFullscreen.value && !isInNativePlayerFullscreen.value) {
      return false;
    }
    exitPlayerFullscreenFromSystemBack();
    return true;
  };
}

onMounted(async () => {
  if (typeof window !== 'undefined') {
    window.addEventListener('popstate', handleFullscreenPopState);
  }
  // 初始化画质偏好
  initializeQualityPreference();
  
  if (!props.roomId || props.platform == null) {
    if (props.initialError) {
      if (isOfflineStreamError(props.initialError)) {
          streamError.value = props.initialError ?? null;
          isOfflineError.value = true;
      } else {
          streamError.value = props.initialError ?? null;
          isOfflineError.value = false; // Ensure it's not marked as offline for other errors
      }
    }
    isLoadingStream.value = false;
  }

  persistCurrentDanmuPreferences();

});

onUnmounted(async () => {
  beginPlayerInitRun();
  if (typeof window !== 'undefined') {
    window.removeEventListener('popstate', handleFullscreenPopState);
    const dtvWindow = window as DtvWindow;
    delete dtvWindow.__DTV_HANDLE_ANDROID_BACK__;
  }
  const platformToStop: StreamingPlatform = props.platform;
  const roomIdToStop: string | null = props.roomId;
  const proxyCleanup = stopDouyuProxy();
  await stopCurrentDanmakuListener(platformToStop, roomIdToStop);
  await proxyCleanup;

  destroyPlayerInstance();
  danmakuMessages.value = []; 
});

</script>
