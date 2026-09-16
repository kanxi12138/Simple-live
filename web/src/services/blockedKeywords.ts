import { readonly, shallowRef } from 'vue';

const STORAGE_KEY = 'danmu_block_keywords';
const keywords = shallowRef<string[]>([]);
let initialized = false;
const normalize = (values: string[]): string[] => [...new Set(values.map((value) => value.trim().toLowerCase()).filter(Boolean))];
const parse = (raw: string | null): string[] => {
  if (!raw) return [];
  const value: unknown = JSON.parse(raw);
  if (!Array.isArray(value) || !value.every((item) => typeof item === 'string')) throw new Error('屏蔽词格式无效');
  return normalize(value);
};

export const getBlockedKeywords = () => {
  if (!initialized && typeof window !== 'undefined') {
    initialized = true;
    try { keywords.value = parse(window.localStorage.getItem(STORAGE_KEY)); }
    catch { console.warn('弹幕屏蔽词初始化失败：存储读取或格式错误'); }
    window.addEventListener('storage', (event) => {
      if ((event.key === STORAGE_KEY || event.key === null) && event.storageArea === window.localStorage) {
        try { keywords.value = parse(event.newValue); }
        catch { console.warn('弹幕屏蔽词同步失败：格式错误'); }
      }
    });
  }
  return readonly(keywords);
};

/** Publishes the new cache only after persistence succeeds. */
export const saveBlockedKeywords = (values: string[]): void => {
  const next = normalize(values);
  try { window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next)); }
  catch { throw new Error('屏蔽词保存失败，原设置已保留。'); }
  keywords.value = next;
};
