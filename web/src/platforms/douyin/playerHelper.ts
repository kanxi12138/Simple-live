import { invoke } from '@tauri-apps/api/core';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';
import { Ref } from 'vue';
import { Platform } from '../common/types';
import type { DanmakuMessage, DanmuOverlayInstance, DanmuOverlayResolver, DanmuRenderOptions, RustGetStreamUrlPayload } from '../../components/player/types';
import type { LiveStreamInfo, StreamVariant } from '../common/types';
import { v4 as uuidv4 } from 'uuid';

const isAndroidRuntime = typeof navigator !== 'undefined' && /android/i.test(navigator.userAgent || '');


export interface DouyinRustDanmakuPayload {
  room_id?: string; 
  user: string;      // Nickname from Rust's DanmakuFrontendPayload
  content: string;
  user_level: number; // from Rust's i64
  fans_club_level: number; // from Rust's i32
}

export async function fetchAndPrepareDouyinStreamConfig(roomId: string, quality: string = '原画'): Promise<{ 
  streamUrl: string | null;
  streamType: string | undefined; 
  title?: string | null; 
  anchorName?: string | null; 
  avatar?: string | null; 
  isLive: boolean; 
  initialError: string | null; // Made non-optional, will always be string or null
}> {
  if (!roomId) {
    return { streamUrl: null, streamType: undefined, title: null, anchorName: null, avatar: null, isLive: false, initialError: '房间ID未提供' };
  }

  try {
    const payloadData = { args: { room_id_str: roomId } };
    const backendQuality = normalizeDouyinQuality(quality);
    // 使用画质参数调用抖音画质切换API
    const result = await invoke<LiveStreamInfo>('get_douyin_live_stream_url_with_quality', { 
      payload: payloadData,
      quality: backendQuality 
    });

    if (result.error_message) {
      console.error(`[DouyinPlayerHelper] Error from backend for room ${roomId}: ${result.error_message}`);
      return {
        streamUrl: null,
        streamType: undefined,
        title: result.title,
        anchorName: result.anchor_name,
        avatar: result.avatar,
        isLive: result.status === 2,
        initialError: result.error_message, // string | null from Rust
      };
    }

    const streamAvailable = result.status === 2 && !!result.stream_url;
    let streamType: string | undefined = undefined;
    let uiMessage: string | null = null; 

    const androidHlsVariant = streamAvailable ? selectAndroidDouyinHlsVariant(result.available_streams, backendQuality) : null;
    const rawStreamUrl = (isAndroidRuntime && androidHlsVariant?.url) ? androidHlsVariant.url : (result.stream_url ?? null);
    const sanitizedStreamUrl = streamAvailable && rawStreamUrl ? enforceHttps(rawStreamUrl) : null;

    if (streamAvailable && rawStreamUrl) {
      const lowerUrl = rawStreamUrl.toLowerCase();
      if (isAndroidRuntime && androidHlsVariant?.url) {
        streamType = 'hls';
      } else if (rawStreamUrl.startsWith('http://127.0.0.1') && rawStreamUrl.endsWith('/live.flv')) {
        streamType = 'flv';
      } else if (lowerUrl.includes('pull-hls') || lowerUrl.endsWith('.m3u8')) {
        streamType = 'hls';
      } else if (lowerUrl.includes('pull-flv') || lowerUrl.includes('.flv')) {
        streamType = 'flv';
      } else {
        console.warn(`[DouyinPlayerHelper] Could not determine stream type for URL: ${rawStreamUrl}. Defaulting to ${isAndroidRuntime ? 'hls' : 'flv'}.`);
        streamType = isAndroidRuntime ? 'hls' : 'flv';
      }
      // uiMessage remains null if stream is available and no prior error.
    } else {
      if (result.status !== 2) {
        uiMessage = result.title ? `主播 ${result.anchor_name || ''} 未开播。` : '主播未开播或房间不存在。';
      } else {
        uiMessage = '主播在线，但获取直播流失败。';
      }
    }

    return {
      streamUrl: sanitizedStreamUrl,
      streamType: streamType,
      title: result.title,
      anchorName: result.anchor_name,
      avatar: result.avatar,
      isLive: streamAvailable,
      initialError: uiMessage, // uiMessage is definitely string or null here.
    };

  } catch (e: any) {
    console.error(`[DouyinPlayerHelper] Exception while fetching Douyin stream details for ${roomId}:`, e);
    return { 
        streamUrl: null, 
        streamType: undefined, 
        title: null, 
        anchorName: null, 
        avatar: null, 
        isLive: false, 
        initialError: e.message || '获取直播信息失败: 未知错误' // Ensure string here
    };
  }
}

function normalizeDouyinQuality(input: string): string {
  const upper = input.trim().toUpperCase();
  if (upper === 'OD' || upper === '原画') return 'OD';
  if (upper === 'BD' || upper === '标清') return 'BD';
  if (upper === 'UHD' || upper === '高清') return 'UHD';
  return 'OD';
}

function selectAndroidDouyinHlsVariant(
  variants: StreamVariant[] | null | undefined,
  requestedQuality: string,
): StreamVariant | null {
  if (!isAndroidRuntime || !Array.isArray(variants) || variants.length === 0) {
    return null;
  }

  const hlsVariants = variants.filter((variant) => {
    const url = (variant?.url || '').toLowerCase();
    const format = (variant?.format || '').toLowerCase();
    const protocol = (variant?.protocol || '').toLowerCase();
    return Boolean(url) && (
      format === 'hls' ||
      protocol === 'hls' ||
      url.includes('.m3u8') ||
      url.includes('pull-hls')
    );
  });

  if (hlsVariants.length === 0) {
    return null;
  }

  const preferredKeys = requestedQuality === 'OD'
    ? ['ORIGIN', 'FULL_HD1']
    : requestedQuality === 'UHD'
      ? ['FULL_HD1', 'ORIGIN']
      : ['HD1', 'SD1', 'SD2'];

  for (const key of preferredKeys) {
    const matched = hlsVariants.find((variant) => (variant.desc || '').toUpperCase() === key);
    if (matched?.url) {
      return matched;
    }
  }

  return hlsVariants[0] || null;
}

export async function startDouyinDanmakuListener(
  roomId: string,
  danmuOverlay: DanmuOverlayResolver, // For emitting danmaku to overlay
  danmakuMessagesRef: Ref<DanmakuMessage[]>, // For updating DanmuList
  renderOptions?: DanmuRenderOptions
): Promise<() => void> {
  const resolveOverlay = (): DanmuOverlayInstance | null =>
    typeof danmuOverlay === 'function' ? danmuOverlay() : danmuOverlay;
  
  const rustPayload: RustGetStreamUrlPayload = { 
    args: { room_id_str: roomId }, 
    platform: Platform.DOUYIN, 
  };
  await invoke('start_douyin_danmu_listener', { payload: rustPayload });
  
  const eventName = 'danmaku-message';

  const unlisten = await listen<DouyinRustDanmakuPayload>(eventName, (event: TauriEvent<DouyinRustDanmakuPayload>) => {
    if (event.payload) {
      const rustP = event.payload;
      const frontendDanmaku: DanmakuMessage = {
        id: uuidv4(),
        nickname: rustP.user || '未知用户',
        content: rustP.content || '',
        level: String(rustP.user_level || 0),
        badgeLevel: rustP.fans_club_level > 0 ? String(rustP.fans_club_level) : undefined,
        room_id: rustP.room_id || roomId, // Ensure room_id is present
      };

      const shouldDisplay = renderOptions?.shouldDisplay ? renderOptions.shouldDisplay(frontendDanmaku) : true;
      const activeOverlay = resolveOverlay();

      if (shouldDisplay && activeOverlay?.sendComment) {
        try {
          const commentOptions = renderOptions?.buildCommentOptions?.(frontendDanmaku) ?? {};
          const styleFromOptions = commentOptions.style ?? {};
          const preferredColor = styleFromOptions.color || frontendDanmaku.color || '#FFFFFF';
          activeOverlay.sendComment({
            id: frontendDanmaku.id,
            txt: frontendDanmaku.content,
            duration: commentOptions.duration ?? 12000,
            mode: commentOptions.mode ?? 'scroll',
            style: {
              ...styleFromOptions,
              color: preferredColor,
            },
          });
        } catch (emitError) {
          console.warn('[DouyinPlayerHelper] Failed emitting danmu.js comment:', emitError);
        }
      }
      const shouldAppend = renderOptions?.shouldAppendToList ? renderOptions.shouldAppendToList(frontendDanmaku) : true;
      if (shouldAppend) {
        danmakuMessagesRef.value.push(frontendDanmaku);
        if (danmakuMessagesRef.value.length > 200) { // Manage danmaku array size
          danmakuMessagesRef.value.splice(0, danmakuMessagesRef.value.length - 200);
        }
      }
    }
  });
  return unlisten;
}

export async function stopDouyinDanmaku(currentUnlistenFn: (() => void) | null): Promise<void> {
  if (currentUnlistenFn) {
    currentUnlistenFn();
  }
  try {
    const rustPayload: RustGetStreamUrlPayload = { 
      args: { room_id_str: "stop_listening" }, 
      platform: Platform.DOUYIN, 
    };
    await invoke('start_douyin_danmu_listener', { payload: rustPayload });
  } catch (error) {
    console.error('[DouyinPlayerHelper] Error stopping Douyin danmaku listener:', error);
  }
}

function enforceHttps(url: string): string {
  if (!url) {
    return url;
  }
  if (url.startsWith('https://')) {
    return url;
  }
  if (url.startsWith('http://')) {
    return `https://${url.slice('http://'.length)}`;
  }
  return url;
}
