<template>
  <div class="player-view">
    <MainPlayer
      v-if="roomId && !isLoadingDetails"
      :key="playerKey"
      :room-id="roomId"
      :platform="Platform.DOUYU"
      :is-followed="isFollowed"
      :title="streamerDetails?.roomTitle ?? undefined"
      :anchor-name="streamerDetails?.nickname ?? undefined"
      :avatar="streamerDetails?.avatarUrl ?? undefined"
      :is-live="streamerDetails?.isLive ?? undefined"
      :initial-error="detailsError"
      @follow="handleFollow"
      @unfollow="handleUnfollow"
      @close-player="handleClosePlayer"
      @fullscreen-change="handlePlayerFullscreenChange"
      @request-refresh-details="handleRefreshDetails"
      @request-player-reload="handlePlayerReload"
    />
    <div v-else-if="roomId && isLoadingDetails" class="loading-details loading-player">
      <LoadingDots />
    </div>
    <div v-else-if="detailsError" class="invalid-room">
      <p>错误: {{ detailsError }}</p>
      <button @click="router.back()">返回</button>
    </div>
    <div v-else class="invalid-room">
      <p>无效的斗鱼房间 ID。</p>
      <button @click="router.back()">返回</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import MainPlayer from '../components/player/index.vue';
import LoadingDots from '../components/Common/LoadingDots.vue';
import { useFollowStore } from '../store/followStore';
import type { FollowedStreamer } from '../platforms/common/types';
import { Platform } from '../platforms/common/types';
import { fetchDouyuStreamerDetails } from '../platforms/douyu/streamerInfoParser';
import type { StreamerDetails } from '../platforms/common/types';

const props = defineProps<{
  roomId: string;
}>();

const emit = defineEmits(['fullscreen-change']);

const router = useRouter();
const followStore = useFollowStore();

const streamerDetails = ref<StreamerDetails | null>(null);
const isLoadingDetails = ref(false);
const detailsError = ref<string | null>(null);
const playerKey = ref(0);
let hasLoadedDetailsForCurrentRoom = false;

const loadStreamerDetails = async (currentRoomId: string) => {
  if (!currentRoomId) {
    streamerDetails.value = null;
    detailsError.value = 'Room ID is invalid.';
    isLoadingDetails.value = false;
    hasLoadedDetailsForCurrentRoom = false;
    return;
  }

  if (hasLoadedDetailsForCurrentRoom && streamerDetails.value?.roomId === currentRoomId) {
    if (isLoadingDetails.value) {
      isLoadingDetails.value = false;
    }
    return;
  }

  isLoadingDetails.value = true;
  detailsError.value = null;
  if (streamerDetails.value?.roomId !== currentRoomId) {
    streamerDetails.value = null;
  }

  try {
    const result = await fetchDouyuStreamerDetails(currentRoomId);
    if (result?.errorMessage) {
      detailsError.value = result.errorMessage;
      streamerDetails.value = null;
      console.warn(`[DouyuPlayerView] Error from fetchDouyuStreamerDetails: ${result.errorMessage}`);
    } else if (!result || !result.nickname) {
      detailsError.value = '获取到的主播信息无效或不完整。';
      streamerDetails.value = null;
      console.warn('[DouyuPlayerView] Invalid or incomplete data from backend.', result);
    } else {
      streamerDetails.value = result;
      detailsError.value = null;
    }
  } catch (error: unknown) {
    console.error(`[DouyuPlayerView] Exception while loading streamer details for ${currentRoomId}:`, error);
    detailsError.value = error instanceof Error ? error.message : '加载主播详情时发生未知错误。';
    streamerDetails.value = null;
  } finally {
    isLoadingDetails.value = false;
    hasLoadedDetailsForCurrentRoom = true;
  }
};

const isFollowed = computed(() => followStore.isFollowed(Platform.DOUYU, props.roomId));

interface MainPlayerFollowEventData {
  nickname: string;
  avatarUrl: string;
  roomTitle?: string;
}

const handleFollow = (streamerDataFromPlayer: MainPlayerFollowEventData) => {
  const streamerToFollow: FollowedStreamer = {
    id: props.roomId,
    platform: Platform.DOUYU,
    nickname: streamerDataFromPlayer.nickname,
    avatarUrl: streamerDataFromPlayer.avatarUrl,
    roomTitle: streamerDataFromPlayer.roomTitle,
  };
  followStore.followStreamer(streamerToFollow);
};

const handleUnfollow = () => {
  followStore.unfollowStreamer(Platform.DOUYU, props.roomId);
};

const handleClosePlayer = () => {
  console.log('[DouyuPlayerView] Close button clicked. Navigating to Douyu home.');
  router.replace('/');
};

const handlePlayerFullscreenChange = (isFullscreen: boolean) => {
  emit('fullscreen-change', isFullscreen);
};

const handleRefreshDetails = () => {
  if (props.roomId) {
    hasLoadedDetailsForCurrentRoom = false;
    streamerDetails.value = null;
    detailsError.value = null;
    void loadStreamerDetails(props.roomId);
    return;
  }
  console.warn('[DouyuPlayerView] request-refresh-details received but no roomId available.');
};

const handlePlayerReload = () => {
  playerKey.value += 1;
  if (props.roomId) {
    hasLoadedDetailsForCurrentRoom = false;
    streamerDetails.value = null;
    detailsError.value = null;
    void loadStreamerDetails(props.roomId);
  }
};

watch(
  () => props.roomId,
  (newRoomId, oldRoomId) => {
    if (newRoomId) {
      if (newRoomId !== oldRoomId) {
        hasLoadedDetailsForCurrentRoom = false;
        void loadStreamerDetails(newRoomId);
      } else if (!hasLoadedDetailsForCurrentRoom) {
        void loadStreamerDetails(newRoomId);
      }
      return;
    }

    streamerDetails.value = null;
    detailsError.value = null;
    isLoadingDetails.value = false;
    hasLoadedDetailsForCurrentRoom = false;
  },
  { immediate: true },
);

onMounted(() => {
  if (props.roomId && hasLoadedDetailsForCurrentRoom && isLoadingDetails.value) {
    isLoadingDetails.value = false;
  } else if (!props.roomId && isLoadingDetails.value) {
    isLoadingDetails.value = false;
  }
});
</script>

<style scoped>
.player-view {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  width: 100%;
  background-color: transparent;
  color: white;
  justify-content: center;
  align-items: stretch;
  text-align: center;
}

.invalid-room p {
  padding: 10px 20px;
  font-size: 1.1em;
}

.invalid-room button {
  padding: 10px 20px;
  font-size: 1em;
  cursor: pointer;
  background-color: #5c16c5;
  color: white;
  border: none;
  border-radius: 5px;
  margin-top: 15px;
}

:root[data-theme='light'] .player-view {
  background-color: transparent;
  color: var(--main-text-primary-light, #212529);
}

:root[data-theme='light'] .invalid-room p {
  color: var(--main-text-primary-light, #212529);
}

:root[data-theme='light'] .invalid-room button {
  background-color: var(--primary-color-light, #007bff);
  color: white;
}
</style>
