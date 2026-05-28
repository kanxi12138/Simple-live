import { invoke } from '@tauri-apps/api/core';
import { v4 as uuidv4 } from 'uuid';
import type { Ref } from 'vue';
import cryptoJsSource from '../../../../core/src/platforms/douyu/cryptojs.min.js?raw';
import { postAndroidDebugLog } from '../../runtime/androidDiagnostics';

import type {
  DanmakuMessage,
  DanmuOverlayInstance,
  DanmuOverlayResolver,
  DanmuRenderOptions,
} from '../../components/player/types';

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

type DouyuSttMessage = Record<string, string>;

const QUALITY_ORIGIN = '原画';
const QUALITY_HIGH = '高清';
const QUALITY_STANDARD = '标清';
const DEFAULT_DOUYU_DID = '10000000000000000000000000001501';
const DOUYU_DANMAKU_URL = 'wss://danmuproxy.douyu.com:8506';
const DOUYU_DANMAKU_HEARTBEAT_MS = 45_000;
const DOUYU_DANMAKU_RECONNECT_LIMIT = 5;

let douyuProxyActive = false;
let currentDouyuDanmakuSocket: WebSocket | null = null;
let currentDouyuDanmakuHeartbeatTimer: number | null = null;
let currentDouyuDanmakuReconnectTimer: number | null = null;
let currentDouyuDanmakuConnectionToken = 0;
let currentDouyuDanmakuMessageCount = 0;

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
      const signData = executeDouyuSign(
        homeH5Enc,
        realRoomId,
        DEFAULT_DOUYU_DID,
        Math.floor(Date.now() / 1000),
      );
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
    } catch (error: unknown) {
      const errorMessage = error instanceof Error ? error.message : String(error || 'unknown error');
      console.error(`[DouyuPlayerHelper] Failed to fetch stream url (${attempt}/${maxAttempts}):`, errorMessage);
      if (isDouyuOfflineMessage(errorMessage)) {
        throw new Error('douyu_room_offline');
      }

      if (attempt === maxAttempts) {
        throw new Error(`Failed to fetch Douyu stream: ${errorMessage}`);
      }

      await new Promise((resolve) => window.setTimeout(resolve, 1000 * attempt));
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
  const normalizedRoomId = await resolveDouyuDanmakuRoomId(roomId);
  postAndroidDebugLog('i', `[DouyuDanmaku] start room=${roomId} normalized=${normalizedRoomId}`);

  const resolveOverlay = (): DanmuOverlayInstance | null =>
    typeof danmuOverlay === 'function' ? danmuOverlay() : danmuOverlay;

  const nativeBridge = getNativeDouyuDanmakuBridge();
  if (nativeBridge) {
    return startNativeDouyuDanmakuListener(
      nativeBridge,
      normalizedRoomId,
      roomId,
      resolveOverlay,
      danmakuMessagesRef,
      renderOptions,
    );
  }

  stopDouyuDanmakuConnection();

  const connectionToken = ++currentDouyuDanmakuConnectionToken;
  let reconnectAttempts = 0;
  let pendingBuffer = new Uint8Array(0);
  let isClosedManually = false;

  const emitDanmakuMessage = (payload: UnifiedRustDanmakuPayload) => {
    currentDouyuDanmakuMessageCount += 1;
    if (currentDouyuDanmakuMessageCount <= 5) {
      postAndroidDebugLog(
        'i',
        `[DouyuDanmaku] emit chat room=${normalizedRoomId} user=${payload.user} len=${payload.content.length}`,
      );
    }
    const frontendDanmaku = buildDouyuFrontendDanmaku(payload, roomId);
    pushDouyuDanmakuToUi(frontendDanmaku, resolveOverlay, danmakuMessagesRef, renderOptions);
  };

  const scheduleReconnect = () => {
    if (isClosedManually || connectionToken !== currentDouyuDanmakuConnectionToken) {
      return;
    }
    if (reconnectAttempts >= DOUYU_DANMAKU_RECONNECT_LIMIT) {
      postAndroidDebugLog('e', `[DouyuDanmaku] reconnect limit reached room=${normalizedRoomId}`);
      return;
    }
    reconnectAttempts += 1;
    postAndroidDebugLog('w', `[DouyuDanmaku] schedule reconnect room=${normalizedRoomId} attempt=${reconnectAttempts}`);
    clearDouyuDanmakuReconnectTimer();
    currentDouyuDanmakuReconnectTimer = window.setTimeout(() => {
      void connectDanmakuSocket();
    }, 5000);
  };

  const connectDanmakuSocket = async () => {
    if (connectionToken !== currentDouyuDanmakuConnectionToken) {
      return;
    }

    stopDouyuDanmakuConnection();
    pendingBuffer = new Uint8Array(0);
    currentDouyuDanmakuMessageCount = 0;

    const socket = new WebSocket(DOUYU_DANMAKU_URL);
    socket.binaryType = 'arraybuffer';
    currentDouyuDanmakuSocket = socket;
    postAndroidDebugLog('i', `[DouyuDanmaku] connecting room=${normalizedRoomId} url=${DOUYU_DANMAKU_URL}`);

    socket.onopen = () => {
      if (connectionToken !== currentDouyuDanmakuConnectionToken) {
        socket.close();
        return;
      }
      reconnectAttempts = 0;
      postAndroidDebugLog('i', `[DouyuDanmaku] open room=${normalizedRoomId}`);
      sendDouyuDanmakuPacket(socket, `type@=loginreq/roomid@=${normalizedRoomId}/`);
      sendDouyuDanmakuPacket(socket, `type@=joingroup/rid@=${normalizedRoomId}/gid@=-9999/`);
      clearDouyuDanmakuHeartbeatTimer();
      currentDouyuDanmakuHeartbeatTimer = window.setInterval(() => {
        postAndroidDebugLog('d', `[DouyuDanmaku] heartbeat room=${normalizedRoomId}`);
        sendDouyuDanmakuPacket(socket, 'type@=mrkl/');
      }, DOUYU_DANMAKU_HEARTBEAT_MS);
    };

    socket.onmessage = async (event) => {
      if (connectionToken !== currentDouyuDanmakuConnectionToken) {
        return;
      }
      const chunk = await toUint8Array(event.data);
      postAndroidDebugLog('d', `[DouyuDanmaku] recv chunk room=${normalizedRoomId} bytes=${chunk.length}`);
      pendingBuffer = appendUint8Array(pendingBuffer, chunk);
      const parsedBatch = decodeDouyuPacketBatch(pendingBuffer);
      pendingBuffer = parsedBatch.remaining;
      postAndroidDebugLog(
        'd',
        `[DouyuDanmaku] parsed room=${normalizedRoomId} messages=${parsedBatch.messages.length} remaining=${parsedBatch.remaining.length}`,
      );
      for (const message of parsedBatch.messages) {
        postAndroidDebugLog(
          'd',
          `[DouyuDanmaku] message room=${normalizedRoomId} type=${message.type || ''} hasTxt=${message.txt ? 1 : 0} hasDms=${message.dms ? 1 : 0}`,
        );
        if (message.type !== 'chatmsg' || !message.txt || !message.dms) {
          continue;
        }
        emitDanmakuMessage({
          room_id: normalizedRoomId,
          user: message.nn || 'Unknown',
          content: message.txt,
          user_level: Number.parseInt(message.level || '0', 10) || 0,
          fans_club_level: Number.parseInt(message.bl || '0', 10) || 0,
        });
      }
    };

    socket.onerror = (error) => {
      postAndroidDebugLog('e', `[DouyuDanmaku] socket error room=${normalizedRoomId} ${String(error)}`);
      console.warn('[DouyuPlayerHelper] Douyu danmaku websocket error:', error);
    };

    socket.onclose = (event) => {
      clearDouyuDanmakuHeartbeatTimer();
      currentDouyuDanmakuSocket = null;
      postAndroidDebugLog(
        'w',
        `[DouyuDanmaku] close room=${normalizedRoomId} code=${event.code} reason=${event.reason || 'none'} clean=${event.wasClean}`,
      );
      scheduleReconnect();
    };
  };

  await connectDanmakuSocket();

  return () => {
    isClosedManually = true;
    postAndroidDebugLog('i', `[DouyuDanmaku] stop room=${normalizedRoomId}`);
    stopDouyuDanmakuConnection();
  };
}

export async function stopDouyuDanmaku(roomId: string, currentUnlistenFn: (() => void) | null): Promise<void> {
  void roomId;
  if (currentUnlistenFn) {
    currentUnlistenFn();
  }
  getNativeDouyuDanmakuBridge()?.stop();
  stopDouyuDanmakuConnection();
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
      .sort(
        (left, right) =>
          (left.bit ?? Number.MAX_SAFE_INTEGER) - (right.bit ?? Number.MAX_SAFE_INTEGER)
          || left.rate - right.rate,
      );
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

async function resolveDouyuDanmakuRoomId(roomId: string): Promise<string> {
  try {
    const roomInit = await invoke<DouyuRoomInitInfo>('fetch_douyu_room_init_cmd', { roomId });
    if (roomInit?.room_id) {
      return roomInit.room_id;
    }
  } catch (error) {
    console.warn('[DouyuPlayerHelper] Failed to normalize danmaku room id:', error);
  }
  return roomId;
}

function sendDouyuDanmakuPacket(socket: WebSocket, body: string): void {
  if (socket.readyState !== WebSocket.OPEN) {
    postAndroidDebugLog('w', `[DouyuDanmaku] skip send state=${socket.readyState} body=${body.slice(0, 32)}`);
    return;
  }
  postAndroidDebugLog('d', `[DouyuDanmaku] send body=${body.slice(0, 48)}`);
  socket.send(encodeDouyuDanmakuBody(body));
}

function encodeDouyuDanmakuBody(body: string): ArrayBuffer {
  const bodyBytes = new TextEncoder().encode(body);
  const packetLength = bodyBytes.length + 9;
  const buffer = new ArrayBuffer(packetLength + 4);
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);

  view.setUint32(0, packetLength, true);
  view.setUint32(4, packetLength, true);
  view.setUint16(8, 689, true);
  bytes.set(bodyBytes, 12);
  bytes[12 + bodyBytes.length] = 0;

  return buffer;
}

function decodeDouyuPacketBatch(buffer: Uint8Array): {
  messages: DouyuSttMessage[];
  remaining: Uint8Array;
} {
  const messages: DouyuSttMessage[] = [];
  let cursor = 0;

  while (cursor + 12 <= buffer.length) {
    const view = new DataView(buffer.buffer, buffer.byteOffset + cursor);
    const packetLength = view.getUint32(0, true);
    if (packetLength < 9) {
      break;
    }

    const frameEnd = cursor + 4 + packetLength;
    if (frameEnd > buffer.length) {
      break;
    }

    const bodyLength = packetLength - 9;
    const bodyStart = cursor + 12;
    const bodyEnd = bodyStart + bodyLength;
    const body = new TextDecoder().decode(buffer.slice(bodyStart, bodyEnd)).replace(/\0+$/g, '');
    for (const part of body.split('//')) {
      const parsed = parseDouyuSttMessage(part);
      if (parsed) {
        messages.push(parsed);
      }
    }

    cursor = frameEnd;
  }

  return {
    messages,
    remaining: cursor > 0 ? buffer.slice(cursor) : buffer,
  };
}

function parseDouyuSttMessage(message: string): DouyuSttMessage | null {
  const result: DouyuSttMessage = {};

  for (const field of message.split('/')) {
    if (!field) {
      continue;
    }
    const splitIndex = field.indexOf('@=');
    if (splitIndex < 0) {
      continue;
    }
    const key = field.slice(0, splitIndex);
    const value = field
      .slice(splitIndex + 2)
      .replace(/@S/g, '/')
      .replace(/@A/g, '@');
    result[key] = value;
  }

  return Object.keys(result).length ? result : null;
}

async function toUint8Array(data: Blob | ArrayBuffer | string): Promise<Uint8Array> {
  if (typeof data === 'string') {
    return new TextEncoder().encode(data);
  }
  if (data instanceof ArrayBuffer) {
    return new Uint8Array(data);
  }
  return new Uint8Array(await data.arrayBuffer());
}

function appendUint8Array(left: Uint8Array, right: Uint8Array): Uint8Array {
  if (!left.length) {
    return right;
  }
  if (!right.length) {
    return left;
  }
  const merged = new Uint8Array(left.length + right.length);
  merged.set(left);
  merged.set(right, left.length);
  return merged;
}

function stopDouyuDanmakuConnection(): void {
  clearDouyuDanmakuHeartbeatTimer();
  clearDouyuDanmakuReconnectTimer();
  if (currentDouyuDanmakuSocket) {
    currentDouyuDanmakuSocket.onopen = null;
    currentDouyuDanmakuSocket.onmessage = null;
    currentDouyuDanmakuSocket.onerror = null;
    currentDouyuDanmakuSocket.onclose = null;
    if (
      currentDouyuDanmakuSocket.readyState === WebSocket.OPEN
      || currentDouyuDanmakuSocket.readyState === WebSocket.CONNECTING
    ) {
      currentDouyuDanmakuSocket.close();
    }
    currentDouyuDanmakuSocket = null;
  }
}

function clearDouyuDanmakuHeartbeatTimer(): void {
  if (currentDouyuDanmakuHeartbeatTimer !== null) {
    window.clearInterval(currentDouyuDanmakuHeartbeatTimer);
    currentDouyuDanmakuHeartbeatTimer = null;
  }
}

function clearDouyuDanmakuReconnectTimer(): void {
  if (currentDouyuDanmakuReconnectTimer !== null) {
    window.clearTimeout(currentDouyuDanmakuReconnectTimer);
    currentDouyuDanmakuReconnectTimer = null;
  }
}

function getNativeDouyuDanmakuBridge(): Window['DTVDouyuDanmaku'] | null {
  if (typeof window === 'undefined') {
    return null;
  }
  return window.DTVDouyuDanmaku ?? null;
}

function buildDouyuFrontendDanmaku(
  payload: UnifiedRustDanmakuPayload,
  fallbackRoomId: string,
): DanmakuMessage {
  return {
    id: uuidv4(),
    nickname: payload.user || 'Unknown',
    content: payload.content || '',
    level: String(payload.user_level || 0),
    badgeLevel: payload.fans_club_level > 0 ? String(payload.fans_club_level) : undefined,
    room_id: payload.room_id || fallbackRoomId,
  };
}

function pushDouyuDanmakuToUi(
  frontendDanmaku: DanmakuMessage,
  resolveOverlay: () => DanmuOverlayInstance | null,
  danmakuMessagesRef: Ref<DanmakuMessage[]>,
  renderOptions?: DanmuRenderOptions,
): void {
  const shouldDisplay = renderOptions?.shouldDisplay ? renderOptions.shouldDisplay(frontendDanmaku) : true;
  const shouldAppend = renderOptions?.shouldAppendToList ? renderOptions.shouldAppendToList(frontendDanmaku) : true;
  const activeOverlay = resolveOverlay();

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
}

function startNativeDouyuDanmakuListener(
  nativeBridge: NonNullable<Window['DTVDouyuDanmaku']>,
  normalizedRoomId: string,
  fallbackRoomId: string,
  resolveOverlay: () => DanmuOverlayInstance | null,
  danmakuMessagesRef: Ref<DanmakuMessage[]>,
  renderOptions?: DanmuRenderOptions,
): () => void {
  const eventHandler = (event: Event) => {
    const customEvent = event as CustomEvent<string>;
    if (!customEvent.detail) {
      return;
    }
    try {
      const payload = JSON.parse(customEvent.detail) as UnifiedRustDanmakuPayload;
      const frontendDanmaku = buildDouyuFrontendDanmaku(payload, fallbackRoomId);
      pushDouyuDanmakuToUi(frontendDanmaku, resolveOverlay, danmakuMessagesRef, renderOptions);
    } catch (error) {
      console.warn('[DouyuPlayerHelper] Failed to parse native Douyu danmaku payload:', error);
    }
  };

  const statusHandler = (event: Event) => {
    const customEvent = event as CustomEvent<string>;
    postAndroidDebugLog('w', `[DouyuDanmaku] native status room=${normalizedRoomId} detail=${customEvent.detail || ''}`);
  };

  window.addEventListener('dtv-douyu-danmaku', eventHandler as EventListener);
  window.addEventListener('dtv-douyu-danmaku-status', statusHandler as EventListener);
  nativeBridge.start(normalizedRoomId);
  postAndroidDebugLog('i', `[DouyuDanmaku] native bridge start room=${normalizedRoomId}`);

  return () => {
    window.removeEventListener('dtv-douyu-danmaku', eventHandler as EventListener);
    window.removeEventListener('dtv-douyu-danmaku-status', statusHandler as EventListener);
    nativeBridge.stop();
    postAndroidDebugLog('i', `[DouyuDanmaku] native bridge stop room=${normalizedRoomId}`);
  };
}
