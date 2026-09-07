import { Platform as StreamingPlatform } from '../../platforms/common/types';
import type { LineOption } from './plugins';

/** Reads a saved line; availability is checked against the next platform response. */
export const resolveStoredLine = (platform?: StreamingPlatform | null): string | null => {
  if (!platform || typeof window === 'undefined') return null;
  try {
    return window.localStorage.getItem(`${platform}_preferred_line`);
  } catch (error) {
    console.warn('[Player] Failed to read stored line preference:', error);
    return null;
  }
};

export const persistLinePreference = (platform?: StreamingPlatform | null, lineKey?: string | null) => {
  if (!platform || !lineKey || typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(`${platform}_preferred_line`, lineKey);
  } catch (error) {
    console.warn('[Player] Failed to persist line preference:', error);
  }
};

export const getLineLabel = (lineOptions: LineOption[], key?: string | null): string => {
  if (!key) {
    return '线路';
  }
  const option = lineOptions.find((item) => item.key === key);
  return option?.label ?? '线路';
};
