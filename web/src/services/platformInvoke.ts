import { invoke as nativeInvoke } from '@tauri-apps/api/core';

type Arguments = Record<string, unknown>;
type Queue = { active: number; waiting: Array<() => void> };
const queues = new Map<string, Queue>();
const pending = new Map<string, Promise<unknown>>();
const playbackCache = new Map<string, unknown>();
let playbackGeneration = 0;

const platformFor = (command: string): string | undefined => {
  const named = ['bilibili', 'douyin', 'douyu', 'huya'].find((name) => command.includes(name));
  if (named) return named;
  if (['search_anchor', 'get_stream_url_cmd', 'get_stream_url_with_quality_cmd',
    'fetch_categories', 'fetch_three_cate', 'fetch_live_list', 'fetch_live_list_for_cate3'].includes(command)) return 'douyu';
  return undefined;
};

/** Drops short-lived playback results on stop, room change, or page exit. */
export const clearPlaybackCache = (): void => {
  playbackGeneration += 1;
  playbackCache.clear();
};

const runLimited = async <Result>(platform: string, operation: () => Promise<Result>): Promise<Result> => {
  let queue = queues.get(platform);
  if (!queue) {
    queue = { active: 0, waiting: [] };
    queues.set(platform, queue);
  }
  if (queue.active >= 2) {
    if (queue.waiting.length >= 20) throw new Error('请求过于频繁，请稍后重试。');
    await new Promise<void>((resolve) => queue.waiting.push(resolve));
  } else {
    queue.active += 1;
  }
  try {
    return await operation();
  } finally {
    // Keep rapid sequential clicks from hammering a platform.
    await new Promise<void>((resolve) => window.setTimeout(resolve, 250));
    const next = queue.waiting.shift();
    if (next) next();
    else queue.active -= 1;
  }
};

/** Coalesces identical platform IPC calls and bounds concurrency. Never retries platform refusals. */
export const invoke = <Result>(command: string, args?: Arguments): Promise<Result> => {
  if (command === 'stop_proxy') clearPlaybackCache();
  const platform = platformFor(command);
  if (!platform || /^(start_|stop_|get_bilibili_cookie|bootstrap_bilibili_cookie)/.test(command)) {
    return nativeInvoke<Result>(command, args);
  }
  const key = JSON.stringify([command, args]);
  const isPlayback = /stream_url|play_url|play_info|get_huya_unified/.test(command);
  if (isPlayback && playbackCache.has(key)) return Promise.resolve(playbackCache.get(key) as Result);
  const existing = pending.get(key);
  if (existing) return existing as Promise<Result>;
  const generation = playbackGeneration;
  const request = runLimited(platform, async () => {
    if (isPlayback && generation !== playbackGeneration) throw new Error('播放请求已取消。');
    const result = await nativeInvoke<Result>(command, args);
    if (isPlayback && generation === playbackGeneration) {
      playbackCache.set(key, result);
      window.setTimeout(() => {
        if (playbackCache.get(key) === result) playbackCache.delete(key);
      }, 5000);
    }
    return result;
  }).catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    if (/未开播|不在线|offline/i.test(message)) throw new Error('主播未开播。');
    if (/限制|拒绝|权限|401|403|429|captcha|verify/i.test(message)) throw new Error('平台限制访问，暂不可用，请稍后手动重试。');
    throw new Error('平台请求失败，暂不可用，请稍后手动重试。');
  }).finally(() => pending.delete(key));
  pending.set(key, request);
  return request;
};

window.addEventListener('pagehide', clearPlaybackCache);
