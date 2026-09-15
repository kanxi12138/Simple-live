/** Native diagnostics never receive URLs, credentials, chat contents, or user identifiers. */
export const postAndroidDebugLog = (level: 'd' | 'i' | 'w' | 'e', _message: string): void => {
  if (import.meta.env.DEV) window.DTVDebug?.log(level, 'Playback diagnostic event (details omitted)');
};
