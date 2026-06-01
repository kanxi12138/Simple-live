<template>
  <div class="custom-m3u8-home">
    <form class="editor-card" @submit.prevent="handleSubmit">
      <div class="editor-grid">
        <input v-model.trim="draft.title" type="text" placeholder="标题" />
        <input v-model.trim="draft.group" type="text" placeholder="分组（可选）" />
        <input v-model.trim="draft.cover" type="url" placeholder="封面 URL（可选）" />
        <textarea v-model.trim="draft.url" placeholder="M3U8 / FLV 地址" rows="3"></textarea>
        <textarea v-model.trim="draft.headers" placeholder="Headers（可选，JSON）" rows="3"></textarea>
      </div>
      <div class="editor-actions">
        <button type="submit">{{ draft.id ? '保存修改' : '新增源' }}</button>
        <button v-if="draft.id" type="button" class="secondary" @click="resetDraft">取消编辑</button>
      </div>
    </form>

    <div v-if="!entries.length" class="empty-state">还没有自定义源。</div>

    <div v-else class="entry-list">
      <article v-for="entry in entries" :key="entry.id" class="entry-card">
        <div class="entry-main" @click="playEntry(entry.id)">
          <img v-if="entry.cover" :src="entry.cover" :alt="entry.title" class="entry-cover" />
          <div v-else class="entry-cover entry-cover--fallback">{{ entry.title.slice(0, 1) }}</div>
          <div class="entry-meta">
            <strong>{{ entry.title }}</strong>
            <span>{{ entry.group || '未分组' }}</span>
            <small>{{ entry.url }}</small>
          </div>
        </div>
        <div class="entry-actions">
          <button type="button" class="secondary" @click="editEntry(entry.id)">编辑</button>
          <button type="button" class="danger" @click="deleteEntry(entry.id)">删除</button>
        </div>
      </article>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive } from 'vue';
import { useRouter } from 'vue-router';
import { useCustomM3u8Store } from '../store/customM3u8Store';

defineOptions({ name: 'CustomM3u8HomeView' });

const router = useRouter();
const store = useCustomM3u8Store();
store.ensureLoaded();

const entries = computed(() => store.entries);
const draft = reactive({
  id: '',
  title: '',
  url: '',
  headers: '',
  cover: '',
  group: '',
});

const resetDraft = () => {
  draft.id = '';
  draft.title = '';
  draft.url = '';
  draft.headers = '';
  draft.cover = '';
  draft.group = '';
};

const handleSubmit = () => {
  if (!draft.url) {
    return;
  }
  store.saveEntry({
    id: draft.id || undefined,
    title: draft.title || '自定义源',
    url: draft.url,
    headers: draft.headers || undefined,
    cover: draft.cover || undefined,
    group: draft.group || undefined,
  });
  resetDraft();
};

const editEntry = (id: string) => {
  const entry = store.getById(id);
  if (!entry) return;
  draft.id = entry.id;
  draft.title = entry.title;
  draft.url = entry.url;
  draft.headers = entry.headers || '';
  draft.cover = entry.cover || '';
  draft.group = entry.group || '';
};

const deleteEntry = (id: string) => {
  store.deleteEntry(id);
  if (draft.id === id) {
    resetDraft();
  }
};

const playEntry = (id: string) => {
  router.push({
    name: 'customM3u8Player',
    params: { encodedId: encodeURIComponent(id) },
  });
};
</script>

<style scoped>
.custom-m3u8-home { display: flex; flex-direction: column; gap: 12px; padding: 10px 12px 16px; }
.editor-card, .entry-card { border: 1px solid var(--glass-border); border-radius: 16px; background: var(--hover-bg); padding: 12px; }
.editor-grid, .entry-list { display: grid; gap: 10px; }
input, textarea { width: 100%; border: 1px solid var(--mobile-border, var(--glass-border)); border-radius: 12px; background: transparent; color: var(--text-primary); padding: 10px 12px; }
.editor-actions, .entry-actions { display: flex; gap: 8px; margin-top: 10px; }
button { min-height: 38px; padding: 0 14px; border: none; border-radius: 12px; background: var(--accent); color: #fff; font-weight: 700; }
button.secondary { background: rgba(148, 163, 184, 0.18); color: var(--text-primary); }
button.danger { background: rgba(239, 68, 68, 0.84); }
.entry-card { display: grid; gap: 10px; }
.entry-main { display: grid; grid-template-columns: 60px minmax(0, 1fr); gap: 12px; align-items: center; cursor: pointer; }
.entry-cover { width: 60px; height: 60px; object-fit: cover; border-radius: 12px; background: rgba(148, 163, 184, 0.16); display: inline-flex; align-items: center; justify-content: center; }
.entry-cover--fallback { color: var(--text-primary); font-weight: 800; }
.entry-meta { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.entry-meta strong, .entry-meta span, .entry-meta small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.entry-meta span, .entry-meta small, .empty-state { color: var(--text-secondary); }
</style>
