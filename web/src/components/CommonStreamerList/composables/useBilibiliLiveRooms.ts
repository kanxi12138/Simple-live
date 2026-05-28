import { ref } from 'vue'
import type { Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { CommonStreamer } from '../../../platforms/common/streamerTypes'
import { useImageProxy } from '../../FollowsList/useProxy'

export function useBilibiliLiveRooms(
  subCategoryId: Ref<string | null>,
  parentCategoryId: Ref<string | null>
) {
  const rooms = ref<CommonStreamer[]>([]) as Ref<CommonStreamer[]>
  const isLoading = ref(false)
  const isLoadingMore = ref(false)
  const error = ref<string | null>(null)
  const currentPage = ref(1)
  const hasMore = ref(true)
  const { ensureProxyStarted, proxify, normalizeImageUrl } = useImageProxy()

  const mapToCommon = (raw: any): CommonStreamer => {
    // Raw structure likely: { uname, title, cover, face, watched_show: { num }, roomid, uid }
    return {
      room_id: String(raw.roomid ?? raw.roomid?.toString() ?? ''),
      title: raw.title ?? '',
      nickname: raw.uname ?? '',
      avatar: proxify(normalizeImageUrl(raw.face ?? '')),
      room_cover: proxify(normalizeImageUrl(raw.cover ?? '')),
      viewer_count_str: (raw.watched_show?.num != null) ? String(raw.watched_show.num) : '',
      platform: 'bilibili',
    }
  }

  const getRandomInt = (min: number, max: number) => {
    if (max <= min) return min
    return Math.floor(Math.random() * (max - min + 1)) + min
  }

  const getRoomKey = (room: CommonStreamer) => (
    room.room_id || `${room.platform}:${room.nickname}:${room.title}`
  )

  const appendUniqueRooms = (incomingRooms: CommonStreamer[]) => {
    const existingKeys = new Set(rooms.value.map(getRoomKey))
    const uniqueRooms = incomingRooms.filter((room) => {
      const roomKey = getRoomKey(room)
      if (existingKeys.has(roomKey)) {
        return false
      }
      existingKeys.add(roomKey)
      return true
    })
    rooms.value = [...rooms.value, ...uniqueRooms]
  }

  const resolveMaxRandomPage = () => {
    if (hasMore.value) {
      return Math.max(currentPage.value + 2, 4)
    }
    return Math.max(currentPage.value - 1, 1)
  }

  const fetchPage = async (page: number, isLoadMore = false) => {
    const areaId = subCategoryId.value
    const parentId = parentCategoryId.value
    if (!areaId || !parentId) {
      rooms.value = []
      hasMore.value = false
      return
    }

    try {
      await ensureProxyStarted()
      if (isLoadMore) isLoadingMore.value = true
      else isLoading.value = true
      error.value = null

      const text = await invoke<string>('fetch_bilibili_live_list', {
        areaId: areaId,
        parentAreaId: parentId,
        page: page,
      })
      const parsed = JSON.parse(text)
      const list: any[] = parsed?.data?.list ?? []
      const newRooms = list.map(mapToCommon)
      if (isLoadMore) rooms.value.push(...newRooms)
      else rooms.value = newRooms

      // Bilibili returns refresh_id or has_more? We estimate by list length
      hasMore.value = newRooms.length > 0
      currentPage.value = page + 1
    } catch (e: any) {
      error.value = typeof e === 'string' ? e : (e?.message || '获取 B 站主播列表失败')
      hasMore.value = false
      if (!isLoadMore) rooms.value = []
    } finally {
      if (isLoadMore) isLoadingMore.value = false
      else isLoading.value = false
    }
  }

  const loadInitialRooms = async () => {
    currentPage.value = 1
    rooms.value = []
    hasMore.value = true
    await fetchPage(1, false)
  }

  const refreshRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return

    const maxPage = resolveMaxRandomPage()
    if (maxPage <= 1) {
      await loadInitialRooms()
      return
    }

    const randomPage = getRandomInt(2, maxPage)
    await fetchPage(randomPage, false)

    if (rooms.value.length === 0) {
      await loadInitialRooms()
    }
  }

  const appendRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return

    const maxPage = resolveMaxRandomPage()
    if (maxPage <= 1) {
      await loadMoreRooms()
      return
    }

    const areaId = subCategoryId.value
    const parentId = parentCategoryId.value
    if (!areaId || !parentId) return

    const randomPage = getRandomInt(2, maxPage)

    try {
      await ensureProxyStarted()
      isLoadingMore.value = true
      error.value = null

      const text = await invoke<string>('fetch_bilibili_live_list', {
        areaId,
        parentAreaId: parentId,
        page: randomPage,
      })
      const parsed = JSON.parse(text)
      const list: any[] = parsed?.data?.list ?? []
      const newRooms = list.map(mapToCommon)
      appendUniqueRooms(newRooms)
    } catch (e: any) {
      error.value = typeof e === 'string' ? e : (e?.message || '获取 B 站主播列表失败')
    } finally {
      isLoadingMore.value = false
    }
  }

  const loadMoreRooms = async () => {
    if (hasMore.value && !isLoading.value && !isLoadingMore.value) {
      await fetchPage(currentPage.value, true)
    }
  }

  return {
    rooms,
    isLoading,
    isLoadingMore,
    error,
    hasMore,
    loadInitialRooms,
    refreshRandomRooms,
    appendRandomRooms,
    loadMoreRooms,
  }
}
