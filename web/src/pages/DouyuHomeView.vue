<template>
  <div class="home-page">
    <div ref="categorySectionRef" class="category-section">
      <CategoryList
        :preferred-short-name="preferredShortName"
        :is-cate2-subscribed="isCate2Subscribed"
        @category-selected="handleCategorySelected"
        @subscribe-cate2="handleSubscribeCate2"
      />
    </div>

    <div v-if="selectedCategoryInfo" class="live-list-section">
      <CommonStreamerList
        ref="streamerListRef"
        :key="selectedCategoryInfo.id"
        :douyu-category="selectedCategoryInfo"
        platformName="douyu"
        playerRouteName="douyuPlayer"
      />
    </div>

    <div v-else-if="isLoadingDefaultCategory" class="loading-section">
      <div class="loading-message">正在加载默认分区...</div>
    </div>

    <div v-else class="loading-section">
      <div class="loading-message">请选择一个分区。</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import CategoryList from '../components/DouyuCategory/index.vue'
import CommonStreamerList from '../components/CommonStreamerList/index.vue'
import type { CategorySelectedEvent } from '../components/DouyuCategory/types'
import { useCustomCategoryStore } from '../store/customCategoryStore'

defineOptions({
  name: 'DouyuHomeView',
})

interface FrontendCate2Item {
  id: string
  name: string
  short_name: string
}

interface FrontendCate1Item {
  cate2List: FrontendCate2Item[]
}

interface FrontendCategoryResponse {
  cate1List: FrontendCate1Item[]
}

interface SelectedCategoryInfo {
  type: 'cate2'
  id: string
  name?: string
}

interface RefreshableStreamerList {
  refreshCurrentList: () => Promise<boolean>
  appendBottomRefreshRooms: () => Promise<boolean>
  canTriggerBottomRefresh: () => boolean
}

const route = useRoute()
const selectedCategoryInfo = ref<SelectedCategoryInfo | null>(null)
const isLoadingDefaultCategory = ref(true)
const streamerListRef = ref<RefreshableStreamerList | null>(null)
const categorySectionRef = ref<HTMLElement | null>(null)
const customStore = useCustomCategoryStore()
customStore.ensureLoaded()

const preferredShortName = computed(() =>
  typeof route.query.shortName === 'string' ? route.query.shortName : null,
)

const isCate2Subscribed = (shortName: string) => customStore.isCate2Subscribed('douyu', shortName)

const handleCategorySelected = (event: CategorySelectedEvent) => {
  if (!event.shortName) {
    selectedCategoryInfo.value = null
    return
  }

  selectedCategoryInfo.value = {
    type: 'cate2',
    id: event.shortName,
    name: event.cate2Name || event.shortName,
  }
}

const handleSubscribeCate2 = (event: CategorySelectedEvent) => {
  if (!event.shortName) return
  customStore.toggleDouyuCate2(event.shortName, event.cate2Name || event.shortName, event.cate2Id)
}

const fetchDefaultCategory = async () => {
  if (preferredShortName.value) {
    isLoadingDefaultCategory.value = false
    return
  }

  isLoadingDefaultCategory.value = true
  try {
    const response = await invoke<FrontendCategoryResponse>('fetch_categories')
    const defaultCate2 = response?.cate1List?.[0]?.cate2List?.[0]

    if (defaultCate2?.short_name) {
      selectedCategoryInfo.value = {
        type: 'cate2',
        id: defaultCate2.short_name,
        name: defaultCate2.name,
      }
    } else {
      selectedCategoryInfo.value = null
    }
  } catch (error) {
    console.error('[DouyuHomeView] Failed to load default category:', error)
    selectedCategoryInfo.value = null
  } finally {
    isLoadingDefaultCategory.value = false
  }
}

onMounted(() => {
  fetchDefaultCategory()
})

const refreshPageContent = async () => {
  if (!selectedCategoryInfo.value) {
    return false
  }
  return await streamerListRef.value?.refreshCurrentList() ?? false
}

const appendBottomRefreshContent = async () => {
  if (!selectedCategoryInfo.value) {
    return false
  }
  return await streamerListRef.value?.appendBottomRefreshRooms() ?? false
}

const canTriggerBottomRefresh = () => {
  if (!selectedCategoryInfo.value) {
    return false
  }
  return streamerListRef.value?.canTriggerBottomRefresh?.() ?? false
}

const scrollToTopAnchor = () => {
  categorySectionRef.value?.scrollIntoView({ block: 'start', behavior: 'auto' })
}

defineExpose({
  refreshPageContent,
  appendBottomRefreshContent,
  canTriggerBottomRefresh,
  scrollToTopAnchor,
})
</script>

<style scoped>
.home-page {
  min-height: 100%;
  display: block;
  overflow: visible;
  background: transparent;
  min-width: 0;
}

.category-section {
  z-index: 10;
  position: sticky;
  top: 0;
  width: 100%;
  background: transparent;
}

.live-list-section {
  overflow: visible;
  width: 100%;
  background: transparent;
}

.loading-section {
  min-height: 240px;
  display: flex;
  justify-content: center;
  align-items: center;
  background: transparent;
}

.loading-message {
  color: var(--secondary-text);
  font-size: 16px;
}
</style>
