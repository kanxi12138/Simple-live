import { ref } from 'vue';
import type { Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { CommonStreamer } from '../../../platforms/common/streamerTypes';

export function useDouyinLiveRooms(
  partitionId: Ref<string | null>,
  partitionTypeId: Ref<string | null>
) {
  const rooms = ref<CommonStreamer[]>([]) as Ref<CommonStreamer[]>;
  const isLoading = ref(false);
  const isLoadingMore = ref(false);
  const error = ref<string | null>(null);
  const currentOffset = ref(0);
  const hasMore = ref(true);
  const currentMsToken = ref<string | null>(null);

  interface DouyinRoomsPage {
    rooms: CommonStreamer[];
    hasMore: boolean;
    nextOffset: number;
  }

  const fetchAndSetMsToken = async () => {
    try {
      currentMsToken.value = await invoke<string>('generate_douyin_ms_token');
    } catch (e) {
      console.error('[useDouyinLiveRoomsCommon] Failed to fetch msToken:', e);
      error.value = 'Failed to initialize session token.';
      currentMsToken.value = null;
      return false;
    }
    return true;
  };

  const mapRawRoomToCommonStreamer = (rawRoom: any): CommonStreamer => {
    const webId = rawRoom.web_rid?.toString?.() || '';
    return {
      room_id: webId || `N/A_RID_${Math.random()}`,
      title: rawRoom.title || '未知标题',
      nickname: rawRoom.owner_nickname || '未知主播',
      avatar: rawRoom.avatar_url || '',
      room_cover: rawRoom.cover_url || 'https://via.placeholder.com/320x180.png?text=No+Image',
      viewer_count_str: rawRoom.user_count_str || '0 人',
      platform: 'douyin',
      web_id: webId,
    };
  };

  const normalizeOffset = (value: unknown, fallback: number) => {
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (typeof value === 'string') {
      const parsed = Number(value);
      if (Number.isFinite(parsed)) return parsed;
    }
    return fallback;
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

  const fetchRoomsPage = async (offset: number): Promise<DouyinRoomsPage> => {
    const response = await invoke<any>('fetch_douyin_partition_rooms', {
      partition: partitionId.value,
      partitionType: partitionTypeId.value,
      offset,
      msToken: currentMsToken.value,
    });

    if (!response || !Array.isArray(response.rooms)) {
      return {
        rooms: [],
        hasMore: false,
        nextOffset: offset,
      };
    }

    const mappedRooms = response.rooms.map(mapRawRoomToCommonStreamer);
    return {
      rooms: mappedRooms,
      hasMore: Boolean(response.has_more),
      nextOffset: normalizeOffset(response.next_offset, offset + mappedRooms.length),
    };
  };

  const applyRoomsPage = (page: DouyinRoomsPage, isLoadMore: boolean) => {
    if (isLoadMore) {
      rooms.value.push(...page.rooms);
    } else {
      rooms.value = page.rooms;
    }
    hasMore.value = page.hasMore;
    currentOffset.value = page.nextOffset;
  };

  const fetchRooms = async (offset: number, isLoadMore: boolean = false) => {
    if (!partitionId.value || !partitionTypeId.value) {
      rooms.value = [];
      currentOffset.value = 0;
      hasMore.value = false;
      return;
    }

    if (!currentMsToken.value) {
      console.error('[useDouyinLiveRoomsCommon] msToken is not set. Aborting fetchRooms.');
      error.value = 'Session token is missing. Please refresh or select category again.';
      if (!isLoadMore) isLoading.value = false;
      else isLoadingMore.value = false;
      hasMore.value = false;
      return;
    }

    if (isLoadMore) {
      isLoadingMore.value = true;
    } else {
      isLoading.value = true;
    }
    error.value = null;

    try {
      const page = await fetchRoomsPage(offset);

      if (page.rooms.length > 0) {
        applyRoomsPage(page, isLoadMore);
      } else {
        console.warn('[useDouyinLiveRoomsCommon] No rooms array in response or invalid structure (expected response.rooms to be an array).');
        if (!isLoadMore) rooms.value = [];
        hasMore.value = false;
      }
    } catch (e: any) {
      console.error('[useDouyinLiveRoomsCommon] Error fetching rooms:', e);
      error.value = typeof e === 'string' ? e : (e?.message || 'Failed to fetch rooms');
      if (!isLoadMore) {
        hasMore.value = false;
        rooms.value = [];
      }
    } finally {
      if (isLoadMore) {
        isLoadingMore.value = false;
      } else {
        isLoading.value = false;
      }
    }
  };

  const loadInitialRooms = async () => {
    currentOffset.value = 0;
    hasMore.value = true;
    isLoading.value = true;
    error.value = null;
    rooms.value = [];

    const tokenFetched = await fetchAndSetMsToken();
    if (tokenFetched && currentMsToken.value) {
      await fetchRooms(0, false);
      if (hasMore.value && rooms.value.length > 0 && rooms.value.length <= 15) {
        await fetchRooms(currentOffset.value, true);
      }
    } else {
      if (!error.value) error.value = 'Failed to initialize session. Cannot load rooms.';
      isLoading.value = false;
      hasMore.value = false;
    }
  };

  const loadMoreRooms = () => {
    if (hasMore.value && !isLoading.value && !isLoadingMore.value && currentMsToken.value) {
      fetchRooms(currentOffset.value, true);
    }
  };

  const refreshRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return;

    currentOffset.value = 0;
    hasMore.value = true;
    isLoading.value = true;
    error.value = null;
    rooms.value = [];

    try {
      const tokenFetched = await fetchAndSetMsToken();
      if (!tokenFetched || !currentMsToken.value) {
        hasMore.value = false;
        return;
      }

      const firstPage = await fetchRoomsPage(0);
      const candidatePages: DouyinRoomsPage[] = [];
      let probePage = firstPage;
      let probesRemaining = 3;

      while (probePage.hasMore && probePage.nextOffset > 0 && probesRemaining > 0) {
        const nextPage = await fetchRoomsPage(probePage.nextOffset);
        if (nextPage.rooms.length === 0) {
          break;
        }
        candidatePages.push(nextPage);
        probePage = nextPage;
        probesRemaining -= 1;
      }

      if (candidatePages.length > 0) {
        const randomIndex = Math.floor(Math.random() * candidatePages.length);
        applyRoomsPage(candidatePages[randomIndex], false);
      } else {
        applyRoomsPage(firstPage, false);
      }

      if (rooms.value.length === 0) {
        await fetchRooms(0, false);
      }
    } catch (e: any) {
      console.error('[useDouyinLiveRoomsCommon] Error refreshing random rooms:', e);
      error.value = typeof e === 'string' ? e : (e?.message || 'Failed to refresh rooms');
      hasMore.value = false;
      rooms.value = [];
      await fetchRooms(0, false);
    } finally {
      isLoading.value = false;
    }
  };

  const appendRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return;

    isLoadingMore.value = true;
    error.value = null;

    try {
      const tokenFetched = await fetchAndSetMsToken();
      if (!tokenFetched || !currentMsToken.value) {
        return;
      }

      const firstPage = await fetchRoomsPage(0);
      const candidatePages: DouyinRoomsPage[] = [firstPage];
      let probePage = firstPage;
      let probesRemaining = 3;

      while (probePage.hasMore && probePage.nextOffset > 0 && probesRemaining > 0) {
        const nextPage = await fetchRoomsPage(probePage.nextOffset);
        if (nextPage.rooms.length === 0) {
          break;
        }
        candidatePages.push(nextPage);
        probePage = nextPage;
        probesRemaining -= 1;
      }

      const nonEmptyPages = candidatePages.filter((page) => page.rooms.length > 0);
      if (nonEmptyPages.length === 0) {
        return;
      }

      const randomIndex = Math.floor(Math.random() * nonEmptyPages.length);
      appendUniqueRooms(nonEmptyPages[randomIndex].rooms);
    } catch (e: any) {
      console.error('[useDouyinLiveRoomsCommon] Error appending random rooms:', e);
      error.value = typeof e === 'string' ? e : (e?.message || 'Failed to append rooms');
    } finally {
      isLoadingMore.value = false;
    }
  };

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
  };
}
