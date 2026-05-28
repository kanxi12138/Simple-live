<template>
  <div class="category-list">
    <template v-if="cate1List.length > 0">
      <Cate1List
        :cate1-list="cate1List"
        :selected-cate1-href="selectedCate1Href"
        @select="selectCate1"
      />
      <Cate2Grid
        v-if="currentCate2List.length > 0"
        :cate2-list="currentCate2List"
        :selected-cate2-href="selectedCate2Href"
        :is-subscribed="isCate2SubscribedItem"
        @select="handleCate2Select"
        @subscribe="handleCate2Subscribe"
      />
    </template>
    <div v-else class="loading-state">
      <div class="loading-text">Loading categories...</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onActivated, onMounted, ref, watch } from 'vue'
import Cate1List from './components/Cate1List.vue'
import Cate2Grid from './components/Cate2Grid.vue'
import type { Category1 as CommonCategory1, Category2 as CommonCategory2, CategorySelectedEvent } from '../../platforms/common/categoryTypes.ts'

const props = defineProps<{
  categoriesData: CommonCategory1[]
  preferredCate1Href?: string | null
  preferredCate2Href?: string | null
  isCate2Subscribed?: (cate2Href: string) => boolean
}>()

const emit = defineEmits<{
  (e: 'category-selected', category: CategorySelectedEvent): void
  (e: 'subscribe-cate2', category: CategorySelectedEvent): void
}>()

const cate1List = ref<CommonCategory1[]>([])
const selectedCate1Href = ref<string | null>(null)
const selectedCate2Href = ref<string | null>(null)

const currentCate2List = computed(() => {
  const cate1 = cate1List.value.find((item) => item.href === selectedCate1Href.value)
  return cate1?.subcategories ?? []
})

const emitCate2Selection = (cate1: CommonCategory1, cate2: CommonCategory2) => {
  emit('category-selected', {
    type: 'cate2',
    cate1Href: cate1.href,
    cate2Href: cate2.href,
    cate1Name: cate1.title,
    cate2Name: cate2.title,
  })
}

const applyPreferredSelection = () => {
  const preferredCate1Href = (props.preferredCate1Href ?? '').trim()
  const preferredCate2Href = (props.preferredCate2Href ?? '').trim()
  if (!cate1List.value.length || (!preferredCate1Href && !preferredCate2Href)) {
    return false
  }

  let targetCate1 = preferredCate1Href
    ? cate1List.value.find((item) => item.href === preferredCate1Href)
    : undefined

  if (!targetCate1 && preferredCate2Href) {
    targetCate1 = cate1List.value.find((item) => item.subcategories.some((sub) => sub.href === preferredCate2Href))
  }

  if (!targetCate1) return false

  const targetCate2 = preferredCate2Href
    ? targetCate1.subcategories.find((item) => item.href === preferredCate2Href)
    : targetCate1.subcategories[0]

  if (!targetCate2) return false

  selectedCate1Href.value = targetCate1.href
  selectedCate2Href.value = targetCate2.href
  emitCate2Selection(targetCate1, targetCate2)
  return true
}

const selectCate1 = (cate1: CommonCategory1) => {
  if (selectedCate1Href.value === cate1.href) return
  selectedCate1Href.value = cate1.href
  const fallbackCate2 = cate1.subcategories[0] ?? null
  selectedCate2Href.value = fallbackCate2?.href ?? null
  if (fallbackCate2) {
    emitCate2Selection(cate1, fallbackCate2)
  }
}

const handleCate2Select = (cate2: CommonCategory2) => {
  selectedCate2Href.value = cate2.href
  const cate1 = cate1List.value.find((item) => item.href === selectedCate1Href.value)
  if (cate1) {
    emitCate2Selection(cate1, cate2)
  }
}

const handleCate2Subscribe = (cate2: CommonCategory2) => {
  const cate1 = cate1List.value.find((item) => item.href === selectedCate1Href.value)
  if (!cate1) return
  emit('subscribe-cate2', {
    type: 'cate2',
    cate1Href: cate1.href,
    cate2Href: cate2.href,
    cate1Name: cate1.title,
    cate2Name: cate2.title,
  })
}

const isCate2SubscribedItem = (cate2: CommonCategory2) => props.isCate2Subscribed?.(cate2.href) ?? false

onMounted(() => {
  cate1List.value = Array.isArray(props.categoriesData) ? props.categoriesData : []
  if (!applyPreferredSelection() && cate1List.value[0]) {
    selectCate1(cate1List.value[0])
  }
})

onActivated(() => {
  if (!applyPreferredSelection()) {
    const cate1 = cate1List.value.find((item) => item.href === selectedCate1Href.value)
    const cate2 = currentCate2List.value.find((item) => item.href === selectedCate2Href.value)
    if (cate1 && cate2) {
      emitCate2Selection(cate1, cate2)
    }
  }
})

watch(
  () => [props.preferredCate1Href, props.preferredCate2Href],
  () => {
    nextTick(() => {
      applyPreferredSelection()
    })
  },
)
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

.loading-state {
  padding: 20px 12px 16px;
  text-align: center;
}

.loading-text {
  font-size: 12px;
  color: var(--secondary-text);
}

:root[data-theme="light"] .category-list {
  background: rgba(226, 232, 240, 0.88);
  border-bottom-color: rgba(148, 163, 184, 0.28);
}
</style>
