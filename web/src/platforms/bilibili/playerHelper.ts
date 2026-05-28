import { invoke } from '@tauri-apps/api/core';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';
import type { LiveStreamInfo, StreamVariant } from '../common/types';
import type { Ref } from 'vue';
import type { DanmakuMessage, DanmuOverlayInstance, DanmuOverlayResolver, DanmuRenderOptions } from '../../components/player/types';
import { v4 as uuidv4 } from 'uuid';

export async function getBilibiliStreamConfig(
  roomId: string,
  quality: string = '原画',
  cookie?: string,
): Promise<{ streamUrl: string; streamType: string | undefined }> {
  if (!roomId) {
    throw new Error('Missing Bilibili room ID');
  }

  const payloadData = { args: { room_id_str: roomId } };
  const effectiveCookie =
    cookie ?? (typeof localStorage !== 'undefined' ? (localStorage.getItem('bilibili_cookie') || undefined) : undefined);

  const result = await invoke<LiveStreamInfo>('get_bilibili_live_stream_url_with_quality', {
    payload: payloadData,
    quality,
    cookie: effectiveCookie || null,
  });

  if (result.error_message) {
    const msg = result.error_message.trim();
    if (msg.includes('未开播')) {
      throw new Error(msg);
    }
    throw new Error('Bilibili stream URL is unavailable');
  }

  if (typeof result.status !== 'undefined' && result.status !== 1) {
    throw new Error('Bilibili streamer is offline');
  }

  if (!result.stream_url) {
    throw new Error('Bilibili stream URL is unavailable');
  }

  if (result.upstream_url) {
    console.info('[Bilibili] Upstream url:', result.upstream_url);
  }
  if (result.available_streams && Array.isArray(result.available_streams)) {
    console.info(`[Bilibili] Available streams: ${result.available_streams.length}`);
    (result.available_streams as StreamVariant[]).forEach((variant, index) => {
      const meta = [variant.format, variant.desc, variant.qn?.toString(), variant.protocol].filter(Boolean).join(' | ');
      console.info(`  [${index + 1}] ${variant.url}${meta ? `  <<< ${meta}` : ''}`);
    });
  }

  let streamType: string | undefined;
  const streamUrlLower = result.stream_url.toLowerCase();

  if (
    streamUrlLower.startsWith('http://127.0.0.1') ||
    streamUrlLower.includes('/live.flv') ||
    streamUrlLower.includes('.flv')
  ) {
    streamType = 'flv';
  } else if (streamUrlLower.includes('.m3u8')) {
    streamType = 'hls';
  }

  if (!streamType && result.available_streams && Array.isArray(result.available_streams)) {
    const matchedVariant = (result.available_streams as StreamVariant[]).find((variant) => {
      if (!variant?.url) {
        return false;
      }
      const formatLower = variant.format?.toLowerCase() ?? '';
      const protocolLower = variant.protocol?.toLowerCase() ?? '';
      const isSameAsPrimary = variant.url === result.stream_url || variant.url === result.upstream_url;
      const isHlsCandidate =
        formatLower === 'ts' ||
        formatLower === 'fmp4' ||
        formatLower === 'mp4' ||
        formatLower === 'm4s' ||
        protocolLower.includes('hls');
      return isSameAsPrimary && isHlsCandidate;
    });

    if (matchedVariant) {
      streamType = 'hls';
    }
  }

  if (!streamType && result.upstream_url) {
    const upstreamLower = result.upstream_url.toLowerCase();
    if (upstreamLower.includes('.m3u8')) {
      streamType = 'hls';
    } else if (
      upstreamLower.startsWith('http://127.0.0.1') ||
      upstreamLower.includes('/live.flv') ||
      upstreamLower.includes('.flv')
    ) {
      streamType = 'flv';
    }
  }

  if (!streamType) {
    streamType = 'flv';
  }

  return {
    streamUrl: withLocalProxyCacheBust(result.stream_url, {
      roomId,
      quality,
    }),
    streamType,
  };
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
    } catch {}
  }
  try {
    await invoke('stop_bilibili_danmaku_listener');
  } catch {}
}

function withLocalProxyCacheBust(
  url: string,
  options: { roomId: string; quality?: string | null },
): string {
  if (!url.startsWith('http://127.0.0.1:')) {
    return url;
  }
  const next = new URL(url);
  next.searchParams.set('_dtv_room', options.roomId);
  if (options.quality) {
    next.searchParams.set('_dtv_quality', options.quality);
  }
  next.searchParams.set('_dtv_t', String(Date.now()));
  return next.toString();
}
