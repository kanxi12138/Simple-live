import { defineStore } from 'pinia';

export type CustomPlatform = 'douyu' | 'douyin' | 'huya' | 'bilibili';
export type CustomCategoryLevel = 'cate2' | 'cate3';

export interface CustomCategoryEntry {
  key: string;
  platform: CustomPlatform;
  categoryLevel: CustomCategoryLevel;
  cate1Name?: string;
  cate1Href?: string;
  cate2Name: string;
  cate2Href?: string;
  cate2Id?: string;
  cate3Id?: string;
  cate3Name?: string;
  douyuShortName?: string;
}

type LegacyCustomCategoryEntry = {
  key?: string;
  platform?: CustomPlatform;
  cate2Name?: string;
  cate1Name?: string;
  cate1Href?: string;
  cate2Href?: string;
  douyuId?: string;
};

type LegacyCustomCategoryEntryV2 = LegacyCustomCategoryEntry & {
  categoryLevel?: CustomCategoryLevel;
  cate2Id?: string;
  cate3Id?: string;
  cate3Name?: string;
  douyuShortName?: string;
};

const STORAGE_KEY = 'dtv_custom_categories_v2';
const LEGACY_STORAGE_KEY = 'dtv_custom_categories_v1';

const normalizeText = (value?: string | null) => (value ?? '').trim();

const buildCate2Key = (platform: CustomPlatform, id: string) => `${platform}:cate2:${id}`;
const buildCate3Key = (platform: CustomPlatform, parentId: string, cate3Id: string) =>
  `${platform}:cate3:${parentId}:${cate3Id}`;

const normalizeEntry = (entry: Partial<CustomCategoryEntry>): CustomCategoryEntry | null => {
  const platform = entry.platform;
  const categoryLevel = entry.categoryLevel;
  const cate2Name = normalizeText(entry.cate2Name);

  if (!platform || !categoryLevel || !cate2Name) {
    return null;
  }

  if (platform === 'douyu') {
    const shortName = normalizeText(entry.douyuShortName);
    if (!shortName) {
      return null;
    }
    if (categoryLevel === 'cate2') {
      return {
        key: buildCate2Key(platform, shortName),
        platform,
        categoryLevel,
        cate2Name,
        cate2Id: normalizeText(entry.cate2Id) || undefined,
        douyuShortName: shortName,
      };
    }

    const cate3Id = normalizeText(entry.cate3Id);
    const cate3Name = normalizeText(entry.cate3Name);
    if (!cate3Id || !cate3Name) {
      return null;
    }
    return {
      key: buildCate3Key(platform, shortName, cate3Id),
      platform,
      categoryLevel,
      cate2Name,
      cate2Id: normalizeText(entry.cate2Id) || undefined,
      cate3Id,
      cate3Name,
      douyuShortName: shortName,
    };
  }

  const cate2Href = normalizeText(entry.cate2Href);
  if (!cate2Href) {
    return null;
  }

  return {
    key: buildCate2Key(platform, cate2Href),
    platform,
    categoryLevel: 'cate2',
    cate1Name: normalizeText(entry.cate1Name) || undefined,
    cate1Href: normalizeText(entry.cate1Href) || undefined,
    cate2Name,
    cate2Href,
  };
};

const migrateLegacyEntry = (entry: LegacyCustomCategoryEntry): CustomCategoryEntry | null => {
  const platform = entry.platform;
  if (!platform) {
    return null;
  }

  if (platform === 'douyu') {
    return normalizeEntry({
      platform,
      categoryLevel: 'cate2',
      cate2Name: entry.cate2Name,
      douyuShortName: entry.douyuId,
    });
  }

  return normalizeEntry({
    platform,
    categoryLevel: 'cate2',
    cate1Name: entry.cate1Name,
    cate1Href: entry.cate1Href,
    cate2Name: entry.cate2Name,
    cate2Href: entry.cate2Href,
  });
};

const dedupeEntries = (entries: CustomCategoryEntry[]) => {
  const seen = new Set<string>();
  return entries.filter((entry) => {
    if (seen.has(entry.key)) {
      return false;
    }
    seen.add(entry.key);
    return true;
  });
};

const parseStoredEntries = (raw: string | null) => {
  if (!raw) {
    return [] as CustomCategoryEntry[];
  }

  const parsed = JSON.parse(raw);
  if (!Array.isArray(parsed)) {
    return [] as CustomCategoryEntry[];
  }

  const migrated = parsed
    .map((item) => {
      if (!item || typeof item !== 'object') {
        return null;
      }
      if ('categoryLevel' in item) {
        return normalizeEntry(item as Partial<LegacyCustomCategoryEntryV2>);
      }
      return migrateLegacyEntry(item as LegacyCustomCategoryEntry);
    })
    .filter((item): item is CustomCategoryEntry => !!item);

  return dedupeEntries(migrated);
};

export const useCustomCategoryStore = defineStore('customCategories', {
  state: () => ({
    entries: [] as CustomCategoryEntry[],
    loaded: false,
  }),
  getters: {
    hasEntries: (state) => state.entries.length > 0,
    groupedByPlatform: (state) => {
      const map: Record<CustomPlatform, CustomCategoryEntry[]> = {
        douyu: [],
        douyin: [],
        huya: [],
        bilibili: [],
      };
      state.entries.forEach((entry) => {
        map[entry.platform].push(entry);
      });
      return map;
    },
  },
  actions: {
    ensureLoaded() {
      if (this.loaded) return;
      if (typeof window === 'undefined') {
        this.loaded = true;
        return;
      }
      try {
        const current = parseStoredEntries(window.localStorage.getItem(STORAGE_KEY));
        const legacy = current.length ? [] : parseStoredEntries(window.localStorage.getItem(LEGACY_STORAGE_KEY));
        this.entries = dedupeEntries(current.length ? current : legacy);
        if (window.localStorage.getItem(LEGACY_STORAGE_KEY)) {
          window.localStorage.removeItem(LEGACY_STORAGE_KEY);
        }
        this.persist();
      } catch (err) {
        console.warn('[CustomCategoryStore] Failed to load categories:', err);
      } finally {
        this.loaded = true;
      }
    },
    persist() {
      if (typeof window === 'undefined') return;
      try {
        window.localStorage.setItem(STORAGE_KEY, JSON.stringify(this.entries));
      } catch (err) {
        console.warn('[CustomCategoryStore] Failed to save categories:', err);
      }
    },
    isKeySubscribed(key: string) {
      return this.entries.some((entry) => entry.key === key);
    },
    isCate2Subscribed(platform: CustomPlatform, id: string) {
      const normalizedId = normalizeText(id);
      if (!normalizedId) return false;
      return this.isKeySubscribed(buildCate2Key(platform, normalizedId));
    },
    isCate3Subscribed(platform: CustomPlatform, parentId: string, cate3Id: string) {
      const normalizedParentId = normalizeText(parentId);
      const normalizedCate3Id = normalizeText(cate3Id);
      if (!normalizedParentId || !normalizedCate3Id) return false;
      return this.isKeySubscribed(buildCate3Key(platform, normalizedParentId, normalizedCate3Id));
    },
    toggleDouyuCate2(shortName: string, cate2Name: string, cate2Id?: string | number) {
      const normalizedShortName = normalizeText(shortName);
      if (!normalizedShortName) return;
      const key = buildCate2Key('douyu', normalizedShortName);
      if (this.isKeySubscribed(key)) {
        this.removeByKey(key);
        return;
      }
      const entry = normalizeEntry({
        platform: 'douyu',
        categoryLevel: 'cate2',
        cate2Name,
        cate2Id: cate2Id == null ? undefined : String(cate2Id),
        douyuShortName: normalizedShortName,
      });
      if (!entry) return;
      this.entries = [...this.entries, entry];
      this.persist();
    },
    toggleDouyuCate3(shortName: string, cate2Id: string | number, cate2Name: string, cate3Id: string, cate3Name: string) {
      const normalizedShortName = normalizeText(shortName);
      const normalizedCate3Id = normalizeText(cate3Id);
      if (!normalizedShortName || !normalizedCate3Id) return;
      const key = buildCate3Key('douyu', normalizedShortName, normalizedCate3Id);
      if (this.isKeySubscribed(key)) {
        this.removeByKey(key);
        return;
      }
      const entry = normalizeEntry({
        platform: 'douyu',
        categoryLevel: 'cate3',
        cate2Name,
        cate2Id: String(cate2Id),
        cate3Id: normalizedCate3Id,
        cate3Name,
        douyuShortName: normalizedShortName,
      });
      if (!entry) return;
      this.entries = [...this.entries, entry];
      this.persist();
    },
    toggleCommonCate2(
      platform: CustomPlatform,
      cate2Href: string,
      cate2Name: string,
      cate1Name?: string,
      cate1Href?: string,
    ) {
      const normalizedHref = normalizeText(cate2Href);
      if (!normalizedHref) return;
      const key = buildCate2Key(platform, normalizedHref);
      if (this.isKeySubscribed(key)) {
        this.removeByKey(key);
        return;
      }
      const entry = normalizeEntry({
        platform,
        categoryLevel: 'cate2',
        cate1Name,
        cate1Href,
        cate2Name,
        cate2Href: normalizedHref,
      });
      if (!entry) return;
      this.entries = [...this.entries, entry];
      this.persist();
    },
    addOrReplaceEntry(entry: CustomCategoryEntry) {
      this.entries = [...this.entries.filter((item) => item.key !== entry.key), entry];
      this.persist();
    },
    removeByKey(key: string) {
      this.entries = this.entries.filter((entry) => entry.key !== key);
      this.persist();
    },
  },
});
