import { invoke } from '@tauri-apps/api/core';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';
import { selectPlaybackVariant, type PlaybackConfig } from '../common/playback';
import type { LiveStreamInfo } from '../common/types';
import type { Ref } from 'vue';
import type { DanmakuMessage, DanmuOverlayInstance, DanmuOverlayResolver, DanmuRenderOptions } from '../../components/player/types';
import { v4 as uuidv4 } from 'uuid';

/** Resolves actual Bilibili streams; propagates platform and room-state errors. */
export async function getBilibiliStreamConfig(
  roomId: string,
  quality: string = '原画',
  cookie?: string,
  line?: string | null,
): Promise<PlaybackConfig> {
  try {
    const result = await invoke<LiveStreamInfo & { qualities: string[]; headers: Record<string, string> }>(
      'get_bilibili_live_stream_url_with_quality', {
        payload: { args: { room_id_str: roomId } }, quality,
        cookie: cookie || localStorage.getItem('bilibili_cookie') || null,
      },
    );
    if (result.error_message) throw new Error(result.error_message);
    if (result.status !== 1) throw new Error('B站主播未开播');
    return { ...selectPlaybackVariant(result.available_streams || [], quality, line, result.qualities), headers: result.headers };
  } catch (error) {
    console.error('[Bilibili] Playback request failed:', error);
    throw error;
  }
}

interface UnifiedRustDanmakuPayload {
  room_id: string;
  user: string;
  content: string;
  user_level: number;
  fans_club_level: number;
}

export async function startBilibiliDanmakuListener(
  roomId: string,
  danmuOverlay: DanmuOverlayResolver,
  danmakuMessagesRef: Ref<DanmakuMessage[]>,
  cookie?: string,
  renderOptions?: DanmuRenderOptions,
): Promise<() => void> {
  const resolveOverlay = (): DanmuOverlayInstance | null =>
    typeof danmuOverlay === 'function' ? danmuOverlay() : danmuOverlay;
  const effectiveCookie =
    cookie ?? (typeof localStorage !== 'undefined' ? (localStorage.getItem('bilibili_cookie') || undefined) : undefined);

  const unlisten = await listen<UnifiedRustDanmakuPayload>('danmaku-message', (event: TauriEvent<UnifiedRustDanmakuPayload>) => {
    if (!event.payload || event.payload.room_id !== roomId) {
      return;
    }
    const content = event.payload.content || '';
    if (content.includes('进入直播间') || content.includes('来了')) {
      return;
    }

    const frontendDanmaku: DanmakuMessage = {
      id: uuidv4(),
      nickname: event.payload.user || 'Unknown',
      content,
      level: String(event.payload.user_level ?? 0),
      badgeLevel: event.payload.fans_club_level != null ? String(event.payload.fans_club_level) : undefined,
      room_id: roomId,
    };

    const shouldDisplay = renderOptions?.shouldDisplay ? renderOptions.shouldDisplay(frontendDanmaku) : true;
    const activeOverlay = resolveOverlay();
    if (shouldDisplay && activeOverlay?.sendComment) {
      try {
        const commentOptions = renderOptions?.buildCommentOptions?.(frontendDanmaku) ?? {};
        const styleFromOptions = commentOptions.style ?? {};
        const preferredColor = styleFromOptions.color || (frontendDanmaku as any).color || '#FFFFFF';
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
        console.warn('[BilibiliPlayerHelper] Failed emitting danmu.js comment:', emitError);
      }
    }

    const shouldAppend = renderOptions?.shouldAppendToList ? renderOptions.shouldAppendToList(frontendDanmaku) : true;
    if (!shouldAppend) {
      return;
    }

    danmakuMessagesRef.value.push(frontendDanmaku);
    if (danmakuMessagesRef.value.length > 200) {
      danmakuMessagesRef.value.splice(0, danmakuMessagesRef.value.length - 200);
    }
  });

  try {
    await invoke('start_bilibili_danmaku_listener', {
      payload: { args: { room_id_str: roomId } },
      cookie: effectiveCookie || null,
    });
  } catch (error) {
    unlisten();
    throw error;
  }

  return unlisten;
}

export async function stopBilibiliDanmaku(currentUnlistenFn: (() => void) | null): Promise<void> {
  if (currentUnlistenFn) {
    try {
      currentUnlistenFn();
    } catch (error) { console.warn('[Bilibili] Failed to stop danmaku:', error); }
  }
  try {
    await invoke('stop_bilibili_danmaku_listener');
  } catch (error) { console.warn('[Bilibili] Failed to stop danmaku:', error); }
}

