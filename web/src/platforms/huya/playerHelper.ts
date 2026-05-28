import { invoke } from '@tauri-apps/api/core';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';
import { Ref } from 'vue';
import type { DanmakuMessage, DanmuOverlayInstance, DanmuOverlayResolver, DanmuRenderOptions } from '../../components/player/types';
import { v4 as uuidv4 } from 'uuid';

export interface HuyaUnifiedEntry {
  quality: string;
  bitRate: number;
  url: string;
}

export async function getHuyaStreamConfig(
  roomId: string,
  quality: string = '原画',
  line?: string | null,
): Promise<{ streamUrl: string; streamType: string | undefined }> {
  console.log('[HuyaPlayerHelper] getHuyaStreamConfig called with roomId:', roomId, 'quality:', quality);
  try {
    const result = await invoke<any>('get_huya_unified_cmd', { roomId, quality, line: line ?? null });
    console.log('[HuyaPlayerHelper] getHuyaStreamConfig got result:', result);

    if (result && result.flv_tx_urls && Array.isArray(result.flv_tx_urls)) {
      const streamUrl =
        pickHuyaUrlByQuality(result.flv_tx_urls, quality) ||
        result.selected_url ||
        result.flv_tx_urls[0]?.url;

      if (streamUrl) {
        const sanitizedUrl = enforceHttps(streamUrl);
        return { streamUrl: sanitizedUrl, streamType: inferStreamType(sanitizedUrl) };
      }

      throw new Error('Huya stream URL is unavailable');
    }

    throw new Error('Failed to fetch Huya room stream details');
  } catch (error: any) {
    console.error('[HuyaPlayerHelper] getHuyaStreamConfig error:', error);
    const msg = String(error?.message || '').trim();
    if (msg.includes('未开播')) {
      throw new Error(msg);
    }
    throw new Error('Huya stream URL is unavailable');
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

  try {
    await invoke('start_huya_danmaku_listener', { payload: { args: { room_id_str: roomId } } });
    console.log('[HuyaPlayerHelper] Backend Huya danmaku listener started');
  } catch (error) {
    console.error('[HuyaPlayerHelper] Failed to start backend Huya danmaku listener:', error);
    throw error;
  }

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

function pickHuyaUrlByQuality(entries: HuyaUnifiedEntry[], quality: string): string | undefined {
  if (matchesHuyaQuality(quality, 'source')) {
    return entries.find((entry) => entry.bitRate === 0)?.url ?? entries.find((entry) => matchesHuyaQuality(entry.quality, 'source'))?.url;
  }
  if (matchesHuyaQuality(quality, 'high')) {
    return entries.find((entry) => entry.bitRate === 4000)?.url ?? entries.find((entry) => matchesHuyaQuality(entry.quality, 'high'))?.url;
  }
  if (matchesHuyaQuality(quality, 'standard')) {
    return entries.find((entry) => entry.bitRate === 2000)?.url ?? entries.find((entry) => matchesHuyaQuality(entry.quality, 'standard'))?.url;
  }
  return entries.find((entry) => entry.quality === quality)?.url;
}

function matchesHuyaQuality(value: string | undefined, target: 'source' | 'high' | 'standard'): boolean {
  if (!value) {
    return false;
  }
  const normalized = value.toLowerCase();
  if (target === 'source') {
    return normalized.includes('原') || normalized.includes('source') || normalized.includes('blue');
  }
  if (target === 'high') {
    return normalized.includes('高') || normalized.includes('high');
  }
  return normalized.includes('标') || normalized.includes('standard') || normalized.includes('流畅');
}

function enforceHttps(url: string): string {
  if (!url) return url;
  if (url.startsWith('http://')) {
    return url.replace('http://', 'https://');
  }
  return url;
}

function inferStreamType(url: string): string | undefined {
  if (!url) return undefined;
  if (url.includes('.flv')) {
    return 'flv';
  }
  if (url.includes('.m3u8')) {
    return 'hls';
  }
  return undefined;
}
