<template>
  <div class="custom-home">
    <div v-if="!entries.length" class="custom-empty-tip">暂无订阅分类，点击分类标签右侧的 + 后会出现在这里。</div>

    <div v-if="entries.length" class="custom-list">
      <div
        v-for="entry in entries"
        :key="entry.key"
        class="custom-chip-shell"
        :style="{ transform: `translateX(${swipeOffsets[entry.key] ?? 0}px)` }"
        @touchstart="handleTouchStart(entry, $event)"
        @touchmove="handleTouchMove(entry, $event)"
        @touchend="handleTouchEnd(entry)"
        @touchcancel="handleTouchCancel(entry)"
      >
        <button
          type="button"
          class="custom-chip"
          :class="['platform-' + entry.platform]"
          @click="handleEntryClick(entry)"
        >
          <span class="chip-name">{{ buildEntryLabel(entry) }}</span>
          <span
            class="chip-remove"
            role="button"
            tabindex="0"
            aria-label="删除订阅"
            @click.stop="promptDelete(entry)"
            @keydown.enter.stop.prevent="promptDelete(entry)"
            @keydown.space.stop.prevent="promptDelete(entry)"
          >
            -
          </span>
        </button>
      </div>
    </div>

    <Transition name="dialog">
      <div v-if="pendingDeleteEntry" class="dialog-backdrop" @click="cancelDelete">
        <div class="dialog" @click.stop>
          <h3 class="dialog-title">删除订阅标签</h3>
          <p class="dialog-text">确定删除 {{ buildEntryLabel(pendingDeleteEntry) }} 吗？</p>
          <div class="dialog-actions">
            <button class="dialog-btn cancel" @click="cancelDelete">取消</button>
            <button class="dialog-btn confirm" @click="confirmDelete">删除</button>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useCustomCategoryStore } from '../store/customCategoryStore'
import type { CustomCategoryEntry } from '../store/customCategoryStore'

defineOptions({ name: 'CustomHomeView' })

const router = useRouter()
const store = useCustomCategoryStore()
store.ensureLoaded()

const entries = computed(() => store.entries)
const pendingDeleteEntry = ref<CustomCategoryEntry | null>(null)
const suppressedClickKey = ref<string | null>(null)
const swipeOffsets = reactive<Record<string, number>>({})
const touchSession = reactive({ key: '', startX: 0, startY: 0, isHorizontal: false, moved: false })

const platformLabel = (platform: string) => {
  if (platform === 'douyu') return '\u6597\u9c7c'
  if (platform === 'douyin') return '\u6296\u97f3'
  if (platform === 'huya') return '\u864e\u7259'
  if (platform === 'bilibili') return 'B\u7ad9'
  return platform
}

const buildEntryLabel = (entry: CustomCategoryEntry) => `${platformLabel(entry.platform)}-${entry.cate2Name}`

const buildRouteTarget = (entry: CustomCategoryEntry) => {
  if (entry.platform === 'douyu') {
    return { name: 'DouyuHome', query: { shortName: entry.douyuShortName } }
  }

  const routeNameMap = {
    douyin: 'DouyinHome',
    huya: 'HuyaHome',
    bilibili: 'BilibiliHome',
  } as const

  return {
    name: routeNameMap[entry.platform as keyof typeof routeNameMap],
    query: { cate1: entry.cate1Href, cate2: entry.cate2Href },
  }
}

const resetTouchSession = (key?: string) => {
  if (key) swipeOffsets[key] = 0
  touchSession.key = ''
  touchSession.startX = 0
  touchSession.startY = 0
  touchSession.isHorizontal = false
  touchSession.moved = false
}

const handleTouchStart = (entry: CustomCategoryEntry, event: TouchEvent) => {
  const touch = event.touches[0]
  if (!touch) return
  touchSession.key = entry.key
  touchSession.startX = touch.clientX
  touchSession.startY = touch.clientY
  touchSession.isHorizontal = false
  touchSession.moved = false
  swipeOffsets[entry.key] = 0
}

const handleTouchMove = (entry: CustomCategoryEntry, event: TouchEvent) => {
  if (touchSession.key !== entry.key) return
  const touch = event.touches[0]
  if (!touch) return

  const deltaX = touch.clientX - touchSession.startX
  const deltaY = touch.clientY - touchSession.startY
  if (!touchSession.isHorizontal) {
    if (Math.abs(deltaX) <= 8 || Math.abs(deltaX) <= Math.abs(deltaY)) return
    touchSession.isHorizontal = true
  }

  event.preventDefault()
  touchSession.moved = true
  swipeOffsets[entry.key] = Math.max(-96, Math.min(0, deltaX))
}

const handleTouchEnd = (entry: CustomCategoryEntry) => {
  if (touchSession.key !== entry.key) return
  const shouldDelete = touchSession.moved && (swipeOffsets[entry.key] ?? 0) <= -56
  resetTouchSession(entry.key)
  if (shouldDelete) {
    suppressedClickKey.value = entry.key
    pendingDeleteEntry.value = entry
  }
}

const handleTouchCancel = (entry: CustomCategoryEntry) => resetTouchSession(entry.key)

const handleEntryClick = (entry: CustomCategoryEntry) => {
  if (suppressedClickKey.value === entry.key) {
    suppressedClickKey.value = null
    return
  }
  router.push(buildRouteTarget(entry))
}

const promptDelete = (entry: CustomCategoryEntry) => {
  suppressedClickKey.value = entry.key
  pendingDeleteEntry.value = entry
}

const cancelDelete = () => {
  pendingDeleteEntry.value = null
  suppressedClickKey.value = null
}

const confirmDelete = () => {
  if (!pendingDeleteEntry.value) return
  store.removeByKey(pendingDeleteEntry.value.key)
  pendingDeleteEntry.value = null
  suppressedClickKey.value = null
}
</script>

<style scoped>
.custom-home {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background: transparent;
}

.custom-empty-tip {
  font-size: 12px;
  color: var(--secondary-text);
  padding: 10px 12px 0;
}

.custom-list {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  padding: 6px 12px 14px;
  align-content: flex-start;
}

.custom-chip-shell {
  transition: transform 0.18s ease;
  touch-action: pan-y;
}

.custom-chip {
  border: none;
  background: var(--glass-bg);
  color: var(--primary-text, rgba(236, 242, 255, 0.9));
  border-radius: 16px;
  padding: 10px 16px;
  display: inline-flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  transition: background 0.2s ease, transform 0.2s ease, box-shadow 0.2s ease, border-color 0.2s ease;
  box-shadow: var(--glass-shadow, 0 10px 22px rgba(6, 10, 20, 0.28));
  border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.08));
}

.custom-chip:hover {
  transform: translateY(-1px);
  box-shadow: 0 12px 24px rgba(10, 15, 28, 0.32);
}

.chip-name {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
}

.chip-remove {
  width: 18px;
  height: 18px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  line-height: 1;
  font-weight: 700;
  color: rgba(248, 250, 252, 0.92);
  background: rgba(248, 113, 113, 0.9);
  box-shadow: 0 4px 10px rgba(248, 113, 113, 0.24);
  flex: 0 0 auto;
}

.custom-chip.platform-douyu { color: #ffccb1; }
.custom-chip.platform-huya { color: #ffe2a5; }
.custom-chip.platform-douyin { color: #d7c7ff; }
.custom-chip.platform-bilibili { color: #ffc3df; }

:root[data-theme="light"] .custom-chip {
  background: var(--bg-tertiary);
  color: #1f2937;
  border: none;
  box-shadow:
    0 6px 14px rgba(15, 23, 42, 0.08),
    0 2px 6px rgba(15, 23, 42, 0.06);
}

:root[data-theme="light"] .chip-remove {
  color: #ffffff;
}

.dialog-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(15, 18, 30, 0.48);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 80;
  padding: 24px;
}

.dialog {
  width: min(360px, 100%);
  border-radius: 18px;
  background: rgba(20, 24, 36, 0.96);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 18px 44px rgba(5, 10, 20, 0.38);
  padding: 20px;
}

.dialog-title {
  margin: 0 0 10px;
  font-size: 16px;
  font-weight: 700;
  color: rgba(244, 247, 255, 0.96);
}

.dialog-text {
  margin: 0;
  font-size: 13px;
  line-height: 1.5;
  color: rgba(210, 220, 244, 0.82);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 18px;
}

.dialog-btn {
  border: none;
  border-radius: 999px;
  padding: 8px 16px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.dialog-btn.cancel {
  background: rgba(255, 255, 255, 0.08);
  color: rgba(236, 242, 255, 0.9);
}

.dialog-btn.confirm {
  background: rgba(248, 113, 113, 0.9);
  color: #ffffff;
}
</style>
