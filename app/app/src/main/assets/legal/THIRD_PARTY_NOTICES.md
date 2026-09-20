# Third-party copyright and license notices

## Combined distribution

Simple-live is distributed as a combined work under GNU GPL version 3 (GPL-3.0-only); see LICENSE. Protocol code was ported from GPL-3.0 code and is linked into the app, so retaining only an MIT notice would not describe the combined distribution correctly. This does not relicense independently licensed third-party components or remove their copyright notices.

The original DTV/web MIT notice, Copyright (c) 2025 c.zeong, is retained in web/LICENSE and docs/licenses/DTV-MIT.txt. The vendored tars-stream MIT notice, Copyright (c) 2018 zerolocust, is retained in core/vendor/tars-stream/LICENSE and its mirrored copy.

## dart_simple_live adaptations

Source: https://github.com/xiaoyaocz/dart_simple_live/tree/ba828e6783b176ea5709fcd09f0eb01dfaceeb51/simple_live_core

Copyright xiaoyaocz and contributors. GNU GPL version 3. The upstream root LICENSE at this exact commit was retrieved and compared with docs/licenses/dart_simple_live-GPL-3.0.txt on 2026-09-14; the texts match after newline normalization.

The Douyu, Huya, Douyin and Bilibili protocol implementations in core/src/platforms, their web/src-tauri mirror, corresponding frontend helpers, and Android Douyu protocol handling include adaptations of this source. The Douyin signature JavaScript retains the upstream implementation. Sources and original porting commits are recorded in docs/v5.2.0-porting.md and Git history.

Changes on 2026-09-14: removed official desktop SDK identity and WUP token fetching in Huya; sign only page-supplied AntiCode; removed unconditional Douyin page fallback; isolated Douyu signing from the privileged WebView; restricted network targets, local playback sessions, retries and diagnostics. These adaptations are modified works, not official platform clients.

## DTV Bilibili playback adaptation

Source: https://github.com/chen-zeong/DTV/blob/d2221304f48b62792769e00355c79dc0612303c3/src-tauri/src/platforms/bilibili/stream_url.rs

Copyright (c) 2025 c.zeong. MIT license; full notice retained in docs/licenses/DTV-MIT.txt. Modified on 2026-09-15 for Simple-live 5.2.2: use room_init before playback, adapt HTML5 requests and FLV/HLS selection to the existing response and player, preserve request limits and network restrictions, isolate CDN probes from login cookies, and report API stages and error codes. Existing dart_simple_live-derived helpers remain under their original GPL-3.0 terms.

## Other dependencies

Bilibili danmaku initialization also references DTV's auth.rs: https://github.com/chen-zeong/DTV/blob/d2221304f48b62792769e00355c79dc0612303c3/src-tauri/src/platforms/bilibili/auth.rs. Modified on 2026-09-15 to resolve rooms with room_init and include web_location in getDanmuInfo signing, retaining the existing asynchronous listener and packet decoding.

Installed npm production dependencies and Cargo metadata are inventoried, with available license texts, in docs/licenses/DEPENDENCIES.txt. Entries include build and possibly other-target dependencies. Declared SPDX expressions are preserved; missing license texts and native/bundled notices require the additional review described in docs/compliance-audit.md. Embedded CryptoJS and QuickJS/V8 retain their upstream terms; no ownership over third-party code is claimed.

App settings include offline access to this notice, GPL-3.0, the original MIT notice, privacy information and the dependency inventory.

## Binary distribution

For any future APK release, provide the exact corresponding source revision, build scripts and required dependency sources/notices alongside the binary. Preserve modification and copyright notices. Merely linking the latest main branch is insufficient to identify older binaries. This audit does not certify every historical binary's license compliance.

References: https://www.gnu.org/licenses/gpl-3.0.html and https://www.gnu.org/licenses/gpl-faq.html.en

## DTV Huya / Douyin playback adaptation (2026-09-16)

Source: https://github.com/chen-zeong/DTV/blob/main/src-tauri/src/platforms/huya/stream_url.rs and https://github.com/chen-zeong/DTV/blob/main/src-tauri/src/platforms/douyin/douyin_streamer_detail.rs.
Copyright (c) 2025 c.zeong, MIT; full notice retained in docs/licenses/DTV-MIT.txt.
Simple-live 6.0.0 adapts WUP CDN token encoding/decoding and HYSDK playback headers, and keyed Douyin FLV fallback. Requests retain local limits and address restrictions; response bodies, credentials and signed URLs are excluded from playback diagnostics. Existing GPL-derived helpers retain their original notices. This supersedes the 2026-09-14 page-only Huya token behavior, without rewriting that historical record.

On 2026-09-17, Huya CDN priority and Douyin per-message error isolation / five-second Ping heartbeat also reference DTV's `src-tauri/src/platforms/huya/stream_url.rs` and `src-tauri/src/platforms/douyin/danmu/{message_handler,websocket_connection}.rs`. Simple-live adds bounded FLV probes, cancellation, scoped CDN redirects and finite reconnects; it does not enable unsupported WebSocket compression. The DTV MIT notice above applies to these adaptations.

## Android 原生迁移（2026-09-18）

7.0.0 将上述平台协议和交互迁移至 app/app/src/main/kotlin/com/simplelive/nativeapp。Compose、Media3、OkHttp、Room 和 DataStore 替代 Vue/Tauri/Rust 运行链路；Rhino 仅用于平台签名脚本，官方网页登录保留专用 WebView。既有 MIT/GPL 派生声明继续有效。

历史声明中的 web/、core/ 路径现可在迁移前归档和 Git 历史中查阅。DTV-MIT.txt、dart_simple_live-GPL-3.0.txt 和 tars-stream 原始许可证保留于 docs/licenses 与 APK 离线资源中。新 Kotlin TARS 编解码器根据现有协议代码重新实现，不链接旧 Rust tars-stream 库。

原生依赖及完整许可证见 docs/licenses/ANDROID-NATIVE-DEPENDENCIES.txt 及相邻许可证文件。旧 npm/Cargo 依赖清单仅是历史迁移资料，不代表新 APK 的依赖。

## DTV Bilibili list compatibility (2026-09-19)

Simple-live 7.0.2 adapts the anonymous list request headers, runtime access_id and list signature from DTV commit e44388b24622860b9b0642d5ee29374cb89ed3c0, files src-tauri/src/platforms/bilibili/live_list.rs and state.rs. Copyright (c) 2025 c.zeong, MIT; full notice retained in docs/licenses/DTV-MIT.txt. Kotlin adaptations isolate list requests from account credentials, omit sensitive diagnostics and bind any official verification result to one request. The official Bilibili captcha SDK is loaded remotely for user-initiated verification, not bundled or relicensed by this project.


## Douyin danmaku compatibility (7.0.4)

Douyin handshake parameters and get_sign script are adapted from DTV commit e44388b24622860b9b0642d5ee29374cb89ed3c0, src-tauri/src/platforms/douyin/danmu/{websocket_connection.rs,signature.rs,sign.js}. Copyright (c) 2025 c.zeong; DTV MIT notice retained. Existing derived-work notices remain applicable. The always-true initialization block is flattened for Rhino function scope compatibility.

nv-websocket-client 2.14, Copyright (C) 2015 Takahiko Kawasaki, Apache License 2.0. Used only for Douyin danmaku WebSocket transport. Full license: docs/licenses/nv-websocket-client-LICENSE.txt. Source: https://github.com/TakahikoKawasaki/nv-websocket-client/tree/nv-websocket-client-2.14

7.0.5 configures Rhino FEATURE_LITTLE_ENDIAN for Douyin signing only, matching the shared Uint8Array/Uint32Array storage used by the upstream script on V8. This does not change Rhino source or add a Java bridge.
