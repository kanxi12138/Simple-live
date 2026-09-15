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
