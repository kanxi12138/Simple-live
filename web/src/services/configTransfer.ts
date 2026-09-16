export const DTV_CONFIG_KIND = 'dtv-config';
export const DTV_CONFIG_VERSION = 1;

type ConfigSource = {
  appVersion?: string | null;
  client: string;
};

export type PortableConfigEntries = Record<string, string>;

export interface DtvConfigPayload {
  kind: typeof DTV_CONFIG_KIND;
  version: typeof DTV_CONFIG_VERSION;
  exportedAt: string;
  source: ConfigSource;
  entries: PortableConfigEntries;
}

const EXACT_EXPORTABLE_KEYS = new Set<string>([
  'danmu_block_keywords',
  'dtv_custom_categories_v1',
  'dtv_danmu_preferences_v1',
  'dtv_player_danmu_collapsed',
  'dtv_player_volume_v1',
  'followedStreamers',
  'followFolders',
  'followListOrder',
  'theme_preference',
]);

const PORTABLE_PLATFORM_KEYS = ['DOUYU', 'DOUYIN', 'HUYA', 'BILIBILI'] as const;
const EXPORTABLE_KEY_PATTERNS = [
  new RegExp(`^(${PORTABLE_PLATFORM_KEYS.join('|')})_preferred_quality$`),
  new RegExp(`^(${PORTABLE_PLATFORM_KEYS.join('|')})_preferred_line$`),
];

const isRecord = (value: unknown): value is Record<string, unknown> => {
  return !!value && typeof value === 'object' && !Array.isArray(value);
};

const textValue = (value: unknown): value is string => typeof value === 'string';
const textArray = (value: unknown): value is string[] => Array.isArray(value) && value.every(textValue);
const removedPlatforms = new Set(['KUAISHOU', 'NETEASECC', 'CUSTOM_M3U8']);
const optionalFields = (value: Record<string, unknown>, keys: string[], valid: (item: unknown) => boolean) =>
  keys.every((key) => value[key] === undefined || valid(value[key]));
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
const streamerValid = (value: unknown): boolean => isRecord(value)
  && textValue(value.platform) && (PORTABLE_PLATFORM_KEYS.includes(value.platform as typeof PORTABLE_PLATFORM_KEYS[number]) || removedPlatforms.has(value.platform))
  && ['id', 'nickname', 'avatarUrl'].every((key) => textValue(value[key]))
  && optionalFields(value, ['displayName', 'roomTitle', 'currentRoomId'], textValue)
  && optionalFields(value, ['lastUpdated', 'followedAt'], finite)
  && optionalFields(value, ['isPinned'], (item) => typeof item === 'boolean')
  && optionalFields(value, ['isLive'], (item) => item === null || typeof item === 'boolean');
const folderValid = (value: unknown): boolean => isRecord(value)
  && textValue(value.id) && textValue(value.name) && textArray(value.streamerIds)
  && optionalFields(value, ['expanded'], (item) => typeof item === 'boolean');

/** Validates known persisted values before touching the user's existing storage. */
const validateEntry = (key: string, raw: string): string => {
  let valid = false;
  try {
    if (key === 'theme_preference') valid = ['light', 'dark', 'system'].includes(raw);
    else if (key.endsWith('_preferred_quality') || key.endsWith('_preferred_line')) valid = raw.trim().length > 0;
    else if (key === 'dtv_player_volume_v1') valid = raw.trim() !== '' && Number.isFinite(Number(raw)) && Number(raw) >= 0 && Number(raw) <= 1;
    else {
      const value: unknown = JSON.parse(raw);
      if (key === 'danmu_block_keywords') valid = textArray(value);
      else if (key === 'dtv_player_danmu_collapsed') valid = typeof value === 'boolean';
      else if (key === 'followedStreamers') valid = Array.isArray(value) && value.every(streamerValid);
      else if (key === 'followFolders') valid = Array.isArray(value) && value.every(folderValid);
      else if (key === 'followListOrder') valid = Array.isArray(value) && value.every((item) => isRecord(item)
        && (item.type === 'folder' ? folderValid(item.data) : item.type === 'streamer' && streamerValid(item.data)));
      else if (key === 'dtv_custom_categories_v1') valid = Array.isArray(value) && value.every((item) => isRecord(item)
        && textValue(item.platform) && ['douyu', 'douyin', 'huya', 'bilibili'].includes(item.platform)
        && textValue(item.cate2Name) && optionalFields(item, ['key', 'cate1Name', 'cate1Href', 'cate2Href', 'douyuId'], textValue));
      else if (key === 'dtv_danmu_preferences_v1' && isRecord(value) && isRecord(value.settings)) {
        const settings = value.settings;
        valid = typeof value.enabled === 'boolean'
          && optionalFields(settings, ['fontSize'], (item) => textValue(item) && /^\d+(?:\.\d+)?px$/.test(item) && parseFloat(item) > 0)
          && optionalFields(settings, ['duration'], (item) => finite(item) && item > 0)
          && optionalFields(settings, ['area'], (item) => finite(item) && item > 0 && item <= 1)
          && optionalFields(settings, ['opacity'], (item) => finite(item) && item >= 0.2 && item <= 1)
          && optionalFields(settings, ['mode'], (item) => ['scroll', 'top', 'bottom'].includes(String(item)))
          && optionalFields(settings, ['density'], (item) => ['dense', 'medium', 'sparse'].includes(String(item)));
      }
    }
  } catch { valid = false; }
  if (!valid) throw new Error(`配置项 ${key} 内容无效，未导入。`);
  return raw;
};

export const isExportableStorageKey = (key: string): boolean => {
  if (EXACT_EXPORTABLE_KEYS.has(key)) {
    return true;
  }
  return EXPORTABLE_KEY_PATTERNS.some((pattern) => pattern.test(key));
};

const sortEntries = (entries: PortableConfigEntries): PortableConfigEntries => {
  return Object.keys(entries)
    .sort((left, right) => left.localeCompare(right))
    .reduce<PortableConfigEntries>((result, key) => {
      result[key] = entries[key];
      return result;
    }, {});
};

export const collectPortableConfigEntries = (storage: Storage): PortableConfigEntries => {
  const entries: PortableConfigEntries = {};
  for (let index = 0; index < storage.length; index += 1) {
    const key = storage.key(index);
    if (!key || !isExportableStorageKey(key)) {
      continue;
    }
    const value = storage.getItem(key);
    if (typeof value === 'string') {
      entries[key] = value;
    }
  }
  return sortEntries(entries);
};

export const createPortableConfigPayload = (
  storage: Storage,
  source: Partial<ConfigSource> = {},
): DtvConfigPayload => {
  return {
    kind: DTV_CONFIG_KIND,
    version: DTV_CONFIG_VERSION,
    exportedAt: new Date().toISOString(),
    source: {
      client: source.client ?? 'desktop',
      appVersion: source.appVersion ?? null,
    },
    entries: collectPortableConfigEntries(storage),
  };
};

export const parsePortableConfigPayload = (raw: string): DtvConfigPayload => {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error('Config file is not valid JSON.');
  }

  if (!isRecord(parsed)) {
    throw new Error('Config file format is invalid.');
  }

  if (parsed.kind !== DTV_CONFIG_KIND) {
    throw new Error('This is not a DTV config export.');
  }

  if (parsed.version !== DTV_CONFIG_VERSION) {
    throw new Error(`Unsupported config file version: ${String(parsed.version)}.`);
  }

  if (!isRecord(parsed.source) || typeof parsed.source.client !== 'string' || !parsed.source.client.trim()) {
    throw new Error('Config file source is invalid.');
  }

  if (typeof parsed.exportedAt !== 'string' || Number.isNaN(Date.parse(parsed.exportedAt))) {
    throw new Error('Config file exportedAt is invalid.');
  }

  if (!isRecord(parsed.entries)) {
    throw new Error('Config file entries are missing.');
  }

  const sanitizedEntries: PortableConfigEntries = {};
  Object.entries(parsed.entries).forEach(([key, value]) => {
    if (!isExportableStorageKey(key)) {
      return;
    }
    if (typeof value !== 'string') throw new Error(`配置项 ${key} 类型无效，未导入。`);
    sanitizedEntries[key] = validateEntry(key, value);
  });

  return {
    kind: DTV_CONFIG_KIND,
    version: DTV_CONFIG_VERSION,
    exportedAt: parsed.exportedAt,
    source: {
      client: parsed.source.client,
      appVersion: typeof parsed.source.appVersion === 'string' ? parsed.source.appVersion : null,
    },
    entries: sortEntries(sanitizedEntries),
  };
};

const pendingRecovery = new WeakMap<Storage, PortableConfigEntries>();

const writeEntries = (storage: Storage, entries: PortableConfigEntries): void => {
  const current = collectPortableConfigEntries(storage);
  Object.keys(current).forEach((key) => { if (!(key in entries)) storage.removeItem(key); });
  Object.entries(entries).forEach(([key, value]) => {
    if (storage.getItem(key) !== value) storage.setItem(key, value);
  });
};

export const replacePortableConfigEntries = (
  storage: Storage,
  entries: PortableConfigEntries,
): void => {
  const validated: PortableConfigEntries = {};
  Object.entries(entries).forEach(([key, value]) => {
    if (isExportableStorageKey(key)) validated[key] = validateEntry(key, value);
  });
  const recovery = pendingRecovery.get(storage);
  if (recovery) {
    try { writeEntries(storage, recovery); pendingRecovery.delete(storage); }
    catch { throw new Error('原配置恢复失败，请释放存储空间后在当前页面重试，勿刷新页面。'); }
  }
  const snapshot = collectPortableConfigEntries(storage);
  try { writeEntries(storage, validated); }
  catch {
    pendingRecovery.set(storage, snapshot);
    try { writeEntries(storage, snapshot); pendingRecovery.delete(storage); }
    catch { throw new Error('导入失败且原配置恢复未完成，请释放存储空间后在当前页面重试，勿刷新页面。'); }
    throw new Error('配置写入失败，原配置已恢复。');
  }
};

const pad = (value: number) => String(value).padStart(2, '0');

export const buildPortableConfigFileName = (now = new Date()): string => {
  const datePart = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
  const timePart = `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `dtv-config-${datePart}-${timePart}.json`;
};
