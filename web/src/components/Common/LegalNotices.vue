<template>
  <details class="legal-notices">
    <summary>开源许可证与隐私说明</summary>
    <p>Simple-live · GPL-3.0 · 个人非商业开源项目</p>
    <details v-for="document in documents" :key="document.title">
      <summary>{{ document.title }}</summary>
      <pre>{{ document.text }}</pre>
    </details>
  </details>
</template>

<script setup lang="ts">
import license from '../../../../LICENSE?raw';
import notices from '../../../../THIRD_PARTY_NOTICES.md?raw';
import privacy from '../../../../PRIVACY.md?raw';
import compliance from '../../../../COMPLIANCE.md?raw';
import mit from '../../../../docs/licenses/DTV-MIT.txt?raw';
import dependencies from '../../../../docs/licenses/DEPENDENCIES.txt?raw';

const documents = [
  { title: '隐私说明', text: privacy },
  { title: '功能边界', text: compliance },
  { title: '第三方版权说明', text: `${notices}\n\n${mit}` },
  { title: 'GNU GPL version 3', text: license },
  { title: '依赖许可证', text: dependencies },
];
</script>

<style scoped>
.legal-notices { margin: 16px 0; }
summary { cursor: pointer; padding: 8px 0; }
pre { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 40vh; overflow: auto; font-size: 12px; }
</style>
