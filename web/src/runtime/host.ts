import { openUrl } from '@tauri-apps/plugin-opener';

export const isBrowser = typeof window !== 'undefined';

export const isTauriRuntime = (): boolean => {
  if (!isBrowser) return false;
  const scope = window as typeof window & {
    __TAURI__?: unknown;
    __TAURI_INTERNALS__?: unknown;
  };
  return Boolean(scope.__TAURI__ || scope.__TAURI_INTERNALS__);
};

export const isLikelyMobileViewport = (): boolean => {
  if (!isBrowser) return true;
  return window.innerWidth <= 960;
};

export const setNativeTheme = async (theme: 'light' | 'dark'): Promise<void> => {
  if (!isTauriRuntime()) return;
  try {
    const mod = await import('@tauri-apps/api/webviewWindow');
    const current = mod.WebviewWindow.getCurrent();
    await current.setTheme(theme);
  } catch (error) {
    console.warn('[runtime/host] setNativeTheme skipped:', error);
  }
};

export const openExternal = async (url: string): Promise<void> => {
  if (!url) return;
  try {
    await openUrl(url);
  } catch (error) {
    console.warn('[runtime/host] opener failed, falling back to window.open:', error);
    if (isBrowser) {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  }
};

export const copyTextToClipboard = async (text: string): Promise<void> => {
  if (!isBrowser) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    document.body.removeChild(textarea);
  }
};

export const exportConfigWithBrowser = async (filename: string, contents: string): Promise<string | null> => {
  if (!isBrowser) return null;
  const blob = new Blob([contents], { type: 'application/json;charset=utf-8' });
  const objectUrl = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = objectUrl;
  anchor.download = filename;
  anchor.rel = 'noopener';
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  window.setTimeout(() => URL.revokeObjectURL(objectUrl), 1000);
  return filename;
};

export const importConfigWithBrowser = async (): Promise<{ path: string; content: string } | null> => {
  if (!isBrowser) return null;

  return new Promise((resolve, reject) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json,application/json';
    input.style.display = 'none';
    input.onchange = async () => {
      const file = input.files?.[0];
      document.body.removeChild(input);
      if (!file) {
        resolve(null);
        return;
      }
      try {
        const content = await file.text();
        resolve({
          path: file.name,
          content,
        });
      } catch (error) {
        reject(error);
      }
    };
    document.body.appendChild(input);
    input.click();
  });
};
