<template>
  <div class="category-list">
    <div v-if="isLoading && !hasError" class="loading-state">
      <div class="loading-spinner"></div>
      <div class="loading-text">Loading categories...</div>
    </div>

    <template v-else-if="cate1List.length > 0">
      <Cate1List
        :cate1-list="cate1ListForCommon"
        :selected-cate1-href="selectedCate1Href"
        @select="handleCate1SelectFromCommon"
      />
      <Cate2Grid
        v-if="sortedCate2List.length > 0"
        :cate2-list="cate2ListForCommon"
        :selected-cate2-href="selectedCate2Href"
        :is-subscribed="isCate2SubscribedItem"
        @select="handleCate2SelectFromCommon"
        @subscribe="handleCate2SubscribeFromCommon"
      />
    </template>

    <div v-else-if="hasError" class="error-state">
      <div class="error-message">Failed to load categories.</div>
      <button @click="reloadCategories" class="reload-btn">Reload</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onActivated, onMounted, ref, watch } from 'vue'
import Cate1List from '../CommonCategory/components/Cate1List.vue'
import Cate2Grid from '../CommonCategory/components/Cate2Grid.vue'
import { useCategories } from './composables/useCategories'
import { useSelection } from './composables/useSelection'
import type { CategorySelectedEvent, Category2 } from './types'
import type { Category1 as CommonCategory1, Category2 as CommonCategory2 } from '../../platforms/common/categoryTypes.ts'

const props = defineProps<{
  preferredShortName?: string | null
  isCate2Subscribed?: (shortName: string) => boolean
}>()

const emit = defineEmits<{
  (e: 'category-selected', category: CategorySelectedEvent): void
  (e: 'subscribe-cate2', category: CategorySelectedEvent): void
}>()

const hasError = ref(false)
const isLoading = ref(true)

const {
  selectedCate1Id,
  selectedCate2Id,
  selectCate1,
  handleCate2Click: originalHandleCate2Click,
  resetSelection,
} = useSelection(emit as (event: 'category-selected', ...args: any[]) => void)

const {
  cate1List,
  cate2List,
  fetchCategories,
  sortedCate2List,
} = useCategories(selectedCate1Id, selectedCate2Id)

const cate1ListForCommon = computed(() => cate1List.value.map((cate1) => ({
  title: cate1.cate1Name,
  href: String(cate1.cate1Id),
  subcategories: cate2List.value
    .filter((cate2) => cate2.cate1Id === cate1.cate1Id)
    .map((cate2) => ({
      title: cate2.cate2Name,
      href: String(cate2.cate2Id),
    })),
})))

const cate2ListForCommon = computed(() => sortedCate2List.value.map((cate2) => ({
  title: cate2.cate2Name,
  href: String(cate2.cate2Id),
})))

const selectedCate1Href = computed(() => selectedCate1Id.value != null ? String(selectedCate1Id.value) : null)
const selectedCate2Href = computed(() => selectedCate2Id.value != null ? String(selectedCate2Id.value) : null)

const applyPreferredSelection = () => {
  const preferredShortName = (props.preferredShortName ?? '').trim()
  if (!preferredShortName || !cate2List.value.length) return false
  const match = cate2List.value.find((item) => item.shortName === preferredShortName)
  if (!match) return false
  if (selectedCate1Id.value !== match.cate1Id) {
    selectCate1(match.cate1Id)
  }
  originalHandleCate2Click(match)
  return true
}

const handleCate1SelectFromCommon = (cate1: CommonCategory1) => {
  const cate1Id = Number(cate1.href)
  if (!Number.isNaN(cate1Id)) {
    selectCate1(cate1Id)
  }
}

const handleCate2SelectAndEmit = (cate2: Category2) => {
  originalHandleCate2Click(cate2)
}

const handleCate2SelectFromCommon = (cate2: CommonCategory2) => {
  const cate2Id = Number(cate2.href)
  if (Number.isNaN(cate2Id)) return
  const match = sortedCate2List.value.find((item) => item.cate2Id === cate2Id)
  if (match) {
    handleCate2SelectAndEmit(match)
  }
}

const handleCate2SubscribeFromCommon = (cate2: CommonCategory2) => {
  const cate2Id = Number(cate2.href)
  if (Number.isNaN(cate2Id)) return
  const match = sortedCate2List.value.find((item) => item.cate2Id === cate2Id)
  if (!match) return
  emit('subscribe-cate2', {
    type: 'cate2',
    cate2Id: match.cate2Id,
    shortName: match.shortName,
    cate2Name: match.cate2Name,
  })
}

const isCate2SubscribedItem = (cate2: CommonCategory2) => {
  const cate2Id = Number(cate2.href)
  if (Number.isNaN(cate2Id)) return false
  const match = sortedCate2List.value.find((item) => item.cate2Id === cate2Id)
  return !!match && (props.isCate2Subscribed?.(match.shortName) ?? false)
}

const loadCategories = async () => {
  isLoading.value = true
  hasError.value = false
  resetSelection()
  try {
    await fetchCategories()
    isLoading.value = false
    if (!applyPreferredSelection() && cate1List.value[0]) {
      selectCate1(cate1List.value[0].cate1Id)
      if (sortedCate2List.value[0]) {
        handleCate2SelectAndEmit(sortedCate2List.value[0])
      }
    }
  } catch (error) {
    console.error('[DouyuCategory] load failed:', error)
    hasError.value = true
    isLoading.value = false
  }
}

const reloadCategories = () => {
  loadCategories()
}

onMounted(() => {
  loadCategories()
})

onActivated(() => {
  if (!applyPreferredSelection() && sortedCate2List.value[0] && selectedCate2Id.value == null) {
    handleCate2SelectAndEmit(sortedCate2List.value[0])
  }
})

watch(
  () => props.preferredShortName,
  () => {
    nextTick(() => {
      applyPreferredSelection()
    })
  },
)

watch(sortedCate2List, (newList) => {
  if (newList.length > 0 && selectedCate2Id.value == null) {
    handleCate2SelectAndEmit(newList[0])
  }
})
</script>

<style scoped>
.category-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 6px 8px 5px;
  background: rgba(58, 62, 68, 0.72);
  backdrop-filter: blur(14px) saturate(1.02);
  -webkit-backdrop-filter: blur(14px) saturate(1.02);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.loading-state,
.error-state {
  padding: 20px 12px 16px;
  text-align: center;
}

.loading-spinner {
  width: 22px;
  height: 22px;
  border: 2px solid var(--border-color);
  border-top-color: var(--accent-color);
  border-radius: 50%;
  margin: 0 auto 10px;
}

.loading-text,
.error-message {
  font-size: 12px;
  color: var(--secondary-text);
}

.reload-btn {
  margin-top: 10px;
  padding: 6px 14px;
  border: none;
  border-radius: 999px;
  background: var(--accent-color);
  color: var(--accent-text);
}

:root[data-theme="light"] .category-list {
  background: rgba(226, 232, 240, 0.88);
  border-bottom-color: rgba(148, 163, 184, 0.28);
}
</style>
