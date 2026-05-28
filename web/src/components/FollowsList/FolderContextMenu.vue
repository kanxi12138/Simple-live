<template>
  <teleport to="body">
    <Transition name="context-menu">
      <div 
        v-if="show" 
        class="context-menu"
        :style="{ top: `${position.y}px`, left: `${position.x}px` }"
        @click.stop
      >
        <button class="context-menu-item" @click="handleRename">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
          </svg>
          <span>重命名</span>
        </button>
        <button class="context-menu-item danger" @click="handleDelete">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 6 5 6 21 6"></polyline>
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
          </svg>
          <span>删除</span>
        </button>
      </div>
    </Transition>
    <div v-if="show" class="context-menu-backdrop" @click="close"></div>
  </teleport>
  
  <!-- 重命名对话框 -->
  <teleport to="body">
    <Transition name="dialog">
      <div v-if="showRenameDialog" class="dialog-backdrop" @click="cancelRename">
        <div class="dialog" @click.stop>
          <h3 class="dialog-title">重命名文件夹</h3>
          <input
            ref="renameInputRef"
            v-model="renameValue"
            class="dialog-input"
            type="text"
            placeholder="输入文件夹名称"
            maxlength="50"
            @keyup.enter="confirmRename"
            @keyup.esc="cancelRename"
          />
          <div class="dialog-actions">
            <button class="dialog-btn cancel" @click="cancelRename">取消</button>
            <button class="dialog-btn confirm" @click="confirmRename">确定</button>
          </div>
        </div>
      </div>
    </Transition>
  </teleport>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue';

const props = defineProps<{
  show: boolean;
  position: { x: number; y: number };
  folderName: string;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'rename', newName: string): void;
  (e: 'delete'): void;
}>();

const showRenameDialog = ref(false);
const renameValue = ref('');
const renameInputRef = ref<HTMLInputElement | null>(null);

const close = () => {
  emit('close');
};

const handleRename = () => {
  close();
  showRenameDialog.value = true;
  renameValue.value = props.folderName;
  nextTick(() => {
    renameInputRef.value?.focus();
    renameInputRef.value?.select();
  });
};

const handleDelete = () => {
  close();
  emit('delete');
};

const confirmRename = () => {
  const trimmed = renameValue.value.trim();
  // 验证输入：不能为空，不能只包含空格
  if (!trimmed) {
    // 如果输入为空，提示用户并保持对话框打开
    renameInputRef.value?.focus();
    return;
  }
  // 如果名称与原来相同，也允许（用户可能只是想确认）
  emit('rename', trimmed);
  showRenameDialog.value = false;
  renameValue.value = '';
};

const cancelRename = () => {
  showRenameDialog.value = false;
  renameValue.value = '';
};
</script>

<style scoped>
.context-menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 999;
}

.context-menu {
  position: fixed;
  z-index: 1000;
  min-width: 160px;
  background: rgba(30, 30, 36, 0.98);
  border: 1px solid rgba(96, 98, 112, 0.5);
  border-radius: 10px;
  padding: 4px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(12px);
}

.context-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 12px;
  border: none;
  background: transparent;
  color: rgba(226, 232, 240, 0.9);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  border-radius: 6px;
  transition: background 0.15s ease;
}

.context-menu-item:hover {
  background: rgba(148, 163, 184, 0.2);
}

.context-menu-item.danger {
  color: rgba(248, 113, 113, 0.9);
}

.context-menu-item.danger:hover {
  background: rgba(248, 113, 113, 0.15);
}

.context-menu-item svg {
  flex-shrink: 0;
}

.context-menu-enter-active,
.context-menu-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.context-menu-enter-from,
.context-menu-leave-to {
  opacity: 0;
  transform: scale(0.95) translateY(-4px);
}

.dialog-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(7, 10, 18, 0.35);
  backdrop-filter: blur(10px);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.dialog {
  background: var(--glass-bg);
  border: 1px solid var(--glass-border);
  border-radius: 16px;
  padding: 22px 22px 18px;
  min-width: 320px;
  box-shadow: var(--glass-shadow);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
}

.dialog-title {
  margin: 0 0 14px 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--primary-text);
  letter-spacing: 0.01em;
}

.dialog-input {
  width: 100%;
  padding: 11px 14px;
  border: 1px solid var(--glass-border);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.06);
  color: var(--primary-text);
  font-size: 14px;
  margin-bottom: 16px;
  transition: border-color 0.2s ease;
}

.dialog-input:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--accent-color) 65%, transparent);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.dialog-btn {
  padding: 8px 18px;
  border: 1px solid var(--glass-border);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.06);
  color: var(--primary-text);
  font-size: 13px;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease;
}

.dialog-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  border-color: color-mix(in srgb, var(--glass-border) 80%, transparent);
}

.dialog-btn.confirm {
  background: color-mix(in srgb, var(--accent-color) 18%, transparent);
  border-color: color-mix(in srgb, var(--accent-color) 45%, transparent);
  color: var(--primary-text);
}

.dialog-btn.confirm:hover {
  background: color-mix(in srgb, var(--accent-color) 30%, transparent);
  border-color: color-mix(in srgb, var(--accent-color) 65%, transparent);
}

.dialog-enter-active,
.dialog-leave-active {
  transition: opacity 0.2s ease;
}

.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}

:root[data-theme="light"] .context-menu {
  background: rgba(255, 255, 255, 0.98);
  border-color: rgba(209, 217, 234, 0.6);
}

:root[data-theme="light"] .context-menu-item {
  color: #334155;
}

:root[data-theme="light"] .context-menu-item:hover {
  background: rgba(148, 163, 184, 0.15);
}

:root[data-theme="light"] .context-menu-item.danger {
  color: #dc2626;
}

:root[data-theme="light"] .context-menu-item.danger:hover {
  background: rgba(248, 113, 113, 0.12);
}

:root[data-theme="light"] .dialog {
  background: var(--glass-bg);
  border-color: var(--glass-border);
}

:root[data-theme="light"] .dialog-title {
  color: #1f2937;
}

:root[data-theme="light"] .dialog-input {
  background: rgba(255, 255, 255, 0.7);
  border-color: var(--glass-border);
  color: #1f2937;
}

:root[data-theme="light"] .dialog-input:focus {
  border-color: color-mix(in srgb, var(--accent-color) 55%, transparent);
}

:root[data-theme="light"] .dialog-btn {
  background: rgba(255, 255, 255, 0.7);
  border-color: var(--glass-border);
  color: #475569;
}

:root[data-theme="light"] .dialog-btn:hover {
  background: rgba(255, 255, 255, 0.85);
}

:root[data-theme="light"] .dialog-btn.confirm {
  background: color-mix(in srgb, var(--accent-color) 12%, transparent);
  border-color: color-mix(in srgb, var(--accent-color) 40%, transparent);
  color: #1f2937;
}

:root[data-theme="light"] .dialog-btn.confirm:hover {
  background: color-mix(in srgb, var(--accent-color) 20%, transparent);
  border-color: color-mix(in srgb, var(--accent-color) 55%, transparent);
}
</style>
