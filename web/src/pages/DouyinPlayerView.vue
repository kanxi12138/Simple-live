<template>
  <div class="player-view">
    <MainPlayer
      :key="playerKey"
      :room-id="props.roomId"
      :platform="Platform.DOUYIN"
      :is-followed="isFollowed"
      @follow="handleFollow"
      @unfollow="handleUnfollow"
      @close-player="handleClosePlayer"
      @fullscreen-change="handlePlayerFullscreenChange"
      @request-player-reload="handlePlayerReload"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useRouter } from 'vue-router';
import MainPlayer from '../components/player/index.vue';
import { useFollowStore } from '../store/followStore';
import type { FollowedStreamer } from '../platforms/common/types';
import { Platform } from '../platforms/common/types';

const props = defineProps<{
  roomId: string;
}>();

const emit = defineEmits(['fullscreen-change']);

const router = useRouter();
const followStore = useFollowStore();
const playerKey = ref(0);

const isFollowed = computed(() => {
  return followStore.isFollowed(Platform.DOUYIN, props.roomId);
});

const handleFollow = () => {
  const idToSave = props.roomId;
  const streamerToFollow: Omit<FollowedStreamer, 'platform' | 'id' | 'roomTitle' | 'isLive'> = {
    nickname: `主播${idToSave}`,
    avatarUrl: '',
  };

  console.log('Diagnostic: DouyinPlayerView.vue:46 (details omitted)');

  followStore.followStreamer({
    ...streamerToFollow,
    id: idToSave,
    platform: Platform.DOUYIN,
  });

  console.log('Diagnostic: DouyinPlayerView.vue:60 (details omitted)');
};

const handleUnfollow = () => {
  followStore.unfollowStreamer(Platform.DOUYIN, props.roomId);
};

const handleClosePlayer = () => {
  console.log('Diagnostic: DouyinPlayerView.vue:68 (details omitted)');
  router.replace('/douyin');
};

const handlePlayerFullscreenChange = (isFullscreen: boolean) => {
  emit('fullscreen-change', isFullscreen);
};

const handlePlayerReload = () => {
  playerKey.value++;
};

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
  align-items: stretch;
}
</style>
