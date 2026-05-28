<template>
  <div class="douyin-home">
    <div class="douyin-content">
      <div ref="categorySectionRef" class="left-panel">
        <CommonCategory
          :categories-data="categoriesData"
          :preferred-cate1-href="preferredCate1Href"
          :preferred-cate2-href="preferredCate2Href"
          :is-cate2-subscribed="isCate2Subscribed"
          @category-selected="onCategorySelected"
          @subscribe-cate2="handleSubscribeCate2"
        />
      </div>

      <div class="right-panel">
        <CommonStreamerList
          ref="streamerListRef"
          :selected-category="selectedCategory"
          :categories-data="categoriesData"
          platformName="douyin"
          playerRouteName="douyinPlayer"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import CommonCategory from '../components/CommonCategory/index.vue'
import CommonStreamerList from '../components/CommonStreamerList/index.vue'
import { douyinCategoriesData } from '../platforms/douyin/douyinCategoriesData'
import type { CategorySelectedEvent } from '../platforms/common/categoryTypes'
import { useCustomCategoryStore } from '../store/customCategoryStore'

defineOptions({
  name: 'DouyinHomeView',
})

interface RefreshableStreamerList {
  refreshCurrentList: () => Promise<boolean>
  appendBottomRefreshRooms: () => Promise<boolean>
  canTriggerBottomRefresh: () => boolean
}

const route = useRoute()
const categoriesData = douyinCategoriesData
const selectedCategory = ref<CategorySelectedEvent | null>(null)
const streamerListRef = ref<RefreshableStreamerList | null>(null)
const categorySectionRef = ref<HTMLElement | null>(null)
const customStore = useCustomCategoryStore()
customStore.ensureLoaded()

const preferredCate1Href = computed(() => (typeof route.query.cate1 === 'string' ? route.query.cate1 : null))
const preferredCate2Href = computed(() => (typeof route.query.cate2 === 'string' ? route.query.cate2 : null))

const isCate2Subscribed = (href: string) => customStore.isCate2Subscribed('douyin', href)

function onCategorySelected(evt: CategorySelectedEvent) {
  selectedCategory.value = evt
}

const handleSubscribeCate2 = (evt: CategorySelectedEvent) => {
  customStore.toggleCommonCate2(
    'douyin',
    evt.cate2Href,
    evt.cate2Name,
    evt.cate1Name,
    evt.cate1Href,
  )
}

const refreshPageContent = async () => {
  if (!selectedCategory.value?.cate2Href) {
    return false
  }
  return await streamerListRef.value?.refreshCurrentList() ?? false
}

const appendBottomRefreshContent = async () => {
  if (!selectedCategory.value?.cate2Href) {
    return false
  }
  return await streamerListRef.value?.appendBottomRefreshRooms() ?? false
}

const canTriggerBottomRefresh = () => {
  if (!selectedCategory.value?.cate2Href) {
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
.douyin-home {
  display: block;
  min-height: 100%;
  background: transparent;
}

.douyin-content {
  display: block;
  min-height: 100%;
}

.left-panel {
  width: 100%;
  background: transparent;
  z-index: 10;
  overflow: visible;
}

.right-panel {
  overflow: visible;
  background: transparent;
}
</style>
