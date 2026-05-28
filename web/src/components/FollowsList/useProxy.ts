import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export function useImageProxy() {
  const proxyBase = ref('');
  const proxyRequiredPlatforms = new Set(['BILIBILI', 'HUYA']);

  function normalizeImageUrl(url: string | null | undefined): string {
    const raw = (url || '').trim();
    if (!raw) return '';
    if (raw.startsWith('//')) {
      return `https:${raw}`;
    }
    if (raw.startsWith('http://')) {
      return `https://${raw.slice('http://'.length)}`;
    }
    return raw;
  }

  async function ensureProxyStarted(): Promise<boolean> {
    try {
      if (!proxyBase.value) {
        const base = await invoke<string>('start_static_proxy_server');
        proxyBase.value = base || '';
      }
      return !!proxyBase.value;
    } catch (e) {
      console.warn('[useImageProxy] ensureProxyStarted failed:', e);
      return false;
    }
  }

  function proxify(url: string | null | undefined): string {
    const u = normalizeImageUrl(url);
    if (!u) return '';
    try {
      const parsed = new URL(u);
      if (parsed.hostname === '127.0.0.1' || parsed.hostname === 'localhost') {
        return u;
      }
    } catch {
      return u;
    }
    if (!proxyBase.value) return u;
    const base = proxyBase.value.endsWith('/') ? proxyBase.value.slice(0, -1) : proxyBase.value;
    return `${base}/image?url=${encodeURIComponent(u)}`;
  }

  function getAvatarSrc(platform: string, avatarUrl?: string | null) {
    const u = normalizeImageUrl(avatarUrl);
    if (!u) return '';
    if (proxyRequiredPlatforms.has(platform) && !proxyBase.value) {
      return '';
    }
    if (proxyRequiredPlatforms.has(platform)) {
      return proxify(u);
    }
    return u;
  }

  return { proxyBase, ensureProxyStarted, proxify, getAvatarSrc, normalizeImageUrl };
}
