import type { PlaybackConfig } from '../common/playback';
import { invoke } from '@tauri-apps/api/core';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';
import { Ref } from 'vue';
import type { DanmakuMessage, DanmuOverlayInstance, DanmuOverlayResolver, DanmuRenderOptions } from '../../components/player/types';
import { v4 as uuidv4 } from 'uuid';

export interface HuyaUnifiedEntry {
  quality: string;
  bitRate: number;
  line: string;
  url: string;
}

interface HuyaPlaybackResponse {
  is_live: boolean;
  selected_url: string | null;
  flv_tx_urls: HuyaUnifiedEntry[];
  lines: string[];
  headers: Record<string, string>;
}

/** Resolves the requested room and options; propagates platform and token errors. */
export async function getHuyaStreamConfig(
  roomId: string,
  quality: string = '原画',
  line?: string | null,
): Promise<PlaybackConfig> {
  try {
    const result = await invoke<HuyaPlaybackResponse>('get_huya_unified_cmd', { roomId, quality, line: line ?? null });
    if (!result.is_live) throw new Error('虎牙主播未开播');
    const selected = result.flv_tx_urls.find((entry) => entry.url === result.selected_url);
    if (!selected) throw new Error('虎牙未返回可用播放地址');
    return {
      streamUrl: selected.url, streamType: 'flv', headers: result.headers,
      qualities: [...new Set(result.flv_tx_urls.map((entry) => entry.quality))],
      lines: result.lines.map((cdn) => ({ key: cdn, label: cdn })),
      selectedQuality: selected.quality, selectedLine: selected.line,
    };
  } catch (error) {
    console.error('[Huya] Playback request failed:', error);
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

let currentHuyaRoomId: string | null = null;

export async function startHuyaDanmakuListener(
  roomId: string,
  danmuOverlay: DanmuOverlayResolver,
  danmakuMessagesRef: Ref<DanmakuMessage[]>,
  renderOptions?: DanmuRenderOptions,
): Promise<() => void> {
  const resolveOverlay = (): DanmuOverlayInstance | null =>
    typeof danmuOverlay === 'function' ? danmuOverlay() : danmuOverlay;
  console.log('[HuyaPlayerHelper] Starting Huya danmaku listener for room:', roomId);
  currentHuyaRoomId = roomId;

  const unlisten = await listen<UnifiedRustDanmakuPayload>('danmaku-message', (event: TauriEvent<UnifiedRustDanmakuPayload>) => {
    console.log('[HuyaPlayerHelper] Received danmaku event:', event.payload);
    if (!event.payload || event.payload.room_id !== roomId) {
      return;
    }

    const frontendDanmaku: DanmakuMessage = {
      id: uuidv4(),
      nickname: event.payload.user || 'Unknown',
      content: event.payload.content,
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
        console.warn('[HuyaPlayerHelper] Failed emitting danmu.js comment:', emitError);
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

  console.log('[HuyaPlayerHelper] Event listener registered for: danmaku-message');
  try {
    await invoke('start_huya_danmaku_listener', { payload: { args: { room_id_str: roomId } } });
  } catch (error) {
    unlisten();
    throw error;
  }
  return unlisten;
}

export async function stopHuyaDanmaku(currentUnlistenFn: (() => void) | null): Promise<void> {
  if (currentUnlistenFn) {
    try {
      currentUnlistenFn();
      console.log('[HuyaPlayerHelper] Event listener unregistered');
    } catch (error) {
      console.warn('[HuyaPlayerHelper] stopHuyaDanmaku cleanup error:', error);
    }
  }

  try {
    const roomIdToStop = currentHuyaRoomId || '';
    await invoke('stop_huya_danmaku_listener', { roomId: roomIdToStop });
  } catch (error) {
    console.warn('[HuyaPlayerHelper] stopHuyaDanmaku: backend stop encountered error (ignored):', error);
  }

  currentHuyaRoomId = null;
  console.log('[HuyaPlayerHelper] Huya danmaku stopped');
}

