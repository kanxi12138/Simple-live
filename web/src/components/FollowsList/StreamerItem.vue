<template>
  <div class="streamer-item-content" :class="{ big: big }">
    <div class="item-content" @click="onClick">
      <div ref="avatarRef" class="avatar-container" :class="{ big: big }">
        <img 
          v-if="shouldLoadAvatar"
          :src="avatarSrc"
          :alt="streamer.nickname"
          loading="lazy"
          decoding="async"
          fetchpriority="low"
          @error="handleImgError($event, streamer)"
          class="avatar-image"
        >
        <div v-else-if="canLoadAvatar" class="avatar-placeholder" aria-hidden="true"></div>
        <div v-else class="avatar-fallback">{{ streamer.nickname[0] }}</div>
      </div>
      
      <div class="streamer-details">
        <div class="primary-row">
          <span class="nickname" :title="streamer.nickname">{{ streamer.nickname }}</span>
          <!-- 移除左侧平台名，改为右侧胶囊与状态点集成 -->
        </div>
        <div class="secondary-row" :title="streamer.roomTitle">
          {{ streamer.roomTitle || '暂无直播标题' }}
        </div>
      </div>
    </div>

    <div class="status-container">
      <div v-if="showPlatform" class="platform-badge">
        <span class="live-indicator" :class="getLiveIndicatorClass(streamer)"></span>
        <span class="badge-text">{{ platformLabel(streamer.platform) }}</span>
      </div>
      <div v-else class="live-indicator" :class="getLiveIndicatorClass(streamer)"></div>
    </div>
  </div>
</template>

<style scoped>
.streamer-item-content {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  width: 100%;
  user-select: none;
  transition: background-color 0.15s ease;
  border-radius: 0;
  position: relative;
  overflow: hidden;
  border: none;
  background: transparent;
  margin-bottom: 0;
}

.streamer-item-content:hover {
  background: transparent;
  border-color: transparent;
  transform: none;
  z-index: 1;
}

.item-content {
  display: flex;
  align-items: center;
  padding: 2px 2px;
  gap: 10px;
  flex: 1;
  min-width: 0;
  z-index: 2;
}

.avatar-container {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  position: relative;
  flex: 0 0 auto;
  transition: all 0.3s ease;
}

.avatar-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 50%;
  border: 1px solid var(--border-color);
  background: var(--tertiary-bg);
  transition: all 0.3s ease;
}

.avatar-container.is-live::before {
  content: '';
  position: absolute;
  inset: -3px;
  border-radius: 50%;
  background: var(--accent-gradient);
  padding: 2px;
  -webkit-mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;
  animation: rotate-border 3s linear infinite;
}

.avatar-container.is-live .avatar-image {
  border-color: #fff;
  box-shadow: 0 0 15px rgba(139, 92, 246, 0.4);
}

@keyframes rotate-border {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.avatar-fallback {
  font-size: 14px;
  font-weight: 700;
  color: var(--primary-text);
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--tertiary-bg);
  border-radius: 50%;
}

.avatar-placeholder {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: var(--tertiary-bg);
  border: 1px solid var(--border-color);
}

.streamer-details {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.nickname {
  font-weight: 700;
  color: rgba(29, 29, 31, 0.92);
  font-size: 12.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  letter-spacing: 0;
  transition: color 0.2s ease;
}

:root[data-theme="dark"] .nickname {
  color: rgba(229, 231, 235, 0.92);
}

.secondary-row {
  font-size: 11.5px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
  margin-top: 3px;
  opacity: 0.9;
  transition: color 0.2s ease, opacity 0.2s ease;
}

.streamer-item-content:hover .nickname {
  color: var(--text-primary);
}

.streamer-item-content:hover .secondary-row {
  color: rgba(29, 29, 31, 0.9);
  opacity: 1;
  text-shadow: 0 0 6px rgba(255, 255, 255, 0.35);
}

:root[data-theme="dark"] .streamer-item-content:hover .secondary-row {
  color: rgba(229, 231, 235, 0.95);
  text-shadow: 0 0 8px rgba(255, 255, 255, 0.25);
}

.status-container {
  display: flex;
  align-items: center;
  margin-left: auto;
  flex: 0 0 auto;
  padding-right: 6px;
  z-index: 2;
}

.platform-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  border-radius: 20px;
  padding: 3px 6px 3px 8px;
  background: var(--tertiary-bg);
  color: var(--primary-text);
  font-size: 10px;
  font-weight: 800;
  border: 1px solid var(--border-color);
}

.live-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--dim-text);
  transition: all 0.3s ease;
}

.live-indicator.is-live { 
  background: #10b981; 
}

.live-indicator.is-replay { background: #f59e0b; box-shadow: none; }
.live-indicator.is-offline { background: var(--border-color); }
:root[data-theme="light"] .live-indicator.is-offline { background: #9ca3af; }
</style>

<script setup lang="ts">
import { Platform } from '../../platforms/common/types';
import type { FollowedStreamer } from '../../platforms/common/types';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';

const props = defineProps<{
  streamer: FollowedStreamer,
  getAvatarSrc: (s: FollowedStreamer) => string,
  handleImgError: (ev: Event, s: FollowedStreamer) => void,
  getLiveIndicatorClass: (s: FollowedStreamer) => string,
  proxyBase?: string,
  big?: boolean,
  showPlatform?: boolean
}>();

const emit = defineEmits<{ (e: 'clickItem', s: FollowedStreamer): void }>();

const onClick = () => emit('clickItem', props.streamer);

const canLoadAvatar = computed(() => {
  return !!props.streamer.avatarUrl && (props.streamer.platform !== Platform.BILIBILI || !!props.proxyBase);
});

const isAvatarVisible = ref(false);
const avatarRef = ref<HTMLElement | null>(null);
let avatarObserver: IntersectionObserver | null = null;

const setupAvatarObserver = () => {
  if (!canLoadAvatar.value) {
    isAvatarVisible.value = false;
    return;
  }
  if (typeof window === 'undefined' || !('IntersectionObserver' in window)) {
    isAvatarVisible.value = true;
    return;
  }
  if (avatarObserver) {
    avatarObserver.disconnect();
    avatarObserver = null;
  }
  avatarObserver = new IntersectionObserver(
    (entries) => {
      const entry = entries[0];
      if (entry?.isIntersecting) {
        isAvatarVisible.value = true;
        avatarObserver?.disconnect();
        avatarObserver = null;
      }
    },
    { rootMargin: '200px' }
  );
  if (avatarRef.value) {
    avatarObserver.observe(avatarRef.value);
  } else {
    isAvatarVisible.value = true;
  }
};

onMounted(setupAvatarObserver);
watch(canLoadAvatar, setupAvatarObserver);
onUnmounted(() => avatarObserver?.disconnect());

const platformLabel = (p: Platform): string => {
  switch (p) {
    case Platform.DOUYU: return '斗鱼';
    case Platform.DOUYIN: return '抖音';
    case Platform.HUYA: return '虎牙';
    case Platform.BILIBILI: return 'B站';
    default: return '未知';
  }
};

const showPlatform = computed(() => !!props.showPlatform);

const shouldLoadAvatar = computed(() => canLoadAvatar.value && isAvatarVisible.value);
const avatarSrc = computed(() => (shouldLoadAvatar.value ? props.getAvatarSrc(props.streamer) : ''));
</script>
