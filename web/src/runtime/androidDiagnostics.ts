const DEBUG_TAG = '[AndroidDiagnostics]';
const MAX_MESSAGE_LENGTH = 2000;

const getDebugBridge = (): Window['DTVDebug'] | null => {
  if (typeof window === 'undefined') {
    return null;
  }
  return window.DTVDebug ?? null;
};

export const postAndroidDebugLog = (level: 'd' | 'i' | 'w' | 'e', message: string): void => {
  const bridge = getDebugBridge();
  if (!bridge || !message) {
    return;
  }

  try {
    bridge.log(level, message.slice(0, MAX_MESSAGE_LENGTH));
  } catch (error) {
    console.warn(`${DEBUG_TAG} Failed to forward debug log:`, error);
  }
};

const shouldTrackUrl = (url: string): boolean => {
  return url.includes('127.0.0.1')
    || url.includes('localhost')
    || url.includes('bilibili')
    || url.includes('bilivideo')
    || url.includes('hdslb')
    || url.includes('douyu')
    || url.includes('huya')
    || url.includes('hycdn')
    || url.includes('huyaimg');
};

const describeRequestTarget = (input: RequestInfo | URL): string => {
  if (typeof input === 'string') {
    return input;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input.url;
};

const installFetchDiagnostics = (): void => {
  if (typeof window === 'undefined' || typeof window.fetch !== 'function') {
    return;
  }

  const originalFetch = window.fetch.bind(window);
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const requestUrl = describeRequestTarget(input);
    try {
      const response = await originalFetch(input, init);
      if (!response.ok && shouldTrackUrl(requestUrl)) {
        postAndroidDebugLog('w', `${DEBUG_TAG} fetch ${response.status} ${requestUrl}`);
      }
      return response;
    } catch (error) {
      if (shouldTrackUrl(requestUrl)) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        postAndroidDebugLog('e', `${DEBUG_TAG} fetch failed ${requestUrl} :: ${errorMessage}`);
      }
      throw error;
    }
  };
};

const installXhrDiagnostics = (): void => {
  if (typeof window === 'undefined' || typeof window.XMLHttpRequest === 'undefined') {
    return;
  }

  const originalOpen = window.XMLHttpRequest.prototype.open;
  window.XMLHttpRequest.prototype.open = function patchedOpen(
    method: string,
    url: string | URL,
    async?: boolean,
    username?: string | null,
    password?: string | null,
  ): void {
    const requestUrl = typeof url === 'string' ? url : url.toString();
    this.addEventListener('error', () => {
      if (shouldTrackUrl(requestUrl)) {
        postAndroidDebugLog('e', `${DEBUG_TAG} xhr failed ${method} ${requestUrl}`);
      }
    });
    this.addEventListener('load', () => {
      if (this.status >= 400 && shouldTrackUrl(requestUrl)) {
        postAndroidDebugLog('w', `${DEBUG_TAG} xhr ${this.status} ${method} ${requestUrl}`);
      }
    });
    originalOpen.call(this, method, url, async ?? true, username ?? undefined, password ?? undefined);
  };
};

const installResourceDiagnostics = (): void => {
  if (typeof window === 'undefined') {
    return;
  }

  window.addEventListener('error', (event: Event) => {
    const target = event.target;
    if (!(target instanceof HTMLImageElement || target instanceof HTMLVideoElement || target instanceof HTMLSourceElement)) {
      return;
    }

    const resourceUrl = 'currentSrc' in target ? target.currentSrc : '';
    const fallbackUrl = 'src' in target ? target.src : '';
    const trackedUrl = resourceUrl || fallbackUrl;
    if (!trackedUrl || !shouldTrackUrl(trackedUrl)) {
      return;
    }

    const resourceType = target instanceof HTMLImageElement
      ? 'img'
      : target instanceof HTMLVideoElement
        ? 'video'
        : 'source';
    postAndroidDebugLog('e', `${DEBUG_TAG} ${resourceType} error ${trackedUrl}`);
  }, true);

  window.addEventListener('unhandledrejection', (event: PromiseRejectionEvent) => {
    const reason = event.reason instanceof Error ? event.reason.message : String(event.reason);
    if (reason.includes('Failed to fetch')) {
      postAndroidDebugLog('e', `${DEBUG_TAG} unhandled rejection ${reason}`);
    }
  });
};

const installAndroidDiagnostics = (): void => {
  if (typeof window === 'undefined') {
    return;
  }

  if (!getDebugBridge()) {
    return;
  }

  postAndroidDebugLog('i', `${DEBUG_TAG} installed`);
  installFetchDiagnostics();
  installXhrDiagnostics();
  installResourceDiagnostics();
};

installAndroidDiagnostics();
