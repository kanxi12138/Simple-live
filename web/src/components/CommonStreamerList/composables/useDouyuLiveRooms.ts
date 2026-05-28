import { ref } from 'vue';
import type { Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { CommonStreamer } from '../../../platforms/common/streamerTypes';

interface DouyuStreamer {
  rid: string;
  roomName: string;
  nickname: string;
  roomSrc: string;
  avatar: string;
  hn: string;
  isLive?: boolean;
}

interface LiveListDataWrapper {
  list: DouyuStreamer[];
  total?: number;
  page_count?: number;
}

interface LiveListApiResponse {
  error: number;
  msg?: string;
  data?: LiveListDataWrapper;
}

const PAGE_SIZE = 20;

export function useDouyuLiveRooms(
  categoryTypeRef: Ref<'cate2' | 'cate3' | null>,
  categoryIdRef: Ref<string | null>
) {
  const rooms = ref<CommonStreamer[]>([]);
  const isLoading = ref(false);
  const isLoadingMore = ref(false);
  const hasMore = ref(true);
  const currentPage = ref(0);
  const lastKnownTotal = ref<number | null>(null);
  const lastKnownPageCount = ref<number | null>(null);

  const mapDouyuItemToCommon = (item: DouyuStreamer): CommonStreamer => ({
    room_id: item.rid?.toString() || '',
    title: item.roomName || '',
    nickname: item.nickname || '',
    avatar: item.avatar || '',
    room_cover: item.roomSrc || '',
    viewer_count_str: item.hn || '0',
    platform: 'douyu',
  });

  const getRandomInt = (min: number, max: number) => {
    if (max <= min) return min;
    return Math.floor(Math.random() * (max - min + 1)) + min;
  };

  const getRoomKey = (room: CommonStreamer) => (
    room.room_id || `${room.platform}:${room.nickname}:${room.title}`
  );

  const appendUniqueRooms = (incomingRooms: CommonStreamer[]) => {
    const existingKeys = new Set(rooms.value.map(getRoomKey));
    const uniqueRooms = incomingRooms.filter((room) => {
      const roomKey = getRoomKey(room);
      if (existingKeys.has(roomKey)) {
        return false;
      }
      existingKeys.add(roomKey);
      return true;
    });
    rooms.value = [...rooms.value, ...uniqueRooms];
  };

  const resolveMaxRandomPage = () => {
    if (lastKnownPageCount.value && lastKnownPageCount.value > 1) {
      return lastKnownPageCount.value - 1;
    }
    if (lastKnownTotal.value && lastKnownTotal.value > PAGE_SIZE) {
      return Math.ceil(lastKnownTotal.value / PAGE_SIZE) - 1;
    }
    if (hasMore.value) {
      return Math.max(currentPage.value + 2, 2);
    }
    return 0;
  };

  const fetchRooms = async (pageToFetch: number, isLoadMore: boolean) => {
    const categoryType = categoryTypeRef.value;
    const categoryId = categoryIdRef.value;
    if (!categoryType || !categoryId) {
      rooms.value = [];
      hasMore.value = false;
      currentPage.value = 0;
      return;
    }

    if (isLoadMore) isLoadingMore.value = true;
    else isLoading.value = true;

    let command = '';
    let params: Record<string, unknown> = {};
    if (categoryType === 'cate2') {
      command = 'fetch_live_list';
      params = { cate2: categoryId, offset: pageToFetch * PAGE_SIZE, limit: PAGE_SIZE };
    } else {
      command = 'fetch_live_list_for_cate3';
      params = { cate3Id: categoryId, page: pageToFetch + 1, limit: PAGE_SIZE };
    }

    try {
      const resp = await invoke<LiveListApiResponse>(command, params);
      if (resp.error !== 0 || !resp.data) {
        throw new Error(resp.msg || '斗鱼接口返回错误');
      }

      lastKnownTotal.value = typeof resp.data.total === 'number' ? resp.data.total : null;
      lastKnownPageCount.value = typeof resp.data.page_count === 'number' ? resp.data.page_count : null;

      const newRooms = (resp.data.list || []).map(mapDouyuItemToCommon);
      if (isLoadMore) rooms.value = [...rooms.value, ...newRooms];
      else rooms.value = newRooms;

      if (resp.data.total !== undefined) {
        const totalFetched = (pageToFetch + 1) * PAGE_SIZE;
        hasMore.value = resp.data.total > totalFetched && newRooms.length > 0;
      } else if (resp.data.page_count !== undefined) {
        hasMore.value = pageToFetch + 1 < resp.data.page_count && newRooms.length > 0;
      } else {
        hasMore.value = newRooms.length === PAGE_SIZE;
      }

      currentPage.value = pageToFetch;
    } catch (e) {
      console.error('[useDouyuLiveRooms] invoke error', e);
      if (pageToFetch === 0) rooms.value = [];
      hasMore.value = false;
    } finally {
      if (isLoadMore) isLoadingMore.value = false;
      else isLoading.value = false;
    }
  };

  const loadInitialRooms = async () => {
    rooms.value = [];
    hasMore.value = true;
    currentPage.value = 0;
    await fetchRooms(0, false);
  };

  const refreshRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return;

    const maxPage = resolveMaxRandomPage();
    if (maxPage < 1) {
      await loadInitialRooms();
      return;
    }

    const randomPage = getRandomInt(1, maxPage);
    await fetchRooms(randomPage, false);

    if (rooms.value.length === 0) {
      await loadInitialRooms();
    }
  };

  const appendRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return;

    const maxPage = resolveMaxRandomPage();
    if (maxPage < 1) {
      await loadMoreRooms();
      return;
    }

    const randomPage = getRandomInt(1, maxPage);
    const categoryType = categoryTypeRef.value;
    const categoryId = categoryIdRef.value;
    if (!categoryType || !categoryId) return;

    if (isLoadingMore.value) return;
    isLoadingMore.value = true;

    let command = '';
    let params: Record<string, unknown> = {};
    if (categoryType === 'cate2') {
      command = 'fetch_live_list';
      params = { cate2: categoryId, offset: randomPage * PAGE_SIZE, limit: PAGE_SIZE };
    } else {
      command = 'fetch_live_list_for_cate3';
      params = { cate3Id: categoryId, page: randomPage + 1, limit: PAGE_SIZE };
    }

    try {
      const resp = await invoke<LiveListApiResponse>(command, params);
      if (resp.error !== 0 || !resp.data) {
        throw new Error(resp.msg || '斗鱼接口返回错误');
      }
      const newRooms = (resp.data.list || []).map(mapDouyuItemToCommon);
      appendUniqueRooms(newRooms);
    } catch (e) {
      console.error('[useDouyuLiveRooms] append invoke error', e);
    } finally {
      isLoadingMore.value = false;
    }
  };

  const loadMoreRooms = async () => {
    if (!hasMore.value || isLoading.value || isLoadingMore.value) return;
    await fetchRooms(currentPage.value + 1, true);
  };

  return {
    rooms,
    isLoading,
    isLoadingMore,
    hasMore,
    loadInitialRooms,
    refreshRandomRooms,
    appendRandomRooms,
    loadMoreRooms,
  };
}
