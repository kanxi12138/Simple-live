import { listen } from '@tauri-apps/api/event';
import type { Ref } from 'vue';

import { Platform as StreamingPlatform } from '../../platforms/common/types';
import { startBilibiliDanmakuListener, stopBilibiliDanmaku } from '../../platforms/bilibili/playerHelper';
import { startDouyinDanmakuListener, stopDouyinDanmaku } from '../../platforms/douyin/playerHelper';
import { startDouyuDanmakuListener, stopDouyuDanmaku } from '../../platforms/douyu/playerHelper';
import { startHuyaDanmakuListener, stopHuyaDanmaku } from '../../platforms/huya/playerHelper';

import type { DanmuUserSettings } from './constants';
import type { PlayerProps } from './watchers';
import type { DanmakuMessage, DanmuOverlayInstance } from './types';
import { getDanmuDensityInterval } from './constants';

export interface DanmakuManagerContext {
  danmakuMessages: Ref<DanmakuMessage[]>;
  isDanmuEnabled: Ref<boolean>;
  danmuSettings: DanmuUserSettings;
  isFullScreen: Ref<boolean>;
  isDanmakuListenerActive: Ref<boolean>;
  unlistenDanmakuFn: Ref<(() => void) | null>;
  props: PlayerProps;
}

const BLOCK_KEYWORDS_STORAGE = 'danmu_block_keywords';

const loadBlockedKeywords = (): string[] => {
  if (typeof window === 'undefined') {
    return [];
  }
  try {
    const raw = window.localStorage.getItem(BLOCK_KEYWORDS_STORAGE);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((v) => typeof v === 'string')
      .map((v) => v.trim().toLowerCase())
      .filter((v) => v.length > 0);
  } catch (err) {
    console.warn('Diagnostic: danmakuManager.ts:41 (details omitted)');
    return [];
  }
};

const isBlockedMessage = (message?: DanmakuMessage) => {
  if (!message || message.isSystem) {
    return false;
  }
  const content = (message.content || '').toLowerCase();
  if (!content) return false;
  const keywords = loadBlockedKeywords();
  if (!keywords.length) return false;
  return keywords.some((kw) => content.includes(kw));
};

const startListener = async (
  ctx: DanmakuManagerContext,
  platform: StreamingPlatform,
  roomId: string,
  getDanmuOverlay: () => DanmuOverlayInstance | null,
  generation: number,
) => {
  const session = getSession(ctx);
  if (generation !== session.generation) return;
  if (!roomId || ctx.isDanmakuListenerActive.value) {
    return;
  }

  ctx.isDanmakuListenerActive.value = true;
  const danmuOverlay = getDanmuOverlay();
  if (!danmuOverlay) {
    console.warn('Diagnostic: danmakuManager.ts:73 (details omitted)');
  }

  try {
    let ready = false;
    const announce = (status: string, message?: string) => {
      if (generation !== session.generation || (status === 'ready' && ready)) return;
      ready = status === 'ready';
      if (!ctx.isFullScreen.value) ctx.danmakuMessages.value.push({
        id: 'system-' + Date.now(), nickname: '系统消息', isSystem: true,
        content: status === 'ready' ? '弹幕连接成功！' : (message || '弹幕连接已断开，请刷新重试。'),
        type: status === 'ready' ? 'success' : 'error',
      });
    };
    session.unlistenStatus = await listen<{room_id: string; platform: string; status: string; message?: string}>(
      'danmaku-status', ({ payload }) => {
        if (payload.room_id === roomId && payload.platform.toUpperCase() === platform) announce(payload.status, payload.message);
      },
    );
    let lastOverlayEmitAt = 0;
    const renderOptions = {
      shouldDisplay: (message?: DanmakuMessage) => {
        if (generation !== session.generation || !ctx.isDanmuEnabled.value || isBlockedMessage(message)) {
          return false;
        }
        if (message?.isSystem) {
          return true;
        }
        const densityInterval = getDanmuDensityInterval(ctx.danmuSettings.density);
        if (densityInterval <= 0) {
          lastOverlayEmitAt = Date.now();
          return true;
        }
        const now = Date.now();
        if (now - lastOverlayEmitAt < densityInterval) {
          return false;
        }
        lastOverlayEmitAt = now;
        return true;
      },
      shouldAppendToList: () => {
        if (generation !== session.generation) return false;
        announce('ready');
        return !ctx.isFullScreen.value;
      },
      buildCommentOptions: () => ({
        duration: ctx.danmuSettings.duration,
        mode: ctx.danmuSettings.mode,
        style: {
          fontSize: ctx.danmuSettings.fontSize,
        },
      }),
    };
    let stopFn: (() => void) | null = null;
    if (platform === StreamingPlatform.DOUYU) {
      stopFn = await startDouyuDanmakuListener(roomId, getDanmuOverlay, ctx.danmakuMessages, renderOptions);
    } else if (platform === StreamingPlatform.DOUYIN) {
      stopFn = await startDouyinDanmakuListener(roomId, getDanmuOverlay, ctx.danmakuMessages, renderOptions);
    } else if (platform === StreamingPlatform.HUYA) {
      stopFn = await startHuyaDanmakuListener(roomId, getDanmuOverlay, ctx.danmakuMessages, renderOptions);
    } else if (platform === StreamingPlatform.BILIBILI) {
      stopFn = await startBilibiliDanmakuListener(roomId, getDanmuOverlay, ctx.danmakuMessages, ctx.props.cookie || undefined, renderOptions);
    }

    if (stopFn) {
      ctx.unlistenDanmakuFn.value = stopFn;

    } else {
      console.warn('Diagnostic: danmakuManager.ts:141 (details omitted)');
      ctx.isDanmakuListenerActive.value = false;
    }
  } catch (error) {
    console.error('Diagnostic: danmakuManager.ts:145 (details omitted)');
    ctx.isDanmakuListenerActive.value = false;

    if (!ctx.isFullScreen.value) {
      const errorMessage: DanmakuMessage = {
        id: `system-err-${Date.now()}`,
        nickname: '系统消息',
        content: '弹幕连接失败，请尝试刷新播放器。',
        isSystem: true,
        type: 'error',
        color: '#dc3545',
      };
      ctx.danmakuMessages.value.push(errorMessage);
    }
  }
};

const stopListener = async (
  ctx: DanmakuManagerContext,
  platform?: StreamingPlatform,
  roomId?: string | null | undefined,
) => {
  getSession(ctx).unlistenStatus?.();
  getSession(ctx).unlistenStatus = null;
  platform ??= ctx.props.platform;
  roomId ??= ctx.props.roomId;
  if (platform) {
    if (platform === StreamingPlatform.DOUYU) {
      await stopDouyuDanmaku(roomId!, ctx.unlistenDanmakuFn.value);
    } else if (platform === StreamingPlatform.DOUYIN) {
      await stopDouyinDanmaku(ctx.unlistenDanmakuFn.value);
    } else if (platform === StreamingPlatform.HUYA) {
      await stopHuyaDanmaku(ctx.unlistenDanmakuFn.value);
    } else if (platform === StreamingPlatform.BILIBILI) {
      await stopBilibiliDanmaku(ctx.unlistenDanmakuFn.value);
    }
    if (ctx.unlistenDanmakuFn.value) {
      ctx.unlistenDanmakuFn.value = null;
    }
  } else if (ctx.unlistenDanmakuFn.value) {
    console.warn('Diagnostic: danmakuManager.ts:185 (details omitted)');
    try {
      ctx.unlistenDanmakuFn.value();
      ctx.unlistenDanmakuFn.value = null;
    } catch (error) {
      console.error('Diagnostic: danmakuManager.ts:190 (details omitted)');
      ctx.unlistenDanmakuFn.value = null;
    }
  }

  ctx.isDanmakuListenerActive.value = false;
};

interface ListenerSession {
  generation: number;
  queue: Promise<void>;
  unlistenStatus: (() => void) | null;
}
const sessions = new WeakMap<DanmakuManagerContext, ListenerSession>();
const getSession = (context: DanmakuManagerContext): ListenerSession => {
  let session = sessions.get(context);
  if (!session) {
    session = { generation: 0, queue: Promise.resolve(), unlistenStatus: null };
    sessions.set(context, session);
  }
  return session;
};

/** Serializes starts with pending cleanup; only the current generation may render. */
export const startCurrentDanmakuListener = (
  context: DanmakuManagerContext, platform: StreamingPlatform, roomId: string,
  getOverlay: () => DanmuOverlayInstance | null,
): Promise<void> => {
  const session = getSession(context);
  const generation = session.generation;
  session.queue = session.queue.then(() => startListener(context, platform, roomId, getOverlay, generation));
  return session.queue;
};

/** Invalidates callbacks immediately and queues backend cleanup after any pending start. */
export const stopCurrentDanmakuListener = (
  context: DanmakuManagerContext, platform?: StreamingPlatform, roomId?: string | null,
): Promise<void> => {
  const session = getSession(context);
  session.generation += 1;
  const cleanup = () => stopListener(context, platform, roomId);
  session.queue = session.queue.then(cleanup, cleanup);
  return session.queue;
};
