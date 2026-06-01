<template>
  <transition name="sheet-fade">
    <div v-if="visible" class="sheet-root" @click.self="emit('close')">
      <section class="sheet">
        <div class="sheet-header">
          <div>
            <strong>设置与迁移</strong>
            <p>保留配置导入导出，移动端改成文件流方案。</p>
          </div>
          <button
            type="button"
            class="close-btn"
            @click="emit('close')"
          >
            关闭
          </button>
        </div>

        <div class="actions">
          <button
            type="button"
            class="action-card"
            :disabled="busy"
            @click="emit('export-config')"
          >
            <Download :size="18" />
            <div>
              <strong>{{ exporting ? '导出中...' : '导出配置' }}</strong>
              <span>关注、分组、主题、播放器偏好、B 站登录态。</span>
            </div>
          </button>

          <button
            type="button"
            class="action-card"
            :disabled="busy"
            @click="emit('import-config')"
          >
            <Upload :size="18" />
            <div>
              <strong>{{ importing ? '导入中...' : '导入配置' }}</strong>
              <span>导入后会覆盖当前本地配置并刷新界面。</span>
            </div>
          </button>

          <button
            type="button"
            class="action-card"
            @click="emit('open-github')"
          >
            <Github :size="18" />
            <div>
              <strong>项目主页</strong>
              <span>{{ appVersion ? `当前版本 ${appVersion}` : '打开 GitHub' }}</span>
            </div>
          </button>
        </div>

        <div class="section-card">
          <strong>检查更新</strong>
          <div class="action-row">
            <button
              type="button"
              class="inline-btn"
              :disabled="checkingUpdate || downloadingUpdate"
              @click="emit('check-update')"
            >
              {{ checkingUpdate ? '检查中...' : downloadingUpdate ? '下载中...' : '检查更新' }}
            </button>
          </div>
          <p v-if="updateMessage" class="update-message">{{ updateMessage }}</p>
        </div>

        <transition name="dialog">
          <div
            v-if="showUpdateConfirm"
            class="dialog-backdrop"
            @click="emit('cancel-update')"
          >
            <div class="dialog" @click.stop>
              <h3 class="dialog-title">发现新版本</h3>
              <p class="dialog-text">{{ updateConfirmMessage }}</p>
              <div class="dialog-actions">
                <button class="dialog-btn cancel" @click="emit('cancel-update')">取消</button>
                <button class="dialog-btn confirm" @click="emit('confirm-update')">立即更新</button>
              </div>
            </div>
          </div>
        </transition>

        <transition name="dialog">
          <div
            v-if="showImportDialog"
            class="dialog-backdrop"
            @click="emit('cancel-import')"
          >
            <div class="dialog import-dialog" @click.stop>
              <h3 class="dialog-title">粘贴配置内容</h3>
              <textarea
                ref="importTextareaRef"
                class="import-textarea"
                placeholder="请粘贴之前导出的配置内容..."
                rows="8"
              ></textarea>
              <div class="dialog-actions">
                <button class="dialog-btn cancel" @click="emit('cancel-import')">取消</button>
                <button class="dialog-btn confirm" @click="handleConfirmImport">确认导入</button>
              </div>
            </div>
          </div>
        </transition>

        <p
          v-if="status"
          class="status"
          :class="`status--${status.tone}`"
        >
          {{ status.text }}
        </p>
      </section>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { Download, Github, Upload } from 'lucide-vue-next';

const props = defineProps<{
  visible: boolean;
  exporting: boolean;
  importing: boolean;
  appVersion: string;
  status: { tone: 'info' | 'success' | 'error'; text: string } | null;
  checkingUpdate: boolean;
  downloadingUpdate: boolean;
  updateMessage: string;
  showUpdateConfirm: boolean;
  updateConfirmMessage: string;
  showImportDialog: boolean;
}>();

const emit = defineEmits<{
  (event: 'close'): void;
  (event: 'export-config'): void;
  (event: 'import-config'): void;
  (event: 'open-github'): void;
  (event: 'check-update'): void;
  (event: 'confirm-update'): void;
  (event: 'cancel-update'): void;
  (event: 'confirm-import', text: string): void;
  (event: 'cancel-import'): void;
}>();

const importTextareaRef = ref<HTMLTextAreaElement | null>(null);

const busy = computed(() => props.exporting || props.importing);

const handleConfirmImport = () => {
  const rawText = importTextareaRef.value?.value ?? '';
  emit('confirm-import', rawText);
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
  border: 1px solid var(--mobile-border);
  border-bottom: none;
  box-shadow: var(--mobile-sheet-shadow);
}

.sheet-header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.sheet-header strong {
  font-size: 18px;
  line-height: 1.2;
}

.sheet-header p {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--mobile-text-secondary);
}

.close-btn {
  min-width: 56px;
  min-height: 40px;
  border: 1px solid var(--mobile-border);
  border-radius: 12px;
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
  border-radius: 14px;
  background: var(--mobile-surface-strong);
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

.section-card {
  margin-top: 14px;
  padding: 14px;
  border: 1px solid var(--mobile-border);
  border-radius: 14px;
  background: var(--mobile-surface-strong);
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

.dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 86;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: rgba(2, 6, 23, 0.56);
}

.dialog {
  width: min(320px, calc(100vw - 40px));
  border: 1px solid var(--mobile-border);
  border-radius: 16px;
  padding: 18px;
  background: var(--mobile-surface-strong, var(--mobile-surface));
  color: var(--mobile-text-primary);
  box-shadow: var(--shadow-lg);
}

.dialog-title {
  margin: 0;
  font-size: 16px;
}

.dialog-text {
  margin: 10px 0 0;
  color: var(--mobile-text-secondary);
  font-size: 13px;
  line-height: 1.5;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 16px;
}

.dialog-btn {
  min-height: 38px;
  padding: 0 14px;
  border: 1px solid var(--mobile-border);
  border-radius: 12px;
  background: var(--mobile-surface-muted);
  color: var(--mobile-text-primary);
  font-weight: 700;
}

.dialog-btn.confirm {
  background: var(--mobile-icon-btn-bg);
}

.status {
  margin: 14px 0 0;
  font-size: 12px;
}

.status--info {
  color: var(--accent);
}

.status--success {
  color: var(--success-color);
}

.status--error {
  color: var(--error-color);
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

.dialog-enter-active,
.dialog-leave-active {
  transition: opacity 180ms ease;
}

.dialog-enter-active .dialog,
.dialog-leave-active .dialog {
  transition: transform 180ms ease;
}

.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}

.dialog-enter-from .dialog,
.dialog-leave-to .dialog {
  transform: translateY(12px);
}

.import-dialog {
  width: min(360px, calc(100vw - 40px));
}

.import-textarea {
  width: 100%;
  margin-top: 12px;
  padding: 10px;
  border: 1px solid var(--mobile-border);
  border-radius: 10px;
  background: var(--mobile-surface-muted);
  color: var(--mobile-text-primary);
  font-size: 12px;
  font-family: monospace;
  resize: vertical;
  outline: none;
  box-sizing: border-box;
}

.import-textarea:focus {
  border-color: var(--accent);
}
</style>
