<template>
  <motion.div
    class="folder-item"
    :data-folder-id="folder.id"
    :class="{ 
      'is-dragging': isDragging, 
      'is-expanded': folder.expanded,
      'is-drag-over': isDragOver,
      'can-accept-drop': canAcceptDrop
    }"
    @mousedown="handleHeaderMouseDown"
    @mouseup="handleHeaderMouseUp"
    @contextmenu.prevent="handleContextMenu"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
  >
    <div class="folder-header" @click="handleToggleClick">
      <svg 
        class="folder-icon" 
        :class="{ 'is-expanded': folder.expanded }"
        xmlns="http://www.w3.org/2000/svg" 
        width="16" 
        height="16" 
        viewBox="0 0 24 24" 
        fill="none" 
        stroke="currentColor" 
        stroke-width="2" 
        stroke-linecap="round" 
        stroke-linejoin="round"
      >
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
      </svg>
      <span class="folder-name">{{ folder.name }}</span>
      <span class="folder-count">{{ folder.streamerIds.length }}</span>
      <motion.span
        class="expand-icon"
        :animate="{ rotate: folder.expanded ? 180 : 0 }"
        :transition="{ duration: 0.2, ease: [0.25, 0.8, 0.4, 1] }"
      >
        <svg 
          xmlns="http://www.w3.org/2000/svg" 
          width="12" 
          height="12" 
          viewBox="0 0 24 24" 
          fill="none" 
          stroke="currentColor" 
          stroke-width="2.5" 
          stroke-linecap="round" 
          stroke-linejoin="round"
        >
          <polyline points="6 9 12 15 18 9"></polyline>
        </svg>
      </motion.span>
    </div>
    <AnimatePresence :initial="false">
      <motion.div
        v-if="folder.expanded && folderItems.length > 0"
        class="folder-content"
        :class="{ 'disable-pointer': globalDragging }"
        ref="folderContentRef"
        @mouseleave="handleFolderContentMouseLeave"
        :initial="{ height: 0, opacity: 0 }"
        :animate="{ height: folderContentHeight, opacity: 1 }"
        :exit="{ height: 0, opacity: 0, transition: { type: 'tween', duration: 0.24, ease: [0.64, 0, 0.78, 0.39] } }"
        :transition="{ type: 'tween', duration: 0.24, ease: [0.22, 0.61, 0.36, 1] }"
        >
        <motion.div
          class="folder-hover-highlight"
          :initial="folderHoverHighlightInitial"
          :animate="folderHoverHighlightMotion"
          :transition="folderHoverHighlightTransition"
          aria-hidden="true"
          :style="{ borderRadius: '12px', minHeight: '38px' }"
        />
        <ul class="folder-streamers-list">
          <li
            v-for="(streamer, index) in folderItems"
            :key="`${streamer.platform}:${streamer.id}`"
            class="folder-streamer-item"
            :class="getStreamerItemClass(streamer)"
            :ref="(el) => setFolderItemRef(index, el)"
            @mouseenter="handleFolderItemMouseEnter(index)"
            @click.stop="handleClick(streamer)"
            @mousedown.stop="(e) => handleFolderStreamerMouseDown(streamer, e)"
            @mouseup.stop="handleFolderStreamerMouseUp"
            @mouseleave="() => { handleFolderItemMouseLeave(index); handleFolderStreamerMouseUp(); }"
          >
            <StreamerItem 
              :streamer="streamer"
              :getAvatarSrc="getAvatarSrc"
              :handleImgError="handleImgError"
              :getLiveIndicatorClass="getLiveIndicatorClass"
              :proxyBase="proxyBase"
              @clickItem="(s) => emit('selectAnchor', s)"
            />
          </li>
        </ul>
      </motion.div>
    </AnimatePresence>
  </motion.div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from 'vue';
import { AnimatePresence, motion } from 'motion-v';
import type { FollowedStreamer } from '../../platforms/common/types';
import type { FollowFolder } from '../../store/followStore';
import StreamerItem from './StreamerItem.vue';

const props = defineProps<{
  folder: FollowFolder;
  allStreamers: FollowedStreamer[];
  getAvatarSrc: (s: FollowedStreamer) => string;
  handleImgError: (ev: Event, s: FollowedStreamer) => void;
  getLiveIndicatorClass: (s: FollowedStreamer) => string;
  proxyBase?: string;
  isDragging?: boolean;
  isDragOver?: boolean;
  canAcceptDrop?: boolean;
  globalDragging?: boolean;
}>();

const emit = defineEmits<{
  (e: 'selectAnchor', streamer: FollowedStreamer): void;
  (e: 'toggleExpand', folderId: string): void;
  (e: 'dragStart', folderId: string, event: MouseEvent): void;
  (e: 'contextMenu', folderId: string, event: MouseEvent): void;
  (e: 'dragOver', folderId: string): void;
  (e: 'dragLeave'): void;
  (e: 'drop', folderId: string): void;
  (e: 'streamerDragStart', streamer: FollowedStreamer, event: MouseEvent): void;
}>();

const sortStreamersByStatus = (items: FollowedStreamer[]): FollowedStreamer[] => {
  const live: FollowedStreamer[] = [];
  const looping: FollowedStreamer[] = [];
  const rest: FollowedStreamer[] = [];
  items.forEach(streamer => {
    if (streamer.liveStatus === 'LIVE' || (!streamer.liveStatus && streamer.isLive)) {
      live.push(streamer);
    } else if (streamer.liveStatus === 'REPLAY') {
      looping.push(streamer);
    } else {
      rest.push(streamer);
    }
  });
  return [...live, ...looping, ...rest];
};

const folderItems = computed(() => {
  const seen = new Set<string>();
  const result: FollowedStreamer[] = [];
  for (const key of props.folder.streamerIds) {
    const [platform, id] = (key || '').split(':');
    const platformKey = (platform || '').toUpperCase();
    const dedupKey = `${platformKey}:${id}`;
    if (seen.has(dedupKey)) continue;
    const found = props.allStreamers.find((s: FollowedStreamer) => String(s.platform).toUpperCase() === platformKey && s.id === id);
    if (found) {
      seen.add(dedupKey);
      result.push(found);
    }
  }
  return sortStreamersByStatus(result);
});

const folderContentRef = ref<HTMLElement | null>(null);
const folderContentHeight = ref(0);
let folderContentObserver: ResizeObserver | null = null;
const folderItemRefs = new Map<number, HTMLElement>();
const hoveredFolderIndex = ref<number | null>(null);
const folderHoverHighlight = reactive({ x: 0, y: 0, width: 0, height: 0, visible: false });
const folderHoverHighlightInitial = { x: 0, y: 0, width: 0, height: 32, opacity: 0 };
const folderHoverHighlightTransition = { type: 'spring', stiffness: 280, damping: 28, mass: 0.7 } as const;
const folderHoverHighlightMotion = computed(() => ({
  x: folderHoverHighlight.x,
  y: folderHoverHighlight.y,
  width: folderHoverHighlight.width,
  height: folderHoverHighlight.height,
  opacity: folderHoverHighlight.visible ? 1 : 0,
}));

const resolveElement = (el: any): HTMLElement | null => {
  if (!el) return null;
  const element = el?.$el ?? el;
  return element instanceof HTMLElement ? element : null;
};

const recomputeFolderHoverHighlight = () => {
  const index = hoveredFolderIndex.value;
  if (index === null) {
    folderHoverHighlight.visible = false;
    return;
  }
  const contentEl = resolveElement(folderContentRef.value);
  const itemEl = folderItemRefs.get(index);
  if (!contentEl || !itemEl) {
    folderHoverHighlight.visible = false;
    return;
  }
  const contentRect = contentEl.getBoundingClientRect();
  const itemRect = itemEl.getBoundingClientRect();
  const scrollLeft = contentEl.scrollLeft;
  const scrollTop = contentEl.scrollTop;
  folderHoverHighlight.x = itemRect.left - contentRect.left + scrollLeft;
  folderHoverHighlight.y = itemRect.top - contentRect.top + scrollTop;
  folderHoverHighlight.width = itemRect.width;
  folderHoverHighlight.height = itemRect.height;
  folderHoverHighlight.visible = true;
};

const updateFolderContentHeight = () => {
  const contentEl = resolveElement(folderContentRef.value);
  if (!contentEl) return;
  folderContentHeight.value = contentEl.scrollHeight;
};

watch(folderContentRef, (current) => {
  if (folderContentObserver) {
    folderContentObserver.disconnect();
    folderContentObserver = null;
  }
  const element = resolveElement(current);
  if (!element) return;
  folderContentObserver = new ResizeObserver(updateFolderContentHeight);
  folderContentObserver.observe(element);
  nextTick(updateFolderContentHeight);
});

const setFolderItemRef = (index: number, el: any) => {
  if (!el) {
    folderItemRefs.delete(index);
    return;
  }
  const element = el?.$el ?? el;
  if (element instanceof HTMLElement) {
    folderItemRefs.set(index, element);
    if (hoveredFolderIndex.value === index) {
      nextTick(recomputeFolderHoverHighlight);
    }
  }
};

const handleFolderItemMouseEnter = (index: number) => {
  hoveredFolderIndex.value = index;
  nextTick(recomputeFolderHoverHighlight);
};

const handleFolderItemMouseLeave = (index: number) => {
  if (hoveredFolderIndex.value === index) {
    hoveredFolderIndex.value = null;
    folderHoverHighlight.visible = false;
  }
};

const handleFolderContentMouseLeave = () => {
  hoveredFolderIndex.value = null;
  folderHoverHighlight.visible = false;
};

watch(folderItems, () => {
  folderItemRefs.clear();
  hoveredFolderIndex.value = null;
  folderHoverHighlight.visible = false;
  nextTick(() => {
    recomputeFolderHoverHighlight();
    updateFolderContentHeight();
  });
});

watch(() => props.folder.expanded, (expanded) => {
  if (!expanded) {
    hoveredFolderIndex.value = null;
    folderHoverHighlight.visible = false;
    folderContentHeight.value = 0;
    return;
  }
  nextTick(updateFolderContentHeight);
});

onBeforeUnmount(() => {
  folderItemRefs.clear();
  if (folderContentObserver) {
    folderContentObserver.disconnect();
    folderContentObserver = null;
  }
});

const toggleExpand = () => emit('toggleExpand', props.folder.id);

const LONG_PRESS_MS = 220;
const MOVE_THRESHOLD_PX = 6;
let headerPressTimer: number | null = null;
let headerLongPressTriggered = false;
let headerDragStarted = false;
let headerStartPoint: { x: number; y: number } | null = null;

const clearHeaderPress = () => {
  if (headerPressTimer !== null) {
    clearTimeout(headerPressTimer);
    headerPressTimer = null;
  }
};

const cleanupHeaderListeners = () => {
  document.removeEventListener('mousemove', handleHeaderMouseMove);
  document.removeEventListener('mouseup', handleHeaderMouseUp);
};

const handleHeaderMouseMove = (e: MouseEvent) => {
  if (!headerStartPoint || headerDragStarted) return;
  const movedDist = Math.hypot(e.clientX - headerStartPoint.x, e.clientY - headerStartPoint.y);
  if (movedDist >= MOVE_THRESHOLD_PX) {
    headerDragStarted = true;
    clearHeaderPress();
    emit('dragStart', props.folder.id, e);
    headerStartPoint = null;
    cleanupHeaderListeners();
  }
};

const handleHeaderMouseDown = (e: MouseEvent) => {
  if (e.button !== 0 || props.globalDragging) return;
  e.preventDefault();
  headerLongPressTriggered = false;
  headerDragStarted = false;
  headerStartPoint = { x: e.clientX, y: e.clientY };
  clearHeaderPress();
  cleanupHeaderListeners();
  document.addEventListener('mousemove', handleHeaderMouseMove);
  document.addEventListener('mouseup', handleHeaderMouseUp);
  headerPressTimer = window.setTimeout(() => {
    headerLongPressTriggered = true;
    headerDragStarted = true;
    emit('dragStart', props.folder.id, e);
    headerStartPoint = null;
    cleanupHeaderListeners();
  }, LONG_PRESS_MS);
};

const handleHeaderMouseUp = () => {
  clearHeaderPress();
  headerStartPoint = null;
  cleanupHeaderListeners();
};

const handleToggleClick = () => {
  if (headerLongPressTriggered) {
    headerLongPressTriggered = false;
    return;
  }
  toggleExpand();
};

const handleContextMenu = (e: MouseEvent) => {
  emit('contextMenu', props.folder.id, e);
};

const handleClick = (streamer: FollowedStreamer) => {
  // 若已进入长按拖动，阻止点击进入观看
  if (longPressTriggered) {
    longPressTriggered = false;
    return;
  }
  emit('selectAnchor', streamer);
};

const handleMouseEnter = () => {
  if (props.canAcceptDrop) {
    emit('dragOver', props.folder.id);
  }
};

const handleMouseLeave = () => {
  clearHeaderPress();
  headerStartPoint = null;
  cleanupHeaderListeners();
  if (props.canAcceptDrop) {
    emit('dragLeave');
  }
};

// 长按触发拖动，避免单击立即进入拖动
const LONG_PRESS_MS_FOL = 220;
const MOVE_THRESHOLD_PX_FOL = 6;
let longPressTimer: number | null = null;
let longPressTriggered = false;
let longPressDragStarted = false;
let longPressStartPoint: { x: number; y: number } | null = null;
let longPressStreamer: FollowedStreamer | null = null;

const clearLongPress = () => {
  if (longPressTimer !== null) {
    clearTimeout(longPressTimer);
    longPressTimer = null;
  }
};

const cleanupFolderStreamerListeners = () => {
  document.removeEventListener('mousemove', handleFolderStreamerMouseMove);
  document.removeEventListener('mouseup', handleFolderStreamerMouseUp);
};

const handleFolderStreamerMouseMove = (event: MouseEvent) => {
  if (!longPressStartPoint || longPressDragStarted) return;
  const movedDist = Math.hypot(event.clientX - longPressStartPoint.x, event.clientY - longPressStartPoint.y);
  if (movedDist >= MOVE_THRESHOLD_PX_FOL) {
    longPressDragStarted = true;
    longPressTriggered = true;
    clearLongPress();
    if (longPressStreamer) {
      emit('streamerDragStart', longPressStreamer, event);
    }
    longPressStartPoint = null;
    cleanupFolderStreamerListeners();
  }
};

const handleFolderStreamerMouseDown = (streamer: FollowedStreamer, event: MouseEvent) => {
  if (props.globalDragging) return;
  event.preventDefault();
  longPressTriggered = false;
  longPressDragStarted = false;
  longPressStartPoint = { x: event.clientX, y: event.clientY };
  longPressStreamer = streamer;
  clearLongPress();
  cleanupFolderStreamerListeners();
  document.addEventListener('mousemove', handleFolderStreamerMouseMove);
  document.addEventListener('mouseup', handleFolderStreamerMouseUp);
  longPressTimer = window.setTimeout(() => {
    longPressTriggered = true;
    longPressDragStarted = true;
    emit('streamerDragStart', streamer, event);
    longPressStartPoint = null;
    cleanupFolderStreamerListeners();
  }, LONG_PRESS_MS_FOL);
};

const handleFolderStreamerMouseUp = () => {
  clearLongPress();
  longPressStartPoint = null;
  longPressStreamer = null;
  cleanupFolderStreamerListeners();
};

const getStreamerItemClass = (streamer: FollowedStreamer) => {
  return {
    'status-live': streamer.liveStatus === 'LIVE',
    'status-replay': streamer.liveStatus === 'REPLAY',
    'status-offline': streamer.liveStatus === 'OFFLINE' || !streamer.liveStatus || streamer.liveStatus === 'UNKNOWN',
  };
};

</script>

<style scoped>
.folder-item {
  position: relative;
  margin-bottom: 8px;
  border-radius: 24px;
  background: var(--color-card);
  border: 1px solid var(--color-border);
  box-shadow: none;
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
  overflow: hidden;
  transition: box-shadow 0.25s ease, background 0.25s ease, border-color 0.25s ease;
  user-select: none;
}

:root[data-theme="light"] .folder-item {
  background: rgba(255, 255, 255, 0.92);
  border: 1px solid rgba(15, 23, 42, 0.12);
  box-shadow: none;
}

.folder-item.is-expanded {
  background: var(--color-card);
}

.folder-item.is-drag-over {
  border-color: rgba(255, 255, 255, 0.7);
  box-shadow:
    0 0 0 2px rgba(255, 255, 255, 0.55),
    0 0 16px rgba(255, 255, 255, 0.45),
    0 0 28px rgba(255, 255, 255, 0.3);
}

.folder-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  cursor: pointer;
  border-radius: 14px;
  transition: all 0.25s ease;
  background: transparent;
  border: none;
}

.folder-header:hover {
  background: rgba(255, 255, 255, 0.08);
}

.folder-item.is-drag-over .folder-header {
  box-shadow: none;
}

.folder-icon {
  width: 16px;
  height: 16px;
  color: #9ca3af;
  transition: all 0.3s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

.folder-item.is-expanded .folder-icon {
  color: #e5e7eb;
  transform: scale(1.2);
  filter: none;
}

.folder-name {
  flex: 1;
  font-weight: 700;
  font-size: 14px;
  color: var(--primary-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  letter-spacing: 0.02em;
  transition: color 0.2s ease;
}

:root[data-theme="dark"] .folder-name {
  color: #e7eee9;
}

.folder-count {
  font-size: 10px;
  color: #9ca3af;
  font-weight: 800;
  background: rgba(0, 0, 0, 0.18);
  padding: 2px 8px;
  border-radius: 20px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  box-shadow: none;
  transition: color 0.2s ease, border-color 0.2s ease, background 0.2s ease;
}

.expand-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 12px;
  height: 12px;
  color: #9ca3af;
}

.folder-content {
  padding: 10px 12px 16px;
  position: relative;
  overflow: hidden;
  will-change: height;
}

.folder-hover-highlight {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
  border-radius: 10px;
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.18), rgba(255, 255, 255, 0.08));
  border: 1px solid rgba(255, 255, 255, 0.2);
  box-shadow: none;
  opacity: 0;
  transform: translate3d(0, 0, 0);
  transition:
    transform 220ms cubic-bezier(0.16, 1, 0.3, 1),
    width 220ms cubic-bezier(0.16, 1, 0.3, 1),
    height 220ms cubic-bezier(0.16, 1, 0.3, 1),
    opacity 120ms ease;
  z-index: 0;
}

:root[data-theme="light"] .folder-hover-highlight {
  background: rgba(120, 120, 120, 0.16);
  border: 1px solid transparent;
}

.folder-streamers-list {
  position: relative;
  z-index: 1;
}

.folder-streamers-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.folder-streamer-item {
  position: relative;
  display: flex;
  align-items: center;
  padding: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  transition: all 0.2s ease;
}

.folder-streamer-item:hover {
  background: transparent;
}

.folder-header:hover .folder-name {
  color: #f8fafc;
}

.folder-header:hover .folder-count {
  color: #f8fafc;
  border-color: rgba(255, 255, 255, 0.35);
  background: rgba(255, 255, 255, 0.06);
}

:root[data-theme="light"] .folder-header:hover .folder-name {
  color: #0f172a;
}

:root[data-theme="light"] .folder-header:hover {
  background: rgba(15, 23, 42, 0.06);
}

:root[data-theme="light"] .folder-header:hover .folder-count {
  color: #0f172a;
  background: rgba(15, 23, 42, 0.04);
}

:root[data-theme="light"] .folder-icon,
:root[data-theme="light"] .expand-icon {
  color: #6b7280;
}

:root[data-theme="light"] .folder-item.is-expanded .folder-icon {
  color: #111827;
}

:root[data-theme="light"] .folder-count {
  color: #6b7280;
  background: rgba(15, 23, 42, 0.06);
  border-color: rgba(15, 23, 42, 0.12);
}
</style>
