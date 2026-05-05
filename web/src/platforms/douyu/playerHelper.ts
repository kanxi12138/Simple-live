import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { v4 as uuidv4 } from 'uuid';
import type { Ref } from 'vue';
import cryptoJsSource from '../../../../core/src/platforms/douyu/cryptojs.min.js?raw';

import type { DanmakuMessage, DanmuOverlayInstance, DanmuOverlayResolver, DanmuRenderOptions } from '../../components/player/types';

export interface UnifiedRustDanmakuPayload {
  room_id?: string;
  user: string;
  content: string;
  user_level: number;
  fans_club_level: number;
}

interface DouyuRoomInitInfo {
  room_id: string;
  is_live: boolean;
}

interface DouyuRateVariant {
  name: string;
  rate: number;
  bit?: number | null;
}

interface DouyuPlayInfo {
  variants: DouyuRateVariant[];
  cdns: string[];
}

const QUALITY_ORIGIN = '原画';
const QUALITY_HIGH = '高清';
const QUALITY_STANDARD = '标清';
const DEFAULT_DOUYU_DID = '10000000000000000000000000001501';

let douyuProxyActive = false;

export async function getDouyuStreamConfig(
  roomId: string,
  quality: string = QUALITY_ORIGIN,
  line?: string | null,
): Promise<{ streamUrl: string; streamType: string | undefined }> {
  await stopDouyuProxy();

  let finalStreamUrl: string | null = null;
  let streamType: string | undefined;
  const maxAttempts = 2;

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      const roomInit = await invoke<DouyuRoomInitInfo>('fetch_douyu_room_init_cmd', { roomId });
      if (!roomInit?.room_id) {
        throw new Error('Douyu room init returned empty room id');
      }
      if (!roomInit.is_live) {
        throw new Error('douyu_room_offline');
      }

      const realRoomId = roomInit.room_id;
      const homeH5Enc = await invoke<string>('fetch_douyu_home_h5_enc_cmd', { roomId: realRoomId });
      const signData = executeDouyuSign(homeH5Enc, realRoomId, DEFAULT_DOUYU_DID, Math.floor(Date.now() / 1000));
      const playInfo = await invoke<DouyuPlayInfo>('fetch_douyu_play_info_cmd', {
        roomId: realRoomId,
        signData,
      });

      const selectedRate = resolveRateForQuality(quality, playInfo?.variants ?? []);
      const selectedCdn = selectDouyuCdn(line, playInfo?.cdns ?? []);
      const streamUrl = await invoke<string>('fetch_douyu_play_url_cmd', {
        roomId: realRoomId,
        signData,
        rate: selectedRate,
        cdn: selectedCdn,
      });

      if (!streamUrl) {
        throw new Error('Douyu stream URL is empty');
      }

      const playable = buildDouyuPlayableUrl(streamUrl);
      finalStreamUrl = playable.url;
      streamType = playable.streamType;
      break;
    } catch (error: any) {
      console.error(`[DouyuPlayerHelper] Failed to fetch stream url (${attempt}/${maxAttempts}):`, error?.message);
      if (isDouyuOfflineMessage(error?.message)) {
        throw new Error('douyu_room_offline');
      }

      if (attempt === maxAttempts) {
        throw new Error(`Failed to fetch Douyu stream: ${error?.message || 'unknown error'}`);
      }

      await new Promise((resolve) => setTimeout(resolve, 1000 * attempt));
    }
  }

  if (!finalStreamUrl) {
    throw new Error('Unable to resolve a valid Douyu stream URL');
  }

  await invoke('set_stream_url_cmd', { url: finalStreamUrl });
  const proxyUrl = await invoke<string>('start_proxy');
  douyuProxyActive = true;

  return {
    streamUrl: withLocalProxyCacheBust(proxyUrl),
    streamType: streamType === 'hls' ? 'hls' : 'flv',
  };
}

export async function startDouyuDanmakuListener(
  roomId: string,
  danmuOverlay: DanmuOverlayResolver,
  danmakuMessagesRef: Ref<DanmakuMessage[]>,
  renderOptions?: DanmuRenderOptions,
): Promise<() => void> {
  const resolveOverlay = (): DanmuOverlayInstance | null =>
    typeof danmuOverlay === 'function' ? danmuOverlay() : danmuOverlay;
  let loggedFirstPayload = false;
  const unlisten = await listen<UnifiedRustDanmakuPayload>('danmaku-message', (event) => {
    if (!event.payload) {
      return;
    }

    const payload = event.payload;
    if (!payload.content) {
      return;
    }

    const frontendDanmaku: DanmakuMessage = {
      id: uuidv4(),
      nickname: payload.user || 'Unknown',
      content: payload.content || '',
      level: String(payload.user_level || 0),
      badgeLevel: payload.fans_club_level > 0 ? String(payload.fans_club_level) : undefined,
      room_id: payload.room_id || roomId,
    };

    const shouldDisplay = renderOptions?.shouldDisplay ? renderOptions.shouldDisplay(frontendDanmaku) : true;
    const shouldAppend = renderOptions?.shouldAppendToList ? renderOptions.shouldAppendToList(frontendDanmaku) : true;
    const activeOverlay = resolveOverlay();

    if (!loggedFirstPayload) {
      loggedFirstPayload = true;
    }

    if (shouldDisplay && activeOverlay?.sendComment) {
      try {
        activeOverlay.play?.();
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
        console.warn('[DouyuPlayerHelper] Failed emitting danmu.js comment:', emitError);
      }
    }

    if (!shouldAppend) {
      return;
    }

    danmakuMessagesRef.value.push(frontendDanmaku);
    if (danmakuMessagesRef.value.length > 200) {
      danmakuMessagesRef.value.splice(0, danmakuMessagesRef.value.length - 200);
    }
  });

  try {
    await invoke('start_danmaku_listener', { roomId });
  } catch (error) {
    unlisten();
    throw error;
  }

  return () => {
    unlisten();
  };
}

export async function stopDouyuDanmaku(_roomId: string, currentUnlistenFn: (() => void) | null): Promise<void> {
  if (currentUnlistenFn) {
    currentUnlistenFn();
  }
  try {
    if (_roomId) {
      await invoke('stop_danmaku_listener', { roomId: _roomId });
    }
  } catch (error) {
    console.error('[DouyuPlayerHelper] Error invoking stop_danmaku_listener for Douyu:', error);
  }
}

export async function stopDouyuProxy(): Promise<void> {
  if (!douyuProxyActive) {
    return;
  }
  try {
    await invoke('stop_proxy');
  } catch (error) {
    console.error('[DouyuPlayerHelper] Error stopping proxy server:', error);
  } finally {
    douyuProxyActive = false;
  }
}

export function isDouyuOfflineMessage(message: string | null | undefined): boolean {
  const normalized = String(message || '').toLowerCase();
  if (!normalized) {
    return false;
  }

  return normalized.includes('douyu_room_offline')
    || normalized.includes('error: 1')
    || normalized.includes('error: 102')
    || normalized.includes('error code 1')
    || normalized.includes('error code 102')
    || normalized.includes('show_status')
    || normalized.includes('not live');
}

function executeDouyuSign(script: string, rid: string, did: string, ts: number): string {
  const ridJs = JSON.stringify(rid);
  const didJs = JSON.stringify(did);
  const signer = new Function(
    `${cryptoJsSource}\n${script}\nreturn ub98484234(${ridJs}, ${didJs}, ${ts});`,
  );
  const result = signer();

  if (typeof result !== 'string' || !result.trim()) {
    throw new Error('Douyu sign result is empty');
  }

  return result;
}

function normalizeDouyuCdnKey(input: string | null | undefined): string {
  const normalized = String(input || '').trim().toLowerCase();
  if (normalized === 'ws-h5' || normalized === 'tct-h5' || normalized === 'ali-h5' || normalized === 'hs-h5') {
    return normalized;
  }
  return 'ws-h5';
}

function selectDouyuCdn(requested: string | null | undefined, available: string[]): string {
  const normalizedRequested = normalizeDouyuCdnKey(requested);
  if (!available.length) {
    return normalizedRequested;
  }
  return available.find((item) => item.toLowerCase() === normalizedRequested) ?? available[0];
}

function resolveRateForQuality(quality: string, variants: DouyuRateVariant[]): number {
  if (!variants.length) {
    return 0;
  }

  if (quality === QUALITY_ORIGIN) {
    return variants.find((variant) => variant.rate === 0)?.rate
      ?? variants.reduce((best, variant) => Math.min(best, variant.rate), Number.POSITIVE_INFINITY);
  }

  if (quality === QUALITY_HIGH) {
    const sortedHigh = [...variants]
      .filter((variant) => variant.rate !== 0)
      .sort((left, right) => (right.bit ?? 0) - (left.bit ?? 0) || right.rate - left.rate);
    return variants.find((variant) => variant.rate === 4)?.rate
      ?? sortedHigh[0]?.rate
      ?? 0;
  }

  if (quality === QUALITY_STANDARD) {
    const sortedLow = [...variants]
      .filter((variant) => variant.rate !== 0)
      .sort((left, right) => (left.bit ?? Number.MAX_SAFE_INTEGER) - (right.bit ?? Number.MAX_SAFE_INTEGER) || left.rate - right.rate);
    return variants.find((variant) => variant.rate === 3)?.rate
      ?? sortedLow[0]?.rate
      ?? 0;
  }

  return Math.max(...variants.map((variant) => variant.rate));
}

function buildDouyuPlayableUrl(streamUrl: string): { url: string; streamType: string } {
  const decoded = streamUrl ? streamUrl.replace(/&amp;/g, '&') : streamUrl;
  const inferredType = inferDouyuStreamType(decoded);

  if (decoded && /^https?:\/\//i.test(decoded)) {
    return {
      url: decoded,
      streamType: inferredType,
    };
  }

  const key = extractDouyuStreamKey(decoded);
  if (key) {
    return {
      url: `http://vplay1a.douyucdn.cn/live/${key}.flv?uuid=`,
      streamType: 'flv',
    };
  }
  return {
    url: decoded,
    streamType: inferredType,
  };
}

function withLocalProxyCacheBust(proxyUrl: string): string {
  const separator = proxyUrl.includes('?') ? '&' : '?';
  return `${proxyUrl}${separator}t=${Date.now()}`;
}

function extractDouyuStreamKey(streamUrl: string): string | null {
  if (!streamUrl) {
    return null;
  }

  const patterns = [
    /\/live\/(\d{1,8}[0-9a-zA-Z]+)(?:_\d{0,4})?(?:\.flv|\.xs|\/playlist|\.m3u8)/i,
    /(\d{1,8}[0-9a-zA-Z]+)(?:_\d{0,4})?(?:\.flv|\.xs|\/playlist|\.m3u8)/i,
  ];

  for (const pattern of patterns) {
    const match = streamUrl.match(pattern);
    if (match?.[1]) {
      return match[1];
    }
  }

  return null;
}

function inferDouyuStreamType(streamUrl: string): string {
  const normalized = String(streamUrl || '').toLowerCase();
  if (normalized.includes('.m3u8') || normalized.includes('/playlist')) {
    return 'hls';
  }
  return 'flv';
}
