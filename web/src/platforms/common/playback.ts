import type { StreamVariant } from './types';

/** A real platform option and the label shown by the existing line control. */
export interface PlaybackLine {
  key: string;
  label: string;
}

/** A selected stream and the actual options returned for its room. */
export interface PlaybackConfig {
  streamUrl: string;
  streamType: string | undefined;
  qualities?: string[];
  lines?: PlaybackLine[];
  selectedQuality?: string;
  selectedLine?: string | null;
  headers?: Record<string, string>;
}

/**
 * Maps returned variants into the existing controls without inventing options.
 * @param variants Platform-provided streams.
 * @param quality Saved or requested quality name.
 * @param line Saved or requested line key.
 * @param qualities Optional names of qualities fetched separately by the platform.
 * @returns The selected URL, format and available controls.
 * @throws {Error} When no usable stream was returned.
 */
export const selectPlaybackVariant = (
  variants: StreamVariant[],
  quality: string,
  line?: string | null,
  qualities?: string[],
): PlaybackConfig => {
  const available = variants.filter((variant) => variant.url && variant.desc);
  const selectedQuality = available.find((variant) => variant.desc === quality)?.desc
    ?? available[0]?.desc;
  if (!selectedQuality) throw new Error('平台未返回可用的画质和播放地址。');
  const candidates = available.filter((variant) => variant.desc === selectedQuality);
  const lines = candidates.map((variant, index) => ({
    key: `${variant.format || 'flv'}:${index}`,
    label: `线路 ${index + 1} · ${isHlsVariant(variant) ? 'HLS' : 'FLV'}`,
  }));
  const selectedIndex = Math.max(0, lines.findIndex((option) => option.key === line));
  const selected = candidates[selectedIndex];
  return {
    streamUrl: selected.url,
    streamType: isHlsVariant(selected) ? 'hls' : 'flv',
    qualities: qualities?.length ? qualities : [...new Set(available.map((variant) => variant.desc as string))],
    lines,
    selectedQuality,
    selectedLine: lines[selectedIndex].key,
  };
};

const isHlsVariant = (variant: StreamVariant): boolean => (
  /hls/i.test(variant.protocol || '')
  || /^(hls|ts|fmp4|mp4|m4s)$/i.test(variant.format || '')
  || variant.url.includes('.m3u8')
);
