import { ref } from 'vue';
import type { Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { CommonStreamer } from '../../../platforms/common/streamerTypes';
import { useImageProxy } from '../../FollowsList/useProxy';

export interface UseHuyaLiveRoomsOptions {
  defaultPageSize?: number; // platform-specific default, Huya=120
}

export function useHuyaLiveRooms(
  gidRef: Ref<string | null>,
  options: UseHuyaLiveRoomsOptions = { defaultPageSize: 120 }
) {
  const rooms = ref<CommonStreamer[]>([]) as Ref<CommonStreamer[]>;
  const isLoading = ref(false);
  const isLoadingMore = ref(false);
  const error = ref<string | null>(null);
  const currentPage = ref(1);
  const hasMore = ref(true);
  const pageSize = options.defaultPageSize ?? 120;

  const { proxify, ensureProxyStarted } = useImageProxy();

  const huyaCoverParams = 'x-oss-process=image/resize,limit_0,m_fill,w_338,h_190/sharpen,80/format,jpg/interlace,1/quality,q_90';
  const appendHuyaCoverParams = (url: string): string => {
    if (!url) return url;
    if (url.includes('x-oss-process=')) return url;
    return url.includes('?') ? `${url}&${huyaCoverParams}` : `${url}?${huyaCoverParams}`;
  };

  const mapHuyaItemToCommonStreamer = (item: any): CommonStreamer => {
    const viewers = typeof item.lUserCount === 'number' ? item.lUserCount : 0;
    const rawCover = item.room_cover || item.sScreenshot || '';
    return {
      room_id: item.room_id?.toString() || item.lProfileRoom?.toString() || '',
      title: item.title || item.sIntroduction || '',
      nickname: item.nickname || item.sNick || '',
      avatar: proxify(item.avatar || item.sAvatar180 || ''),
      room_cover: proxify(appendHuyaCoverParams(rawCover)),
      viewer_count_str: item.viewer_count_str || (viewers ? `${viewers}` : '0'),
      platform: 'huya',
    };
  };

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
    if (hasMore.value) {
      return Math.max(currentPage.value + 2, 4);
    }
    return Math.max(currentPage.value - 1, 1);
  };

  const fetchRooms = async (pageNo: number, isLoadMore: boolean = false) => {
    const gid = gidRef.value;
    if (!gid) {
      rooms.value = [];
      hasMore.value = false;
      currentPage.value = 1;
      return;
    }

    if (isLoadMore) isLoadingMore.value = true; else isLoading.value = true;
    error.value = null;

    // Ensure proxy server is started before mapping covers
    await ensureProxyStarted();

    try {
      const resp = await invoke<{ error: number; msg?: string; data?: any[] }>('fetch_huya_live_list', {
        iGid: gid,
        iPageNo: pageNo,
        iPageSize: pageSize,
      });

      if (resp.error !== 0 || !Array.isArray(resp.data)) {
        throw new Error(resp.msg || '虎牙接口返回错误');
      }
      const newRooms = resp.data.map(mapHuyaItemToCommonStreamer);

      if (isLoadMore) {
        rooms.value.push(...newRooms);
      } else {
        rooms.value = newRooms;
      }

      hasMore.value = newRooms.length === pageSize;
      currentPage.value = pageNo + 1;

    } catch (e: any) {
      console.error('[useHuyaLiveRooms] invoke error', e);
      error.value = e?.message || '加载失败';
      if (!isLoadMore) {
        rooms.value = [];
        hasMore.value = false;
      }
    } finally {
      if (isLoadMore) isLoadingMore.value = false; else isLoading.value = false;
    }
  };

  const loadInitialRooms = async () => {
    rooms.value = [];
    hasMore.value = true;
    currentPage.value = 1;
    error.value = null;
    isLoading.value = true;
    await fetchRooms(1, false);
  };

  const refreshRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return;

    const maxPage = resolveMaxRandomPage();
    if (maxPage <= 1) {
      await loadInitialRooms();
      return;
    }

    const randomPage = getRandomInt(2, maxPage);
    await fetchRooms(randomPage, false);

    if (rooms.value.length === 0) {
      await loadInitialRooms();
    }
  };

  const appendRandomRooms = async () => {
    if (isLoading.value || isLoadingMore.value) return;

    const maxPage = resolveMaxRandomPage();
    if (maxPage <= 1) {
      await loadMoreRooms();
      return;
    }

    const gid = gidRef.value;
    if (!gid) return;

    const randomPage = getRandomInt(2, maxPage);
    isLoadingMore.value = true;
    error.value = null;

    await ensureProxyStarted();

    try {
      const resp = await invoke<{ error: number; msg?: string; data?: any[] }>('fetch_huya_live_list', {
        iGid: gid,
        iPageNo: randomPage,
        iPageSize: pageSize,
      });

      if (resp.error !== 0 || !Array.isArray(resp.data)) {
        throw new Error(resp.msg || '虎牙接口返回错误');
      }

      const newRooms = resp.data.map(mapHuyaItemToCommonStreamer);
      appendUniqueRooms(newRooms);
    } catch (e: any) {
      console.error('[useHuyaLiveRooms] append invoke error', e);
      error.value = e?.message || '加载失败';
    } finally {
      isLoadingMore.value = false;
    }
  };

  const loadMoreRooms = async () => {
    if (hasMore.value && !isLoading.value && !isLoadingMore.value) {
      await fetchRooms(currentPage.value, true);
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
