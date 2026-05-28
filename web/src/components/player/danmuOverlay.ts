import DanmuJs from 'danmu.js';
import type Player from 'xgplayer';

import { sanitizeDanmuArea, sanitizeDanmuOpacity } from './constants';
import type { DanmuOverlayInstance } from './types';
import type { DanmuUserSettings } from './constants';

const isAndroidRuntime = typeof navigator !== 'undefined' && /android/i.test(navigator.userAgent || '');

interface FallbackDanmuState {
  host: HTMLElement;
  layer: HTMLDivElement;
  playing: boolean;
  visibleModes: Set<string>;
  opacity: number;
  fontSize: number;
  duration: number;
  areaEnd: number;
  nextRow: number;
  cleanup: Set<() => void>;
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const createFallbackDanmuOverlay = (
  overlayHost: HTMLElement,
  danmuSettings: DanmuUserSettings,
  isDanmuEnabled: boolean,
): DanmuOverlayInstance => {
  const layer = document.createElement('div');
  layer.className = 'player-danmu-overlay-fallback';
  Object.assign(layer.style, {
    position: 'absolute',
    inset: '0',
    width: '100%',
    height: '100%',
    overflow: 'hidden',
    pointerEvents: 'none',
    zIndex: '20',
  });
  overlayHost.appendChild(layer);

  const state: FallbackDanmuState = {
    host: overlayHost,
    layer,
    playing: isDanmuEnabled,
    visibleModes: new Set(['scroll', 'top', 'bottom']),
    opacity: isDanmuEnabled ? sanitizeDanmuOpacity(danmuSettings.opacity) : 0,
    fontSize: parseInt(danmuSettings.fontSize, 10) || 20,
    duration: Math.max(4000, danmuSettings.duration),
    areaEnd: sanitizeDanmuArea(danmuSettings.area),
    nextRow: 0,
    cleanup: new Set(),
  };

  const applyContainerOpacity = () => {
    state.layer.style.opacity = String(state.opacity);
  };

  const computeRowTop = (mode: string) => {
    const rect = state.host.getBoundingClientRect();
    const usableHeight = Math.max(40, rect.height * clamp(state.areaEnd, 0.2, 1));
    const lineHeight = Math.max(28, state.fontSize + 12);
    const rows = Math.max(1, Math.floor(usableHeight / lineHeight));
    const rowIndex = state.nextRow % rows;
    state.nextRow += 1;

    if (mode === 'top') {
      return rowIndex * lineHeight;
    }
    if (mode === 'bottom') {
      return Math.max(0, usableHeight - (rowIndex + 1) * lineHeight);
    }
    return rowIndex * lineHeight;
  };

  const mountComment = (comment: { txt: string; duration?: number; mode?: string; style?: Record<string, string> }) => {
    const mode = comment.mode || 'scroll';
    if (!state.playing || !state.visibleModes.has(mode)) {
      return;
    }

    const node = document.createElement('div');
    node.textContent = comment.txt;
    const duration = Math.max(4000, comment.duration ?? state.duration);
    const top = computeRowTop(mode);
    const baseColor = comment.style?.color || '#FFFFFF';

    Object.assign(node.style, {
      position: 'absolute',
      left: mode === 'scroll' ? '100%' : '50%',
      top: `${top}px`,
      transform: mode === 'scroll' ? 'translate3d(0, 0, 0)' : 'translateX(-50%)',
      whiteSpace: 'nowrap',
      fontSize: `${state.fontSize}px`,
      fontWeight: '700',
      lineHeight: '1.2',
      color: baseColor,
      textShadow: '0 1px 2px rgba(0,0,0,0.85), 0 0 6px rgba(0,0,0,0.45)',
      willChange: 'transform, opacity',
      opacity: '1',
      transition: mode === 'scroll'
        ? `transform ${duration}ms linear`
        : `opacity 280ms ease`,
    });

    state.layer.appendChild(node);

    const cleanup = () => {
      if (node.parentElement) {
        node.parentElement.removeChild(node);
      }
      state.cleanup.delete(cleanup);
    };
    state.cleanup.add(cleanup);

    if (mode === 'scroll') {
      requestAnimationFrame(() => {
        const hostWidth = state.host.getBoundingClientRect().width || window.innerWidth || 360;
        const width = node.getBoundingClientRect().width || 120;
        node.style.transform = `translate3d(-${Math.ceil(hostWidth + width)}px, 0, 0)`;
      });
      window.setTimeout(cleanup, duration + 500);
    } else {
      window.setTimeout(() => {
        node.style.opacity = '0';
      }, Math.max(1200, duration - 600));
      window.setTimeout(cleanup, duration + 500);
    }
  };

  applyContainerOpacity();

  return {
    sendComment: (comment) => {
      mountComment(comment);
    },
    play: () => {
      state.playing = true;
    },
    pause: () => {
      state.playing = false;
    },
    stop: () => {
      state.playing = false;
      state.cleanup.forEach((fn) => fn());
      state.cleanup.clear();
      state.layer.innerHTML = '';
    },
    start: () => {
      state.playing = true;
    },
    hide: (mode?: string) => {
      if (mode) {
        state.visibleModes.delete(mode);
      } else {
        state.visibleModes.clear();
      }
    },
    show: (mode?: string) => {
      if (mode) {
        state.visibleModes.add(mode);
      } else {
        state.visibleModes = new Set(['scroll', 'top', 'bottom']);
      }
    },
    setOpacity: (opacity: number) => {
      state.opacity = opacity;
      applyContainerOpacity();
    },
    setFontSize: (size: number | string) => {
      const parsed = typeof size === 'number' ? size : parseInt(size, 10);
      if (!Number.isNaN(parsed) && parsed > 0) {
        state.fontSize = parsed;
      }
    },
    setAllDuration: (_mode: string, duration: number) => {
      if (duration > 0) {
        state.duration = duration;
      }
    },
    setArea: (area: { start: number; end: number; lines?: number }) => {
      state.areaEnd = clamp(area.end, 0.2, 1);
    },
    setPlayRate: () => {},
  };
};

export const ensureDanmuOverlayHost = (player: Player): HTMLElement | null => {
  const root = player.root as HTMLElement | undefined;
  if (!root) {
    return null;
  }

  let host = root.querySelector('.player-danmu-overlay') as HTMLElement | null;
  if (!host) {
    host = document.createElement('div');
    host.className = 'player-danmu-overlay';
  }

  const videoContainer = root.querySelector('xg-video-container');
  if (videoContainer && host.parentElement !== videoContainer) {
    videoContainer.appendChild(host);
  } else if (!videoContainer && host.parentElement !== root) {
    root.appendChild(host);
  } else if (!host.parentElement) {
    root.appendChild(host);
  }

  return host;
};

export const applyDanmuOverlayPreferences = (
  overlay: DanmuOverlayInstance | null,
  danmuSettings: DanmuUserSettings,
  isDanmuEnabled: boolean,
  playerRoot?: HTMLElement | null,
) => {
  if (!overlay) {
    return;
  }
  const host = playerRoot?.querySelector('.player-danmu-overlay') as HTMLElement | null;
  const fontSizeValue = parseInt(danmuSettings.fontSize, 10);
  if (!Number.isNaN(fontSizeValue)) {
    try {
      overlay.setFontSize?.(fontSizeValue);
    } catch (error) {
      console.warn('[Player] Failed to apply danmu font size:', error);
    }
  }
  try {
    const areaValue = sanitizeDanmuArea(danmuSettings.area);
    overlay.setArea?.({ start: 0, end: areaValue });
  } catch (error) {
    console.warn('[Player] Failed to apply danmu area:', error);
  }
  try {
    overlay.setAllDuration?.('scroll', danmuSettings.duration);
    overlay.setAllDuration?.('top', danmuSettings.duration);
    overlay.setAllDuration?.('bottom', danmuSettings.duration);
  } catch (error) {
    // Non-critical for players that do not support bulk duration updates
  }
  try {
    const normalizedOpacity = sanitizeDanmuOpacity(danmuSettings.opacity);
    const nextOpacity = isDanmuEnabled ? normalizedOpacity : 0;
    overlay.setOpacity?.(nextOpacity);
    host?.style.setProperty('--danmu-opacity', String(nextOpacity));
  } catch (error) {
    // Non-critical
  }
};

export const syncDanmuEnabledState = (
  overlay: DanmuOverlayInstance | null,
  danmuSettings: DanmuUserSettings,
  isDanmuEnabled: boolean,
  playerRoot?: HTMLElement | null,
) => {
  if (!overlay) {
    return;
  }
  const normalizedOpacity = sanitizeDanmuOpacity(danmuSettings.opacity);
  const targetOpacity = isDanmuEnabled ? normalizedOpacity : 0;
  try {
    if (isDanmuEnabled) {
      overlay.play?.();
      overlay.show?.('scroll');
      overlay.show?.('top');
      overlay.show?.('bottom');
    } else {
      overlay.pause?.();
    }
    overlay.setOpacity?.(targetOpacity);
    const host = playerRoot?.querySelector('.player-danmu-overlay') as HTMLElement | null;
    host?.style.setProperty('--danmu-opacity', String(targetOpacity));
  } catch (error) {
    console.warn('[Player] Failed updating danmu enabled state:', error);
  }
};

export const createDanmuOverlay = (
  player: Player | null,
  danmuSettings: DanmuUserSettings,
  isDanmuEnabled: boolean,
): DanmuOverlayInstance | null => {
  if (!player) {
    return null;
  }

  const overlayHost = ensureDanmuOverlayHost(player);
  if (!overlayHost) {
    return null;
  }

  overlayHost.innerHTML = '';
  overlayHost.style.setProperty('--danmu-opacity', String(isDanmuEnabled ? sanitizeDanmuOpacity(danmuSettings.opacity) : 0));

  if (isAndroidRuntime) {
    return createFallbackDanmuOverlay(overlayHost, danmuSettings, isDanmuEnabled);
  }

  try {
    const overlay = new DanmuJs({
      container: overlayHost,
      player: player.video || player.media || undefined,
      comments: [],
      mouseControl: false,
      defaultOff: false,
      channelSize: 36,
      containerStyle: {
        pointerEvents: 'none',
      },
    });

    overlay.start?.();
    applyDanmuOverlayPreferences(overlay, danmuSettings, isDanmuEnabled, player.root as HTMLElement);
    syncDanmuEnabledState(overlay, danmuSettings, isDanmuEnabled, player.root as HTMLElement);
    return overlay;
  } catch (error) {
    console.error('[Player] Failed to initialize danmu.js overlay:', error);
    return null;
  }
};
