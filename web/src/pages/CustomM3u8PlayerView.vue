<template>
  <div class="player-view">
    <MainPlayer
      v-if="entry"
      :key="entry.id"
      :room-id="entry.id"
      :platform="Platform.CUSTOM_M3U8"
      :stream-url="entry.url"
      :title="entry.title"
      :anchor-name="entry.group || '自定义 M3U8'"
      :avatar="entry.cover || null"
      :is-followed="false"
      @close-player="handleClosePlayer"
      @fullscreen-change="handlePlayerFullscreenChange"
    />
    <div v-else class="missing-state">源不存在或已删除。</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import MainPlayer from '../components/player/index.vue';
import { Platform } from '../platforms/common/types';
import { useCustomM3u8Store } from '../store/customM3u8Store';

const props = defineProps<{ encodedId: string }>();
const emit = defineEmits(['fullscreen-change']);
const router = useRouter();
const store = useCustomM3u8Store();
store.ensureLoaded();

const entry = computed(() => store.getById(decodeURIComponent(props.encodedId)));

const handleClosePlayer = () => router.replace('/custom-m3u8');
const handlePlayerFullscreenChange = (isFullscreen: boolean) => emit('fullscreen-change', isFullscreen);
</script>

<style scoped>
.player-view { display: flex; flex: 1 1 auto; flex-direction: column; min-height: 0; }
.missing-state { padding: 24px; color: var(--text-secondary); }
</style>
