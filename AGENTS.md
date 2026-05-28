# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

简直播 (Simple-Live) — a multi-platform live-streaming app built with Tauri 2 + Vue 3, targeting Android mobile (primary) and desktop (Windows/macOS/Linux). Supports 斗鱼/抖音/虎牙/Bilibili/Custom-M3U8.

## Build & Run

```powershell
# Install frontend dependencies (run once)
cd web; npm install

# Sync Rust core → web/src-tauri (required before any Tauri command)
npm run android:sync

# Android init (first time only; skip if Gradle project already exists in app/)
npm run android:init
powershell -ExecutionPolicy Bypass -File ..\scripts\sync-generated-android.ps1

# Android debug run
npm run android:dev

# Frontend-only dev (no Tauri)
npm run dev

# Type-check + build frontend
npm run build
```

**Always sync before building for Android.** The `core/` directory is the authoritative Rust source; `android:sync` mirrors it into `web/src-tauri/` via robocopy (skips `target/` and `gen/`).

### Android release build

```powershell
# Full build (Rust + frontend + APK)
cd web
npm run android:sync
npx tauri android build

# Workaround: if symlink fails on Windows (Developer Mode disabled), manually copy .so then run Gradle:
Copy-Item -Force "web\src-tauri\target\aarch64-linux-android\release\libdtv_lib.so" "app\app\src\main\jniLibs\arm64-v8a\"
Copy-Item -Force "web\src-tauri\target\aarch64-linux-android\release\libdtv_lib.so" "web\src-tauri\gen\android\app\src\main\jniLibs\arm64-v8a\"
cd web\src-tauri\gen\android
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
.\gradlew.bat assembleArm64Release -x rustBuildArm64Release
# APK output: web\src-tauri\gen\android\app\build\outputs\apk\arm64\release\app-arm64-release.apk
```

## Architecture

### Directory layout

- `web/` — Vue 3 + Vite + Pinia + xgplayer frontend (the app UI)
- `core/` — Rust/Tauri backend: proxy server, platform integrations, danmaku clients, update system
- `app/` — Android Gradle project (staging area for `tauri android init` output)
- `scripts/` — PowerShell sync helpers (`sync-core.ps1`, `sync-generated-android.ps1`)

### Rust backend (`core/src/`)

Entry point: `core/src/lib.rs` — the `run()` function builds the Tauri app, registers all `#[tauri::command]` handlers, and manages shared state (`DouyuDanmakuHandles`, `StreamUrlStore`, proxy handles, etc.).

Key modules:
- `proxy.rs` — local HTTP proxy (actix-web on ports 34719/34721) for stream URLs and image proxying
- `platforms/` — one submodule per platform (`douyu/`, `douyin/`, `huya/`, `bilibili/`, `common/`), each containing stream URL resolution, danmaku WebSocket handling, search, and room info fetching
- `update_release.rs` — in-app APK update check and download
- `config_transfer.rs` — export/import app config (followed streamers, settings, etc.)

### Frontend (`web/src/`)

- `App.vue` — mobile-first shell: topbar + `<router-view>` with `<keep-alive>` (caches all 6 home views) + bottom nav + sheet overlays (search, follows, settings, player fullscreen)
- `router/index.ts` — route per platform: `/` (斗鱼 home), `/<platform>` (homes), `/player/<platform>/:roomId` (players)
- `pages/` — one HomeView and one PlayerView per platform + shared `PlayerView.vue`
- `components/player/` — xgplayer wrapper, danmaku overlay, controls (with Tauri IPC bridge for danmaku)
- `components/FollowsList/` — follow list with folders, drag-and-drop sorting, streamer items, platform filter overlay (`FollowOverlay.vue`), folder context menu (rename/delete)
- `components/CommonCategory/` / `CommonStreamerList/` — reusable list views for browsing categories and live rooms
- `platforms/` — per-platform TypeScript modules (API calls, parsers, player helpers, cookie state)
  - `platforms/common/types.ts` — shared TypeScript interfaces (`BaseStreamer`, `LiveStreamer`, `FollowedStreamer`, `CommonDanmakuMessage`, etc.)
- `store/` — Pinia stores: `followStore.ts` (followed streamers + folders, transactions for drag-drop), `customM3u8Store.ts`, `categoryStore.ts`, `customCategoryStore.ts`
- `stores/` — additional Pinia stores: `theme.ts`, `bilibili.ts`
- `runtime/` — Tauri/Web runtime adapters (open URL, clipboard-based config export/import via `copyTextToClipboard`, update check)
- `services/configTransfer.ts` — portable config payload format (`dtv-config` v1) for clipboard-based import/export
- `mobile/` — mobile-specific components (bottom nav, sheets for search/follows/settings)

### Platform support pattern

Each platform follows this pattern on both Rust and TS sides:

| Layer | Rust (`core/src/platforms/<name>/`) | TS (`web/src/platforms/<name>/`) |
|-------|-------------------------------------|---------------------------------|
| Stream URL | `stream_url.rs` — fetch play URLs | `playerHelper.ts` — call Tauri commands |
| Danmaku | `danmu/` — WebSocket client, protobuf decode | `danmuOverlay.ts` — render via danmu.js |
| Live list | `fetch_*_rooms` / `live_list.rs` | composable in `CommonStreamerList/` |
| Search | `search.rs` or inline in `lib.rs` | page-level call to Tauri command |
| Category | `fetch_categories` | `*CategoriesData.ts` + API calls |

### State management

- Pinia stores persist via localStorage (follow store, settings, M3U8 sources)
- Rust managed state: `reqwest::Client`, `FollowHttpClient`, danmaku handles (per-room oneshot channels for stop signaling), proxy server handle, `StreamUrlStore`
- Theme: auto-detected via `prefers-color-scheme` + manual toggle, synced to Tauri native window theme

### Config import/export

Config is exported/imported via clipboard (not file download/upload):
- **Export**: `createPortableConfigPayload()` collects all localStorage keys matching `isExportableStorageKey()`, serializes to JSON, copies to clipboard via `copyTextToClipboard()` in `runtime/host.ts` (uses `navigator.clipboard.writeText` with `execCommand('copy')` fallback)
- **Import**: user pastes JSON into a text dialog (`MobileSettingsSheet.vue` or `Navbar.vue` config menu), then `parsePortableConfigPayload()` validates and `replacePortableConfigEntries()` applies to localStorage
- **Mobile**: `App.vue` + `MobileSettingsSheet.vue` (sheet dialog with textarea)
- **Desktop**: `Navbar.vue` (inline textarea within config dropdown menu)
- Success messages: green "已导出所有配置至粘贴板！" / "导入成功！", error: red "导入失败！"

## 编码准则

**权衡取舍：** 这些准则偏向于谨慎而非速度。对于琐碎的任务，请自行斟酌。

### 1. 编码前先思考

**不要主观臆断。不要掩盖疑惑。把权衡取舍摆到台面上。**

在进行实现之前：

- 明确陈述你的假设。如果不确定，请提问。
- 如果存在多种解释，请将它们列出——不要默默地自行决定。
- 如果有更简单的方案，请提出来。在有正当理由时提出反对意见。
- 如果有任何不清楚的地方，请停下来。指明让人困惑的地方。提问。

### 2. 简单优先

**用最少的代码解决问题。不做任何预测性开发。**

- 不添加要求之外的功能。
- 不为一次性代码做抽象处理。
- 不提供未被要求的"灵活性"或"可配置性"。
- 不为不可能发生的场景编写错误处理。
- 如果你写了 200 行代码，但其实 50 行就能搞定，请重写。

问问你自己："高级工程师会认为这太复杂了吗？"如果是，请简化。

### 3. 外科手术般的精准修改

**只改动必须改动的地方。只清理你自己制造的烂摊子。**

在编辑现有代码时：

- 不要去"改进"相邻的代码、注释或格式。
- 不要重构没有出问题的东西。
- 保持现有的代码风格，即使你个人倾向于不同的做法。
- 如果你注意到不相关的死代码，可以提出来——但不要删除它。

当你的修改产生了孤立（弃用）的代码时：

- 删除由于**你的**修改而变得不再使用的导入（imports）、变量或函数。
- 除非被明确要求，否则不要删除原先就存在的死代码。

检验标准：每一行被修改的代码都应该能直接追溯到用户的请求。

### 4. 目标驱动执行

**定义成功标准。循环直到验证通过。**

将任务转化为可验证的目标：

- "添加验证" → "为无效输入编写测试，然后让它们通过"
- "修复 bug" → "编写一个能复现该 bug 的测试，然后让它通过"
- "重构 X" → "确保重构前后测试都能通过"

对于多步任务，请简述计划：

```
1. [步骤] → 验证：[检查项]
2. [步骤] → 验证：[检查项]
3. [步骤] → 验证：[检查项]
```

明确的成功标准能让你独立进行循环工作。模糊的标准（"让它跑起来"）则需要不断地确认。

---

**如果满足以下条件，说明这些准则正在发挥作用：** 代码对比（diffs）中不必要的修改变少，因过度复杂导致的重写减少，并且澄清性的问题出现在动手实现之前，而不是犯错之后。

## 代码规范

你是一位极其严苛的软件架构师和代码审查员。在生成任何代码时，你必须绝对遵守以下所有规则。任何偏离都不可接受。

### 1. 编码风格与格式化

- **Python**：严格遵循 PEP 8。4 空格缩进，行长 ≤ 79，在运算符周围加空格。
- **JavaScript/TypeScript**：遵循 Airbnb 风格指南。2 空格缩进，句末加分号，使用单引号，对象尾随逗号。
- **其他语言**：遵循该社区最主流、最严格的官方风格指南。
- 始终保持一致的括号、引号和大括号换行风格。
- 禁止保留无意义的空白行或行尾空格。

### 2. 命名规范

- 类/接口/类型：PascalCase（例：`UserService`）。
- 函数/方法/变量：camelCase（例：`getUserById`）；Python 使用 `snake_case`（例：`get_user_by_id`）。
- 常量/枚举值：UPPER_SNAKE_CASE。
- 私有成员：Python 使用单下划线前缀 `_private`；JS/TS 使用 `#private` 或 `private` 关键字。
- 名字必须自解释。杜绝任何单字母变量（`i`、`j`、`k` 作为循环索引除外）和模糊缩写。

### 3. 注释与文档

- 每个公共类、函数、方法必须有完整的文档字符串（Python）或 JSDoc（JS/TS），并说明：功能、参数、返回值、可能抛出的异常。
- 非显而易见的复杂逻辑必须添加行内注释，解释"为什么这样做"，而非复述代码。
- TODO/FIXME 必须带有日期和简洁说明，格式：`// TODO(2026-05-06): 需要实现缓存策略`。

### 4. 错误处理与健壮性

- 永远不要吞没异常（禁止空的 `except` 或 `catch` 块）。至少记录日志。
- 自定义错误必须继承合适的异常基类。
- 所有外部输入（用户、API、文件）必须经过验证和清理。
- 业务逻辑中严格禁止使用 `assert`，它只用于测试。

### 5. 代码结构与设计

- 严格遵循单一职责原则。每个函数理想不超过 20 行，只做一件事。
- 嵌套层级不超过 3 层。用提前返回、提取函数等方式减少嵌套。
- 严禁魔法数字/字符串，必须定义为有意义的命名常量。
- 优先使用不可变数据。避免改变传入的参数。
- 通过依赖注入传递外部服务，严禁内部硬编码 new 创建。

### 6. 安全

- 密钥、密码、Token **绝对不能**以任何形式硬编码在代码中。必须读环境变量或安全配置服务。
- 防范 SQL 注入、XSS 等 OWASP Top 10 漏洞。SQL 必须使用参数化查询，输出必须转义。
- 代码中不得出现任何可用于攻击的示例数据、真实 IP 或凭证。

### 7. 性能与可读性

- 避免不必要的循环和深层递归。优先使用内置的高阶函数（map/filter/reduce）或语言特性。
- 可读性胜过微小优化。如果存在更清晰的写法，不得为了极微小的性能提升而牺牲可读性。

### Python 特别强化

- 所有函数签名必须使用完整的类型注解（含参数和返回值）。
- 必须使用 `mypy` 严格模式能通过的语法。避免使用 `Any`，除非极其必要并注释原因。
- 优先使用 `dataclasses` 或 `Pydantic` 定义数据结构，而非裸字典。
- 上下文管理器必须用来管理资源（文件、锁、数据库连接等）。
- 使用 `pathlib` 处理路径，使用 `f-string` 格式化字符串。

### TypeScript 特别强化

- 开启严格模式（strict: true）所需的所有规范：noImplicitAny, strictNullChecks 等。
- 禁止使用 `any`，必须使用具体类型或 `unknown` 配合类型守卫。
- 优先使用 `const` 和 `let`，杜绝 `var`。函数优先使用箭头函数（除非需要动态 this）。
- 异步操作必须使用 `async/await`，并且带 try/catch 处理错误。
- React/Vue 等框架组件必须遵循其官方风格指南（如函数组件、Hooks 规则等）。

## Git 提交

每次修改代码完成之后都要提交 git，确保可以回滚代码。
