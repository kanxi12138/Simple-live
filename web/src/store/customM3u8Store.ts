import { defineStore } from 'pinia';

export interface CustomM3u8Entry {
  id: string;
  title: string;
  url: string;
  headers?: string;
  cover?: string;
  group?: string;
  createdAt: number;
  updatedAt: number;
}

const STORAGE_KEY = 'dtv_custom_m3u8_sources_v1';

const normalizeText = (value?: string | null) => (value ?? '').trim();

const parseStoredEntries = (raw: string | null): CustomM3u8Entry[] => {
  if (!raw) {
    return [];
  }
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .filter((item) => item && typeof item === 'object')
      .map((item) => ({
        id: normalizeText(item.id) || `m3u8_${Date.now()}`,
        title: normalizeText(item.title) || '未命名源',
        url: normalizeText(item.url),
        headers: normalizeText(item.headers) || undefined,
        cover: normalizeText(item.cover) || undefined,
        group: normalizeText(item.group) || undefined,
        createdAt: Number(item.createdAt) || Date.now(),
        updatedAt: Number(item.updatedAt) || Date.now(),
      }))
      .filter((item) => !!item.url);
  } catch (error) {
    console.warn('[CustomM3u8Store] Failed to parse saved sources:', error);
    return [];
  }
};

export const useCustomM3u8Store = defineStore('customM3u8', {
  state: () => ({
    entries: [] as CustomM3u8Entry[],
    loaded: false,
  }),
  getters: {
    getById: (state) => (id: string) => state.entries.find((entry) => entry.id === id) ?? null,
  },
  actions: {
    ensureLoaded() {
      if (this.loaded) return;
      if (typeof window === 'undefined') {
        this.loaded = true;
        return;
      }
      this.entries = parseStoredEntries(window.localStorage.getItem(STORAGE_KEY));
      this.loaded = true;
    },
    persist() {
      if (typeof window === 'undefined') return;
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(this.entries));
    },
    saveEntry(payload: {
      id?: string;
      title: string;
      url: string;
      headers?: string;
      cover?: string;
      group?: string;
    }) {
      const now = Date.now();
      const id = normalizeText(payload.id) || `m3u8_${now}_${Math.random().toString(36).slice(2, 8)}`;
      const nextEntry: CustomM3u8Entry = {
        id,
        title: normalizeText(payload.title) || '未命名源',
        url: normalizeText(payload.url),
        headers: normalizeText(payload.headers) || undefined,
        cover: normalizeText(payload.cover) || undefined,
        group: normalizeText(payload.group) || undefined,
        createdAt: this.getById(id)?.createdAt ?? now,
        updatedAt: now,
      };
      this.entries = [
        nextEntry,
        ...this.entries.filter((entry) => entry.id !== id),
      ];
      this.persist();
      return nextEntry;
    },
    deleteEntry(id: string) {
      this.entries = this.entries.filter((entry) => entry.id !== id);
      this.persist();
    },
  },
});
