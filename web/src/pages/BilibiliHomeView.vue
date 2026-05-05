<template>
  <div class="bili-home-view-layout">
    <div ref="categorySectionRef" class="bili-category-section">
      <CommonCategory
        :categories-data="biliCategoriesData as any"
        :preferred-cate1-href="preferredCate1Href"
        :preferred-cate2-href="preferredCate2Href"
        :is-cate2-subscribed="isCate2Subscribed"
        @category-selected="onCategorySelected"
        @subscribe-cate2="handleSubscribeCate2"
      />
    </div>

    <CommonStreamerList
      ref="streamerListRef"
      :selected-category="currentSelectedCategory"
      :categories-data="biliCategoriesData as any"
      platformName="bilibili"
      playerRouteName="bilibiliPlayer"
      class="bili-streamer-list-section"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import CommonCategory from '../components/CommonCategory/index.vue'
import CommonStreamerList from '../components/CommonStreamerList/index.vue'
import { biliCategoriesData } from '../platforms/bilibili/biliCategoriesData'
import type { CategorySelectedEvent } from '../platforms/common/categoryTypes.ts'
import { useCustomCategoryStore } from '../store/customCategoryStore'

defineOptions({
  name: 'BilibiliHomeView',
})

interface RefreshableStreamerList {
  refreshCurrentList: () => Promise<boolean>
  appendBottomRefreshRooms: () => Promise<boolean>
  canTriggerBottomRefresh: () => boolean
}

const route = useRoute()
const currentSelectedCategory = ref<CategorySelectedEvent | null>(null)
const streamerListRef = ref<RefreshableStreamerList | null>(null)
const categorySectionRef = ref<HTMLElement | null>(null)
const customStore = useCustomCategoryStore()
customStore.ensureLoaded()

const preferredCate1Href = computed(() => (typeof route.query.cate1 === 'string' ? route.query.cate1 : null))
const preferredCate2Href = computed(() => (typeof route.query.cate2 === 'string' ? route.query.cate2 : null))

const isCate2Subscribed = (href: string) => customStore.isCate2Subscribed('bilibili', href)

const onCategorySelected = (categoryEvent: CategorySelectedEvent) => {
  currentSelectedCategory.value = categoryEvent
}

const handleSubscribeCate2 = (categoryEvent: CategorySelectedEvent) => {
  customStore.toggleCommonCate2(
    'bilibili',
    categoryEvent.cate2Href,
    categoryEvent.cate2Name,
    categoryEvent.cate1Name,
    categoryEvent.cate1Href,
  )
}

const refreshPageContent = async () => {
  if (!currentSelectedCategory.value?.cate2Href) {
    return false
  }
  return await streamerListRef.value?.refreshCurrentList() ?? false
}

const appendBottomRefreshContent = async () => {
  if (!currentSelectedCategory.value?.cate2Href) {
    return false
  }
  return await streamerListRef.value?.appendBottomRefreshRooms() ?? false
}

const canTriggerBottomRefresh = () => {
  if (!currentSelectedCategory.value?.cate2Href) {
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
.bili-home-view-layout {
  display: block;
  min-height: 100%;
  background: transparent;
  overflow: visible;
}

.bili-category-section {
  background: transparent;
  z-index: 10;
}

.bili-streamer-list-section {
  overflow: visible;
  background: transparent;
}
</style>
