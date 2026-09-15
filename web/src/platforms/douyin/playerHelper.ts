import { invoke } from '../../services/platformInvoke';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';
import { Ref } from 'vue';
import { Platform } from '../common/types';
import type { DanmakuMessage, DanmuOverlayInstance, DanmuOverlayResolver, DanmuRenderOptions, RustGetStreamUrlPayload } from '../../components/player/types';
import type { LiveStreamInfo } from '../common/types';
import { v4 as uuidv4 } from 'uuid';

import { selectPlaybackVariant, type PlaybackConfig } from '../common/playback';


export interface DouyinRustDanmakuPayload {
  room_id?: string; 
  user: string;      // Nickname from Rust's DanmakuFrontendPayload
  content: string;
  user_level: number; // from Rust's i64
  fans_club_level: number; // from Rust's i32
}

/**
 * Resolves a Douyin room and maps actual SDK qualities to the existing controls.
 * @param roomId Numeric room identifier.
 * @param quality Requested quality label.
 * @param line Requested returned line key.
 * @returns Playback metadata or an actionable error.
 * @throws No exceptions; failures are returned in initialError.
 */
export async function fetchAndPrepareDouyinStreamConfig(
  roomId: string,
  quality = '原画',
  line?: string | null,
): Promise<PlaybackConfig & {
  title?: string | null; anchorName?: string | null; avatar?: string | null;
  isLive: boolean; initialError: string | null;
}> {
  try {
    const result = await invoke<LiveStreamInfo>('get_douyin_live_stream_url_with_quality', {
      payload: { args: { room_id_str: roomId.trim() } }, quality,
    });
    if (result.error_message) throw new Error(result.error_message);
    const metadata = { title: result.title, anchorName: result.anchor_name, avatar: result.avatar };
    if (result.status !== 2) {
      return { ...metadata, streamUrl: '', streamType: undefined, isLive: false, initialError: '主播未开播。' };
    }
    return {
      ...metadata, ...selectPlaybackVariant(result.available_streams ?? [], quality, line),
      isLive: true, initialError: null,
      headers: { Referer: 'https://live.douyin.com/', 'User-Agent': navigator.userAgent },
    };
  } catch (error: unknown) {
    console.error('Diagnostic: playerHelper.ts:51 (details omitted)');
    return { streamUrl: '', streamType: undefined, isLive: false,
      initialError: error instanceof Error ? error.message : String(error) };
  }
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
  
  const eventName = 'danmaku-message';

  const unlisten = await listen<DouyinRustDanmakuPayload>(eventName, (event: TauriEvent<DouyinRustDanmakuPayload>) => {
    if (event.payload?.room_id === roomId) {
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
          console.warn('Diagnostic: playerHelper.ts:104 (details omitted)');
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
  try {
    await invoke('start_douyin_danmu_listener', { payload: rustPayload });
  } catch (error) {
    unlisten();
    throw error;
  }
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
    console.error('Diagnostic: playerHelper.ts:136 (details omitted)');
  }
}

