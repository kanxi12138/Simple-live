import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../web/package.json', import.meta.url));
const ts = require('typescript');
const load = (path) => {
  const source = readFileSync(new URL(path, import.meta.url), 'utf8');
  const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } });
  const module = { exports: {} };
  new Function('require', 'module', 'exports', compiled.outputText)(require, module, module.exports);
  return module.exports;
};
class MemoryStorage {
  data = new Map();
  reads = 0;
  failWrites = 0;
  get length() { return this.data.size; }
  key(index) { return [...this.data.keys()][index] ?? null; }
  getItem(key) { this.reads += 1; return this.data.get(key) ?? null; }
  setItem(key, value) {
    if (this.failWrites > 0) { this.failWrites -= 1; throw new Error('storage full'); }
    this.data.set(key, value);
  }
  removeItem(key) { this.data.delete(key); }
}
const config = load('../web/src/services/configTransfer.ts');
const storage = new MemoryStorage();
storage.setItem('theme_preference', 'dark');
storage.setItem('bilibili_cookie', 'test-only-placeholder');
const payload = config.createPortableConfigPayload(storage);
assert.deepEqual(config.parsePortableConfigPayload(JSON.stringify(payload)).entries, { theme_preference: 'dark' });
for (const [key, value] of [['followedStreamers', '{}'], ['followFolders', '[null]'], ['followListOrder', '[{}]'], ['dtv_player_volume_v1', '2'], ['dtv_danmu_preferences_v1', '{"enabled":true,"settings":{"mode":"invalid"}}']]) {
  assert.throws(() => config.parsePortableConfigPayload(JSON.stringify({ ...payload, entries: { [key]: value } })), new RegExp(key));
}
assert.deepEqual(config.parsePortableConfigPayload(JSON.stringify({ ...payload, entries: { unknown: 'anything' } })).entries, {});
storage.failWrites = 1;
assert.throws(() => config.replacePortableConfigEntries(storage, { followedStreamers: '[]' }), /已恢复/);
assert.equal(storage.getItem('theme_preference'), 'dark');
assert.equal(storage.getItem('bilibili_cookie'), 'test-only-placeholder');
storage.failWrites = 2;
assert.throws(() => config.replacePortableConfigEntries(storage, { followedStreamers: '[]' }), /恢复未完成/);
config.replacePortableConfigEntries(storage, { theme_preference: 'light' });
assert.equal(storage.getItem('theme_preference'), 'light');
assert.equal(storage.getItem('bilibili_cookie'), 'test-only-placeholder');

const handlers = new Map();
globalThis.window = { localStorage: storage, addEventListener: (name, callback) => handlers.set(name, callback) };
const blocked = load('../web/src/services/blockedKeywords.ts');
const cache = blocked.getBlockedKeywords();
const reads = storage.reads;
for (let index = 0; index < 10000; index += 1) blocked.getBlockedKeywords().value.includes('example');
assert.equal(storage.reads, reads);
blocked.saveBlockedKeywords([' Hello ', 'hello']);
assert.deepEqual([...cache.value], ['hello']);
storage.failWrites = 1;
assert.throws(() => blocked.saveBlockedKeywords(['replacement']), /原设置已保留/);
assert.deepEqual([...cache.value], ['hello']);
handlers.get('storage')({ key: 'danmu_block_keywords', storageArea: storage, newValue: '["Elsewhere"]' });
assert.deepEqual([...cache.value], ['elsewhere']);
console.log('PASS: config validation, rollback/recovery, cookie preservation, keyword hot path and synchronization');

const diagnostics = load('../web/src/services/diagnostics.ts');
const safe = diagnostics.diagnosticDetails(new Error('HTTP 403 https://example.invalid/?token=hidden Cookie=hidden'));
assert.deepEqual(safe, { category: 'rejected', code: '403' });
assert.ok(!JSON.stringify(safe).includes('hidden'));
console.log('PASS: diagnostic output excludes URLs and credentials');
