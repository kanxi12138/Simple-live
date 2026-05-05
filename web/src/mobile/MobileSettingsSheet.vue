<template>
  <transition name="sheet-fade">
    <div v-if="visible" class="sheet-root" @click.self="emit('close')">
      <section class="sheet">
        <div class="sheet-header">
          <div>
            <strong>设置与迁移</strong>
            <p>保留配置导入导出，移动端改成文件流方案。</p>
          </div>
          <button type="button" class="close-btn" @click="emit('close')">关闭</button>
        </div>

        <div class="actions">
          <button type="button" class="action-card" :disabled="busy" @click="emit('export-config')">
            <Download :size="18" />
            <div>
              <strong>{{ exporting ? '导出中...' : '导出配置' }}</strong>
              <span>关注、分组、主题、播放器偏好、B站登录态</span>
            </div>
          </button>

          <button type="button" class="action-card" :disabled="busy" @click="emit('import-config')">
            <Upload :size="18" />
            <div>
              <strong>{{ importing ? '导入中...' : '导入配置' }}</strong>
              <span>导入后会覆盖当前本地配置并刷新界面</span>
            </div>
          </button>

          <button type="button" class="action-card" @click="emit('open-github')">
            <Github :size="18" />
            <div>
              <strong>项目主页</strong>
              <span>{{ appVersion ? `当前版本 ${appVersion}` : '打开 GitHub' }}</span>
            </div>
          </button>
        </div>

        <div class="webdav-card">
          <strong>WebDAV 配置同步</strong>
          <div class="field-grid">
            <input :value="webdav.url" type="url" placeholder="服务器地址" @input="emitField('url', $event)" />
            <input :value="webdav.username" type="text" placeholder="用户名" @input="emitField('username', $event)" />
            <input :value="webdav.password" type="password" placeholder="密码" @input="emitField('password', $event)" />
            <input :value="webdav.fileName" type="text" placeholder="远端文件名" @input="emitField('fileName', $event)" />
          </div>
          <div class="action-row">
            <button type="button" class="inline-btn" :disabled="busy || syncingWebdav" @click="emit('webdav-upload')">上传配置</button>
            <button type="button" class="inline-btn" :disabled="busy || syncingWebdav" @click="emit('webdav-download')">下载并覆盖</button>
          </div>
        </div>

        <div class="webdav-card">
          <strong>检查更新</strong>
          <div class="action-row">
            <button type="button" class="inline-btn" :disabled="checkingUpdate" @click="emit('check-update')">
              {{ checkingUpdate ? '检查中...' : '检查更新' }}
            </button>
          </div>
          <p v-if="updateMessage" class="update-message">{{ updateMessage }}</p>
        </div>

        <p v-if="status" class="status" :class="`status--${status.tone}`">{{ status.text }}</p>
      </section>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { Download, Github, Upload } from 'lucide-vue-next';

const props = defineProps<{
  visible: boolean;
  exporting: boolean;
  importing: boolean;
  appVersion: string;
  status: { tone: 'info' | 'success' | 'error'; text: string } | null;
  webdav: { url: string; username: string; password: string; fileName: string };
  syncingWebdav: boolean;
  checkingUpdate: boolean;
  updateMessage: string;
}>();

const emit = defineEmits<{
  (event: 'close'): void;
  (event: 'export-config'): void;
  (event: 'import-config'): void;
  (event: 'open-github'): void;
  (event: 'webdav-change', payload: { key: 'url' | 'username' | 'password' | 'fileName'; value: string }): void;
  (event: 'webdav-upload'): void;
  (event: 'webdav-download'): void;
  (event: 'check-update'): void;
}>();

const busy = computed(() => props.exporting || props.importing);
const emitField = (key: 'url' | 'username' | 'password' | 'fileName', event: Event) => {
  const target = event.target as HTMLInputElement;
  emit('webdav-change', { key, value: target.value });
};
</script>

<style scoped>
.sheet-root {
  position: fixed;
  inset: 0;
  z-index: 72;
  background: var(--mobile-sheet-backdrop);
  display: flex;
  align-items: flex-end;
}

.sheet {
  width: 100%;
  max-height: min(74vh, 720px);
  border-radius: 24px 24px 0 0;
  padding: 18px 16px calc(16px + env(safe-area-inset-bottom));
  background: var(--mobile-surface);
  color: var(--mobile-text-primary);
}

.sheet-header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.sheet-header strong {
  font-size: 16px;
}

.sheet-header p {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--mobile-text-secondary);
}

.close-btn {
  min-width: 56px;
  min-height: 36px;
  border: none;
  border-radius: 10px;
  background: var(--mobile-icon-btn-bg);
  color: var(--mobile-text-primary);
  font-weight: 700;
}

.actions {
  display: grid;
  gap: 12px;
  margin-top: 16px;
}

.action-card {
  width: 100%;
  padding: 14px;
  border: 1px solid var(--mobile-border);
  border-radius: 16px;
  background: var(--mobile-surface-muted);
  color: var(--mobile-text-primary);
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr);
  gap: 12px;
  align-items: start;
  text-align: left;
}

.action-card strong {
  display: block;
  font-size: 14px;
}

.action-card span {
  display: block;
  margin-top: 4px;
  color: var(--mobile-text-secondary);
  font-size: 12px;
  line-height: 1.45;
}

.action-card:disabled {
  opacity: 0.68;
}

.webdav-card {
  margin-top: 14px;
  padding: 14px;
  border: 1px solid var(--mobile-border);
  border-radius: 16px;
  background: var(--mobile-surface-muted);
}

.field-grid {
  display: grid;
  gap: 10px;
  margin-top: 12px;
}

.field-grid input {
  width: 100%;
  min-height: 42px;
  border: 1px solid var(--mobile-border);
  border-radius: 12px;
  background: transparent;
  color: var(--mobile-text-primary);
  padding: 0 12px;
}

.action-row {
  display: flex;
  gap: 10px;
  margin-top: 12px;
}

.inline-btn {
  min-height: 38px;
  padding: 0 14px;
  border: none;
  border-radius: 12px;
  background: var(--mobile-icon-btn-bg);
  color: var(--mobile-text-primary);
  font-weight: 700;
}

.update-message {
  margin: 10px 0 0;
  font-size: 12px;
  color: var(--mobile-text-secondary);
}

.status {
  margin: 14px 0 0;
  font-size: 12px;
}

.status--info {
  color: #93c5fd;
}

.status--success {
  color: #86efac;
}

.status--error {
  color: #fda4af;
}

.sheet-fade-enter-active,
.sheet-fade-leave-active {
  transition: opacity 180ms ease;
}

.sheet-fade-enter-active .sheet,
.sheet-fade-leave-active .sheet {
  transition: transform 180ms ease;
}

.sheet-fade-enter-from,
.sheet-fade-leave-to {
  opacity: 0;
}

.sheet-fade-enter-from .sheet,
.sheet-fade-leave-to .sheet {
  transform: translateY(18px);
}
</style>
