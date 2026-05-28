<template>
    <div class="danmu-list-wrapper">
      <div class="danmu-actions-slot">
        <slot name="actions" />
      </div>
      <div v-if="showFilterPanel" class="danmu-filter-panel" @pointerdown.stop>
        <div class="panel-header">
          <span class="panel-title">屏蔽关键词</span>
          <button class="panel-close" type="button" @click="toggleFilterPanel" title="关闭">×</button>
        </div>
        <div class="panel-body">
          <div class="panel-input-row">
            <input
              v-model="keywordInput"
              class="panel-input"
              type="text"
              placeholder="输入关键词回车添加"
              @keydown.enter.prevent="addKeyword"
            />
          </div>
          <div class="panel-list">
            <div v-if="blockedKeywords.length === 0" class="panel-empty">暂无屏蔽词</div>
            <div v-for="(kw, i) in blockedKeywords" :key="`${kw}-${i}`" class="panel-item">
              <span class="panel-item-text">{{ kw }}</span>
              <button class="panel-remove" type="button" @click="removeKeyword(i)">删除</button>
            </div>
          </div>
        </div>
      </div>

      <div class="danmu-messages-area" ref="danmakuListEl" @scroll="handleScroll" @pointerdown="onPointerDown">
        <!-- Empty/Loading Placeholder -->
        <div v-if="(!renderMessages || renderMessages.length === 0)" class="empty-danmu-placeholder">
          <p v-if="!props.roomId">请先选择一个直播间</p>
          <p v-else>暂无弹幕或连接中...</p> <!-- Simplified placeholder -->
        </div>

        <div
          v-for="(danmaku, idx) in renderMessages"
          :key="danmaku.id || `${danmaku.room_id || ''}-${danmaku.nickname}-${danmaku.content}-${idx}`" 
          :class="['danmu-item', { 'system-message': danmaku.isSystem, 'success': danmaku.isSystem && danmaku.type === 'success' }]"
          @click="copyDanmaku(danmaku)"
          title="点击复制弹幕"
        >
          <div class="danmu-meta-line" v-if="!danmaku.isSystem">
            <span v-if="danmaku.badgeName" class="danmu-badge">
              <span class="badge-name">{{ danmaku.badgeName }}</span>
              <span v-if="danmaku.badgeLevel" class="badge-level">{{ danmaku.badgeLevel }}</span>
            </span>
            <span class="danmu-user" :style="{ color: danmaku.color || userColor(danmaku.nickname) }">
              <span v-if="danmaku.level" class="user-level">[Lv.{{ danmaku.level }}]</span>
              {{ danmaku.nickname }}
            </span>
          </div>
          <div class="danmu-content-line">
            <span class="danmu-content">
              <svg v-if="danmaku.isSystem && danmaku.type === 'success'" class="inline-icon success-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41L9 16.17z"/></svg>
              {{ danmaku.content }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </template>
  
  <script setup lang="ts">
  import { ref, watch, nextTick, onMounted, onUnmounted } from 'vue';

  interface DanmakuUIMessage {
    id?: string;
    nickname: string;
    content: string;
    level?: string;
    badgeName?: string;
    badgeLevel?: string;
    color?: string;
    isSystem?: boolean; // 系统消息
    type?: string;
    room_id?: string; // 补充房间ID以便 key 生成更稳定
  }
  
const props = defineProps<{
  roomId: string | null;
  messages: DanmakuUIMessage[];
}>();

const danmakuListEl = ref<HTMLElement | null>(null);
const autoScroll = ref(true); 
const userScrolled = ref(false);
const pointerActive = ref(false);

const showFilterPanel = ref(false);
const keywordInput = ref('');
const blockedKeywords = ref<string[]>([]);
const BLOCK_KEYWORDS_STORAGE = 'danmu_block_keywords';
  
const userColor = (nickname: string | undefined) => {
  if (!nickname || nickname.length === 0) {
    const defaultHue = 0;
    const defaultSaturation = 0;
    const defaultLightness = 75;
      return `hsl(${defaultHue}, ${defaultSaturation}%, ${defaultLightness}%)`;
    }
    let hash = 0;
    for (let i = 0; i < nickname.length; i++) {
      hash = nickname.charCodeAt(i) + ((hash << 5) - hash);
      hash = hash & hash; 
    }
    const hue = hash % 360;
    return `hsl(${hue}, 70%, 75%)`;
  };
  
const isNearBottom = () => {
  const el = danmakuListEl.value;
  if (!el) return true;
  return el.scrollHeight - el.scrollTop - el.clientHeight <= 40;
};

const handleScroll = () => {
  if (!danmakuListEl.value) return;
  const atBottom = isNearBottom();
  userScrolled.value = !atBottom;
  autoScroll.value = atBottom && !pointerActive.value;
};
  
watch(autoScroll, (newValue) => {
  if (newValue) {
    userScrolled.value = false;
    scrollToBottomForce();
  }
});
  
  const renderMessages = ref<DanmakuUIMessage[]>([]);
  const MAX_MSG = 200;
  const PRUNE_BATCH = 100;
  
const onPointerDown = () => {
  pointerActive.value = true;
  autoScroll.value = false; // 用户主动拖动时暂停自动滚动
  userScrolled.value = true;
};
  
  const onGlobalPointerUp = () => {
    if (pointerActive.value) {
      pointerActive.value = false;
      autoScroll.value = true; // 松开后恢复自动滚动
      userScrolled.value = false;
      scrollToBottomForce();
    }
  };
  
  const scrollToBottomForce = () => {
    nextTick(() => {
      const el = danmakuListEl.value;
      if (!el) return;
      // 使用 scrollTo({behavior: 'auto'}) 替代平滑滚动，确保锚点准确
      requestAnimationFrame(() => {
        el.scrollTo({ top: el.scrollHeight, behavior: 'auto' });
        requestAnimationFrame(() => {
          el.scrollTop = el.scrollHeight; // 双重同步确保
        });
      });
    });
  };

watch(() => props.messages, (newMessages, _oldMessages) => {
  const msgs = Array.isArray(newMessages) ? newMessages : [];
  const filtered = filterBlockedMessages(msgs);
  if (filtered.length > MAX_MSG) {
    // 批量裁剪，避免频繁处理导致性能问题
    if (filtered.length % PRUNE_BATCH === 0 || filtered.length > MAX_MSG + PRUNE_BATCH) {
      renderMessages.value = filtered.slice(-MAX_MSG);
    } else {
      renderMessages.value = filtered.slice(-MAX_MSG);
    }
  } else {
    renderMessages.value = filtered;
  }
  if (!pointerActive.value) {
    scrollToBottomForce();
  } else if (autoScroll.value || isNearBottom()) {
    scrollToBottomForce();
  }
}, { deep: true });

watch(blockedKeywords, () => {
  const msgs = Array.isArray(props.messages) ? props.messages : [];
  const filtered = filterBlockedMessages(msgs);
  renderMessages.value = filtered.length > MAX_MSG ? filtered.slice(-MAX_MSG) : filtered;
  if (!pointerActive.value || autoScroll.value || isNearBottom()) {
    scrollToBottomForce();
  }
}, { deep: true });

watch(() => props.roomId, (_newRoomId, _oldRoomId) => {
  userScrolled.value = false;
  autoScroll.value = true;
  scrollToBottomForce();
});

onMounted(() => {
  scrollToBottomForce();
});

onMounted(() => {
  window.addEventListener('pointerup', onGlobalPointerUp);
  loadBlockedKeywords();
});

onUnmounted(() => {
  window.removeEventListener('pointerup', onGlobalPointerUp);
});

const toggleFilterPanel = () => {
  showFilterPanel.value = !showFilterPanel.value;
};

defineExpose({
  toggleFilterPanel,
});

const loadBlockedKeywords = () => {
  if (typeof window === 'undefined') return;
  try {
    const raw = window.localStorage.getItem(BLOCK_KEYWORDS_STORAGE);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        blockedKeywords.value = parsed.filter((v) => typeof v === 'string' && v.trim().length > 0);
      }
    }
  } catch (err) {
    console.warn('[DanmuList] Failed to load blocked keywords:', err);
  }
};

const persistBlockedKeywords = () => {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(BLOCK_KEYWORDS_STORAGE, JSON.stringify(blockedKeywords.value));
  } catch (err) {
    console.warn('[DanmuList] Failed to save blocked keywords:', err);
  }
};

const addKeyword = () => {
  const kw = keywordInput.value.trim();
  if (!kw) return;
  if (blockedKeywords.value.some((k) => k.toLowerCase() === kw.toLowerCase())) {
    keywordInput.value = '';
    return;
  }
  blockedKeywords.value = [...blockedKeywords.value, kw];
  keywordInput.value = '';
  persistBlockedKeywords();
};

const removeKeyword = (idx: number) => {
  if (idx < 0 || idx >= blockedKeywords.value.length) return;
  blockedKeywords.value = blockedKeywords.value.filter((_, i) => i !== idx);
  persistBlockedKeywords();
};

const filterBlockedMessages = (messages: DanmakuUIMessage[]) => {
  if (!blockedKeywords.value.length) return messages;
  const keywords = blockedKeywords.value.map((k) => k.toLowerCase());
  return messages.filter((msg) => {
    if (msg.isSystem) return true;
    const content = (msg.content || '').toLowerCase();
    if (!content) return true;
    return !keywords.some((kw) => content.includes(kw));
  });
};

const copyDanmaku = async (danmaku: DanmakuUIMessage) => {
  const parts: string[] = [];
  if (danmaku.nickname) {
    const levelStr = danmaku.level ? ` [Lv.${danmaku.level}]` : '';
    parts.push(`${danmaku.nickname}${levelStr}:`);
  }
  parts.push(danmaku.content || '');
  const text = parts.join(' ');

  try {
    if (navigator?.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
    }
  } catch (err) {
    console.warn('复制弹幕失败', err);
  }
};
  
  </script>
  
  <style scoped>
  .danmu-list-wrapper {
    display: flex;
    flex-direction: column;
    position: relative;
    height: 100%;
    min-height: 0;
    max-height: 100%;
    width: 100%;
    background: transparent;
    color: var(--primary-text, #e5e9f5);
    font-family: var(--danmu-font-family, "OPPO Sans", "Microsoft YaHei", "PingFang SC", "Helvetica Neue", Arial, sans-serif);
    border-radius: 20px;
    border: none;
    box-shadow: none;
    overflow: hidden;
    isolation: isolate;
  }

  .danmu-list-wrapper::before {
    display: none;
  }

  .danmu-list-wrapper::after {
    display: none;
  }

  .danmu-list-wrapper > * {
    position: relative;
    z-index: 1;
  }

  .danmu-actions-slot {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    pointer-events: none;
  }

  .danmu-actions-slot :deep(*) {
    pointer-events: auto;
  }

  .danmu-filter-panel {
    position: absolute;
    top: 54px;
    right: 12px;
    width: min(280px, calc(100% - 24px));
    background: rgba(15, 23, 42, 0.94);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    padding: 10px 12px 12px;
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    z-index: 3;
    box-shadow: 0 10px 24px rgba(7, 12, 24, 0.35);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .panel-title {
    font-size: 0.8rem;
    color: rgba(229, 236, 255, 0.9);
    font-weight: 600;
  }

  .panel-close {
    border: none;
    background: transparent;
    color: rgba(229, 236, 255, 0.7);
    font-size: 1rem;
    cursor: pointer;
  }

  .panel-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .panel-input-row {
    display: flex;
    gap: 6px;
  }

  .panel-input {
    flex: 1;
    height: 30px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.08);
    color: rgba(236, 242, 255, 0.92);
    padding: 0 8px;
    font-size: 0.78rem;
    outline: none;
  }

  .panel-list {
    max-height: 180px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-right: 2px;
  }

  .panel-list::-webkit-scrollbar {
    width: 0;
    height: 0;
    display: none;
  }

  .panel-empty {
    font-size: 0.76rem;
    color: rgba(200, 210, 236, 0.75);
    text-align: center;
    padding: 6px 0;
  }

  .panel-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 6px 8px;
  }

  .panel-item-text {
    font-size: 0.78rem;
    color: rgba(236, 242, 255, 0.9);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .panel-remove {
    border: none;
    background: rgba(255, 92, 92, 0.2);
    color: #ffb2b2;
    padding: 2px 6px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.72rem;
  }
  
  .danmu-messages-area {
    position: relative;
    flex: 1;
    min-height: 0;
    max-height: 100%;
    overflow-y: auto; 
    padding: 10px 12px;
    scroll-behavior: smooth;
  }
  
  .empty-danmu-placeholder {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    text-align: center;
    width: 100%;
  }
  .empty-danmu-placeholder p {
    margin: 4px 0;
  }
  
.danmu-item {
  text-align: left;
  padding: 6px 10px;
  border-radius: 12px;
  background: transparent;
  border: none;
  word-wrap: break-word;
  overflow-wrap: break-word;
  margin-bottom: 4px;
  transition: transform 0.2s ease;
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-width: 100%; 
  cursor: pointer;
}
  
.danmu-item:hover {
  transform: translateY(-2px);
}
  
.danmu-meta-line {
  font-size: 0.72rem;
  color: rgba(204, 212, 236, 0.72);
  margin-bottom: 0;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  letter-spacing: 0.01em;
  gap: 6px;
}
  
  .danmu-badge {
    background: linear-gradient(135deg, rgba(92, 153, 255, 0.75), rgba(236, 112, 214, 0.68)); 
    color: #ffffff; 
    padding: 2px 7px;
    border-radius: 999px;
    font-size: 0.64rem; 
    margin-right: 8px;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    height: auto;
    line-height: normal;
    flex-shrink: 0;
    box-shadow: 0 6px 14px rgba(100, 140, 255, 0.24);
  }
  
  .badge-level {
    margin-left: 4px;
    font-weight: 600;
    font-size: 0.62rem; 
  }
  
  .danmu-user {
    font-weight: 600;
    margin-right: 6px;
    color: inherit;
  }
  
  .user-level {
    font-size: 0.7rem;
    color: rgba(166, 183, 219, 0.85); 
    margin-right: 5px;
  }
  
.danmu-content-line {
  font-size: 0.8rem;
  line-height: 1.4;
  display: inline-flex;
  max-width: 100%;
}

.danmu-content {
  color: rgba(244, 246, 255, 0.94); 
  white-space: pre-wrap; 
  word-wrap: break-word;
  overflow-wrap: break-word;
  font-size: 0.84rem; 
  line-height: 1.45;
  text-shadow: 0 1px 2px rgba(6, 9, 18, 0.6);
  display: inline-flex;
  align-items: center;
  padding: 6px 10px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.06);
  width: fit-content;
  max-width: 100%;
}
  
  .danmu-messages-area::-webkit-scrollbar {
    width: 0;
    height: 0;
    display: none;
  }

  .danmu-messages-area {
    scrollbar-width: none;
  }

  @media (max-width: 1024px) {
    .danmu-list-wrapper {
      width: 100%;
      border-radius: 12px;
      border-left: 1px solid rgba(255, 255, 255, 0.08);
    }
  }

  .connection-status-placeholder {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    text-align: center;
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 10px;
  }
  
  .connection-status-placeholder.success {
    color: #2f8f46;
  }
  
  .connection-status-placeholder .status-icon {
    width: 32px;
    height: 32px;
    margin-bottom: 8px;
  }
  
  .connection-status-placeholder p {
    margin: 4px 0;
    font-size: 0.9rem; 
    font-weight: 500;
  }
  
  .danmu-item.system-message {
    background: rgba(57, 185, 108, 0.16);
    border-left: 3px solid rgba(57, 185, 108, 0.75);
    margin-top: 4px;
    margin-bottom: 6px;
    box-shadow: 0 10px 20px rgba(26, 54, 39, 0.32);
  }

  .danmu-item.system-message .danmu-content {
    font-weight: 500;
    color: rgba(210, 240, 220, 0.95);
  }

  .danmu-item.system-message.success .danmu-content {
    color: #49df85;
    font-weight: 600;
    background: transparent;
    border: none;
    padding: 0;
  }

  .danmu-item.system-message.success {
    background: transparent;
    border-left: none;
    box-shadow: none;
  }

  .inline-icon {
    width: 1.1em;
    height: 1.1em;
    margin-right: 8px;
    vertical-align: -0.15em;
  }
  
.success-icon {
  fill: #49df85;
}
  

:root[data-theme="light"] .danmu-list-wrapper {
  background: var(--glass-bg);
  color: var(--primary-text-light, #1f2937);
  border: 1px solid var(--glass-border);
  border-left: none;
  box-shadow:
    0 2px 6px rgba(15, 23, 42, 0.06),
    0 10px 22px rgba(15, 23, 42, 0.08);
}

:root[data-theme="light"] .danmu-filter-panel {
  background: rgba(255, 255, 255, 0.96);
  border: 1px solid rgba(148, 163, 184, 0.5);
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.18);
}

:root[data-theme="light"] .panel-title {
  color: rgba(31, 41, 55, 0.9);
}

:root[data-theme="light"] .panel-input {
  background: rgba(248, 250, 252, 1);
  color: rgba(15, 23, 42, 0.9);
  border: 1px solid rgba(203, 213, 225, 0.9);
}

:root[data-theme="light"] .panel-item {
  background: rgba(248, 250, 252, 1);
  border: 1px solid rgba(203, 213, 225, 0.9);
}

:root[data-theme="light"] .panel-item-text {
  color: rgba(31, 41, 55, 0.9);
}

:root[data-theme="light"] .panel-empty {
  color: rgba(100, 116, 139, 0.85);
}

:root[data-theme="light"] .danmu-list-wrapper::before {
  display: none;
}

:root[data-theme="light"] .danmu-list-wrapper::after {
  display: none;
}

 

:root[data-theme="light"] .empty-danmu-placeholder p {
  color: rgba(100, 116, 139, 0.85);
}

:root[data-theme="light"] .danmu-meta-line {
  color: rgba(71, 85, 105, 0.85);
}

:root[data-theme="light"] .danmu-badge {
  color: #ffffff; 
  box-shadow: 0 6px 14px rgba(100, 140, 255, 0.28);
}

:root[data-theme="light"] .user-level {
  color: rgba(100, 116, 139, 0.78);
}

:root[data-theme="light"] .danmu-content {
  color: var(--primary-text-light, #1f2937);
  text-shadow: none;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
}

:root[data-theme="light"] .danmu-item.system-message {
  background: rgba(226, 246, 233, 0.95);
  border-left-color: rgba(78, 196, 120, 0.75);
}

:root[data-theme="light"] .danmu-item.system-message .danmu-content {
  color: rgba(31, 106, 58, 0.9);
}

:root[data-theme="light"] .danmu-item.system-message.success {
  background: transparent;
  border-left-color: transparent;
  box-shadow: none;
}

:root[data-theme="light"] .danmu-item.system-message.success .danmu-content {
  color: rgba(46, 114, 66, 0.95);
}

:root[data-theme="light"] .success-icon {
  fill: rgba(46, 114, 66, 0.95);
}

:root[data-theme="light"] .connection-status-placeholder.success {
  color: rgba(46, 114, 66, 0.95);
}

@media (max-width: 1024px) {
  :root[data-theme="light"] .danmu-list-wrapper {
    border-left: 1px solid rgba(189, 200, 224, 0.55);
  }
}
  
  </style>
