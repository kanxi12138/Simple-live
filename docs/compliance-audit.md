# Simple-live 技术风险审计

审查日期：2026-09-14 至 2026-09-15。范围：当前源码及已有未提交修改，Rust 平台适配、Vue 前端、Android 桥接、内嵌脚本、配置迁移、依赖、构建和发布配置。本文不构成法律意见或合法性认证。

## 修改前发现及处理

下列位置为整改前工作区的行号，修改后的行号会变化；Rust 镜像目录执行相同整改。

| 位置 | 发现 | 处理 |
| --- | --- | --- |
| `core/src/proxy.rs:217,288` | 任意来源 CORS、本机服务固定端口；图片 URL 可任意指定，使用字符串包含判断平台；端口占用被当成已有服务 | 改为随机回环端口、严格 DNS 后缀匹配、允许来源列表及 256 位随机会话凭证，拒绝未知上游和未经验证的服务 |
| `core/src/stream_proxy.rs:34,77` | HLS 子资源只验证协议，可能跨平台转发；错误含上游详情 | 同平台 CDN 白名单、同主机安全重定向、清单大小上限、仅内存资源映射；拒绝加密播放清单；客户端只允许播放兼容头，不转发 Cookie/Authorization |
| `web/src/services/configTransfer.ts:20` | 配置包含 B站 Cookie 和自定义 M3U8 地址 | 移除两类数据的导入导出；保留设置、关注和分组 |
| `web/src/store/customM3u8Store.ts:66` 及相关页面 | 长期保存及播放任意自定义源 | 删除存储、页面、路由、平台入口和播放器分支；启动清理旧源数据，关注列表清理旧自定义源记录 |
| `app/app/src/main/java/www/sp/com/MainActivity.kt:101` | 发布版无条件开启 WebView 调试，桥接输出动态诊断 | 仅调试构建启用；诊断只保留静态事件信息；关闭 Android 应用数据备份及 file:// 页面跨文件/跨域访问 |
| `core/src/platforms/huya/cdn_token.rs:12,25,101` | 使用官方桌面 SDK User-Agent、身份字段及 WUP 请求新 Token | 删除 WUP 获取与官方 SDK 身份，改用公开移动页面提供的 AntiCode，缺失则暂不可用 |
| `core/src/platforms/douyin/web_api.rs` | enter API 任意失败后回退 HTML | 删除无条件回退；HTTP 或平台状态拒绝时返回不可用；不将 douyin.com Cookie 发往 amemv.com |
| `web/src/platforms/douyu/playerHelper.ts` 的 `executeDouyuSign` | 通过 `new Function` 在具有登录和原生桥权限的 WebView 执行平台脚本 | 移到原生隔离解释器，不提供 WebView、文件、网络或 IPC 回调；Android 限制脚本大小及解释器内存 |
| `web/src/components/player/playerControlsPlugins.ts` | 平台画质及线路名称插入 HTML | 改用 textContent，防止名称被解释为脚本或标签 |
| Rust/前端日志与 Android 诊断 | 地址、错误对象、原始返回、弹幕与用户标识可能进入诊断 | 应用诊断改为静态事件标记，前端平台错误返回不包含原始 URL/响应 |
| `web/src/assets/fonts` | 两份 OPPO 字体子集没有随附可核对的授权声明 | 移除内嵌字体文件和加载规则，使用系统字体 |

没有发现专门的直播录制、FLV/HLS 文件保存、直播下载、二次推流、开发者中转服务器、公共解析网站、刷人气、刷礼物或账号自动操作功能。APK 下载属于更新功能；Protobuf 的 member/pay 字段用于解析平台消息，不代表付费破解。播放器临时缓冲及当前弹幕列表不是录像或弹幕数据库。

## 请求与播放生命周期

- 两端搜索防抖为 400 ms；同一平台请求合并，前端待处理队列有上限。
- Rust 平台 HTTP 统一通过 `request_limit`，每平台最多两个响应占用请求槽，消费或丢弃响应后释放；请求设 20 秒超时。瞬时连接失败或超时最多退避重试两次，平台 HTTP 拒绝和业务拒绝不自动重试。播放媒体分片不走 API 限流器。
- 同参数播放解析结果仅在内存中保留最多五秒；停止、切房、页面退出会清理。已排队的旧播放请求不会再解析，正在执行的原生请求会在超时内结束且不回填旧缓存。
- 弹幕重连有次数上限；平台连接初始化失败直接停止。正常心跳和播放分片请求仍持续到连接关闭，不把正常协议循环误判为批量采集。
- 本地服务没有解析 API，只消费 App 已获取的当前播放源；不持久化源，不上传给维护者。非法或未知 CDN 被拒绝，可能影响部分线路。

## 四个平台保留的兼容实现

| 实现 | 保留理由与边界 | 剩余风险 |
| --- | --- | --- |
| Douyin A-Bogus | 签署公开房间/搜索请求所需参数；平台拒绝即返回错误 | 非公开稳定 API，平台条款、签名实现来源和授权、接口变化及风控风险仍在 |
| Douyu H5 signature | homeH5Enc / ub98484234 / getH5Play 的公开播放兼容；脚本不再进入 App WebView | 仍执行第三方脚本；Android 设置内存上限但没有强制 CPU 执行时限；不得据此声称安全沙箱已获完整验证 |
| Huya CDN Token | 根据公开页面提供的 AntiCode 生成当前播放请求参数；不再请求官方桌面 WUP Token | Token 算法、wsSecret/seqid 等仍属于平台接口兼容；页面缺失字段、CDN 拒绝或白名单不足时会不可用 |
| Bilibili WBI | 使用平台返回的 WBI 信息签名，使用实际返回的可选画质和 CDN；运行时 Cookie/buvid 保留 | 平台认证、API 使用条款、用户会话与本地明文存储风险；实际能返回地址不等于取得使用授权 |

非商业、免费、开源及客户端直连都不能消除版权、网络视听、平台接口、商标、不正当竞争或隐私方面的风险。这里仅列出技术位置，是否进一步删除由维护者决定。

## 数据流与隐私

```text
平台信息：用户设备 → 第三方平台 API / 弹幕服务
播放：设备内播放器 → 设备内受限回环服务 → 第三方平台 CDN
更新：用户设备 → GitHub API / Release 资源
```

没有发现“平台 → 开发者服务器 → 用户”或“用户 → 开发者服务器 → 平台”的实现。Cookie/Token 用于对应平台功能，不上传开发者服务器、广告或统计服务；B站服务端返回的弹幕目标另有域名检查。未发现广告 SDK、第三方统计 SDK 或用户行为追踪 SDK。

**按维护者要求保留的例外：** B站 Cookie 的 localStorage 和抖音 WebView CookieManager 保存方式不变，未迁移到系统安全凭据库。因此不能宣称已经满足最初的“敏感数据安全存储”要求。详细保存、用途及清理方式见 PRIVACY.md。未做运行时全量流量和系统/播放器日志捕获。

## 许可证

指定上游提交 `ba828e6783b176ea5709fcd09f0eb01dfaceeb51` 的 GPL-3.0 全文已在线核对；整体 GPL-3.0 分发，原 DTV MIT 和 tars-stream MIT 声明继续保留。修改说明、来源及 Git 历史保留，App 内离线展示许可、隐私和第三方声明。

依赖清单覆盖安装的 npm 生产包和 Cargo Android 元数据，并附可取得的本地/上游许可文本；元数据包含部分构建和其他目标依赖，不能据此声称每项都进入 APK。仍未完整核对 `convert_case@0.4.0`、`deno_core_icudata@0.0.73`、`fxhash@0.2.1`、`mac@0.1.1`、`proc-macro-rules@0.4.0`、`proc-macro-rules-macros@0.4.0`、`delegate@3.2.0`、`mitt@2.1.0`、`tao-macros@0.1.3` 的对应上游完整许可文本。它们的声明已记录；本次不能保证第三方通知义务已全部满足。

Android 声明依赖包含 AndroidX、Material、OkHttp 及 Tauri 插件；没有把 Material/Google Maven 依赖误判为 Google Analytics。原有图标、平台标识、截图及历史代码的全部权利链也未获独立授权证明。发布 APK 前仍需完成这些来源/通知核验，并提供对应版本源码与构建资料。

## 验证与交付状态

前端生产构建（含项目既有 vue-tsc 步骤）、Android arm64 Rust release 编译与 Android assembleArm64Release 均通过。Gradle 排除重复 Rust 构建任务及 lint，Kotlin 非增量编译；未运行额外测试。APK 签名通过，原 release 证书 SHA-256 保持为 00581020a3b8faaf38fbf2c6541c6e2bdca5fbeff1f2cf53e1ac19d307433b68。

产物：artifacts/Simple-live-v5.2.1-compliance-arm64.apk，15,809,226 字节，SHA-256：2a1806945077b2f17e8450eb63aa289a3b84ef6d15a022ed3b226e19d3dd09c5。包名 www.sp.com，版本 5.2.1 / 5002001，minSdk 24，targetSdk 36，仅 arm64-v8a；非 debuggable，allowBackup=false。APK 中原生库的 SHA-256 与本次 Rust 产物一致，避免误用旧 JNI 文件。

对源码常见凭据模式及 APK 的 .so/.dex/.js/.json/.xml/.txt/.properties 条目进行了静态扫描，没有匹配私钥、GitHub Token 或常见云密钥模式；APK 没有 .env、local.properties、key.properties、keystore/jks/p12/pfx 或 private.key 文件名。扫描不是对所有未知凭据格式的证明。源码与 Rust 镜像逐文件一致。

关键词复查记录见 audit-keywords.tsv（258 个源码/配置文件、439 个文件与关键词命中项；供应商代码、静态分类数据的许可证/来源另行检查）。Android 运行依赖清单见 licenses/ANDROID-DEPENDENCIES.txt，共 76 项，包含约束项和本地 POM 许可声明。没有发现广告或统计 SDK。

新 APK 仅作为本地交付，不提交二进制、不创建 GitHub Release。远程提交结果将在交付消息说明。没有运行单元测试、独立 lint、格式化、视觉验收或四个平台在线功能验证；编译成功不能证明平台播放可用。

完整文件清单见本文末尾的交付清单。

## 交付文件清单

相对本地任务起点的变更（包含用户授权纳入的既有源码改动；A 新增、M 修改、D 删除）：

```text
M	.gitignore
A	COMPLIANCE.md
A	LICENSE
A	PRIVACY.md
M	README.md
A	SECURITY.md
D	Simple-live-v5.2.0-release.apk
A	THIRD_PARTY_NOTICES.md
M	app/app/build.gradle.kts
M	app/app/src/main/AndroidManifest.xml
D	app/app/src/main/java/www/sp/com/BackgroundPlaybackService.kt
A	app/app/src/main/java/www/sp/com/DouyinLoginBridge.kt
M	app/app/src/main/java/www/sp/com/MainActivity.kt
M	core/Cargo.lock
M	core/Cargo.toml
M	core/src/lib.rs
A	core/src/network_policy.rs
M	core/src/platforms/bilibili/auth.rs
M	core/src/platforms/bilibili/danmaku.rs
M	core/src/platforms/bilibili/live_list.rs
M	core/src/platforms/bilibili/mod.rs
D	core/src/platforms/bilibili/models.rs
M	core/src/platforms/bilibili/search.rs
M	core/src/platforms/bilibili/state.rs
M	core/src/platforms/bilibili/stream_url.rs
M	core/src/platforms/bilibili/streamer_info.rs
D	core/src/platforms/bilibili/websocket.rs
M	core/src/platforms/common/http_client.rs
M	core/src/platforms/common/mod.rs
A	core/src/platforms/common/request_limit.rs
A	core/src/platforms/douyin/account_search.rs
M	core/src/platforms/douyin/danmu/message_handler.rs
M	core/src/platforms/douyin/danmu/message_parsers.rs
M	core/src/platforms/douyin/danmu/sign.js
M	core/src/platforms/douyin/danmu/signature.rs
M	core/src/platforms/douyin/danmu/web_fetcher.rs
M	core/src/platforms/douyin/danmu/websocket_connection.rs
M	core/src/platforms/douyin/douyin_danmu_listener.rs
M	core/src/platforms/douyin/douyin_streamer_detail.rs
M	core/src/platforms/douyin/mod.rs
A	core/src/platforms/douyin/room_page.rs
M	core/src/platforms/douyin/search.rs
M	core/src/platforms/douyin/web_api.rs
M	core/src/platforms/douyu/danmu_start.rs
M	core/src/platforms/douyu/fetch_douyu_main_categories.rs
M	core/src/platforms/douyu/fetch_douyu_room_info.rs
M	core/src/platforms/douyu/live_list.rs
M	core/src/platforms/douyu/search_anchor.rs
M	core/src/platforms/douyu/stream_url.rs
M	core/src/platforms/douyu/three_cate.rs
A	core/src/platforms/huya/cdn_token.rs
M	core/src/platforms/huya/danmaku.rs
M	core/src/platforms/huya/live_list.rs
M	core/src/platforms/huya/mod.rs
M	core/src/platforms/huya/search.rs
M	core/src/platforms/huya/stream_url.rs
M	core/src/proxy.rs
A	core/src/stream_proxy.rs
M	core/tauri.conf.json
M	docs/HBUILDER_PACKAGING_GUIDE.md
A	docs/audit-keywords.tsv
M	docs/build.md
A	docs/compliance-audit.md
A	docs/licenses/ANDROID-DEPENDENCIES.txt
A	docs/licenses/DEPENDENCIES.txt
A	docs/licenses/DTV-MIT.txt
A	docs/licenses/dart_simple_live-GPL-3.0.txt
A	docs/v5.2.0-porting.md
A	docs/v5.2.1-frontend.md
M	web/package-lock.json
M	web/package.json
M	web/src-tauri/Cargo.toml
M	web/src-tauri/src/lib.rs
A	web/src-tauri/src/network_policy.rs
M	web/src-tauri/src/platforms/bilibili/auth.rs
M	web/src-tauri/src/platforms/bilibili/danmaku.rs
M	web/src-tauri/src/platforms/bilibili/live_list.rs
M	web/src-tauri/src/platforms/bilibili/mod.rs
D	web/src-tauri/src/platforms/bilibili/models.rs
M	web/src-tauri/src/platforms/bilibili/search.rs
M	web/src-tauri/src/platforms/bilibili/state.rs
M	web/src-tauri/src/platforms/bilibili/stream_url.rs
M	web/src-tauri/src/platforms/bilibili/streamer_info.rs
D	web/src-tauri/src/platforms/bilibili/websocket.rs
M	web/src-tauri/src/platforms/common/http_client.rs
M	web/src-tauri/src/platforms/common/mod.rs
A	web/src-tauri/src/platforms/common/request_limit.rs
A	web/src-tauri/src/platforms/douyin/account_search.rs
M	web/src-tauri/src/platforms/douyin/danmu/message_handler.rs
M	web/src-tauri/src/platforms/douyin/danmu/message_parsers.rs
M	web/src-tauri/src/platforms/douyin/danmu/sign.js
M	web/src-tauri/src/platforms/douyin/danmu/signature.rs
M	web/src-tauri/src/platforms/douyin/danmu/web_fetcher.rs
M	web/src-tauri/src/platforms/douyin/danmu/websocket_connection.rs
M	web/src-tauri/src/platforms/douyin/douyin_danmu_listener.rs
M	web/src-tauri/src/platforms/douyin/douyin_streamer_detail.rs
M	web/src-tauri/src/platforms/douyin/mod.rs
A	web/src-tauri/src/platforms/douyin/room_page.rs
M	web/src-tauri/src/platforms/douyin/search.rs
M	web/src-tauri/src/platforms/douyin/web_api.rs
M	web/src-tauri/src/platforms/douyu/danmu_start.rs
M	web/src-tauri/src/platforms/douyu/fetch_douyu_main_categories.rs
M	web/src-tauri/src/platforms/douyu/fetch_douyu_room_info.rs
M	web/src-tauri/src/platforms/douyu/live_list.rs
M	web/src-tauri/src/platforms/douyu/search_anchor.rs
M	web/src-tauri/src/platforms/douyu/stream_url.rs
M	web/src-tauri/src/platforms/douyu/three_cate.rs
A	web/src-tauri/src/platforms/huya/cdn_token.rs
M	web/src-tauri/src/platforms/huya/danmaku.rs
M	web/src-tauri/src/platforms/huya/live_list.rs
M	web/src-tauri/src/platforms/huya/mod.rs
M	web/src-tauri/src/platforms/huya/search.rs
M	web/src-tauri/src/platforms/huya/stream_url.rs
M	web/src-tauri/src/proxy.rs
A	web/src-tauri/src/stream_proxy.rs
M	web/src-tauri/tauri.conf.json
M	web/src/App.vue
D	web/src/assets/fonts/OPPOSans-Bold-subset.woff2
D	web/src/assets/fonts/OPPOSans-Regular-subset.woff2
A	web/src/components/Common/LegalNotices.vue
M	web/src/components/Common/SmoothImage.vue
M	web/src/components/CommonCategory/components/Cate1List.vue
M	web/src/components/CommonStreamerList/composables/useBilibiliLiveRooms.ts
M	web/src/components/CommonStreamerList/composables/useDouyinLiveRooms.ts
M	web/src/components/CommonStreamerList/composables/useDouyuLiveRooms.ts
M	web/src/components/CommonStreamerList/composables/useHuyaLiveRooms.ts
M	web/src/components/CommonStreamerList/index.vue
M	web/src/components/DanmuList/index.vue
M	web/src/components/DouyuCategory/composables/useCategories.ts
M	web/src/components/DouyuCategory/composables/useSelection.ts
M	web/src/components/DouyuCategory/index.vue
M	web/src/components/FollowsList/index.css
M	web/src/components/FollowsList/index.vue
M	web/src/components/FollowsList/useProxy.ts
M	web/src/components/StreamerInfo/index.vue
M	web/src/components/player/constants.ts
M	web/src/components/player/controlLayout.ts
M	web/src/components/player/danmakuManager.ts
M	web/src/components/player/danmuOverlay.ts
M	web/src/components/player/index.vue
M	web/src/components/player/lineOptions.ts
M	web/src/components/player/player.css
M	web/src/components/player/playerControlsPlugins.ts
M	web/src/components/player/plugins.ts
M	web/src/components/player/watchers.ts
M	web/src/components/window-controls/WindowsWindowControls.vue
M	web/src/layout/Navbar.vue
M	web/src/layout/types.ts
M	web/src/main.ts
M	web/src/mobile/MobileBottomNav.vue
M	web/src/mobile/MobileFollowsSheet.vue
M	web/src/mobile/MobileSearchSheet.vue
M	web/src/mobile/MobileSettingsSheet.vue
M	web/src/mobile/MobileTopbar.vue
D	web/src/pages/CustomM3u8HomeView.vue
D	web/src/pages/CustomM3u8PlayerView.vue
M	web/src/pages/DouyinPlayerView.vue
M	web/src/pages/DouyuHomeView.vue
M	web/src/pages/DouyuPlayerView.vue
M	web/src/pages/HuyaPlayerView.vue
M	web/src/pages/PlayerView.vue
M	web/src/platforms/bilibili/cookieHelper.ts
M	web/src/platforms/bilibili/playerHelper.ts
M	web/src/platforms/common/apiService.ts
A	web/src/platforms/common/playback.ts
A	web/src/platforms/common/playbackProxy.ts
M	web/src/platforms/common/streamerTypes.ts
M	web/src/platforms/common/types.ts
M	web/src/platforms/douyin/followListHelper.ts
M	web/src/platforms/douyin/playerHelper.ts
A	web/src/platforms/douyin/searchResults.ts
M	web/src/platforms/douyin/streamerInfoParser.ts
M	web/src/platforms/douyu/api.ts
M	web/src/platforms/douyu/followListHelper.ts
M	web/src/platforms/douyu/parsers.ts
M	web/src/platforms/douyu/playerHelper.ts
M	web/src/platforms/douyu/streamerInfoParser.ts
M	web/src/platforms/huya/playerHelper.ts
M	web/src/router/index.ts
M	web/src/runtime/androidDiagnostics.ts
M	web/src/runtime/host.ts
M	web/src/runtime/updateHost.ts
M	web/src/services/configTransfer.ts
A	web/src/services/platformInvoke.ts
M	web/src/store/categoryStore.ts
M	web/src/store/customCategoryStore.ts
D	web/src/store/customM3u8Store.ts
M	web/src/store/followStore.ts
M	web/src/stores/bilibili.ts
M	web/src/styles/global.css
A	web/src/styles/ui-polish.css
```

## 远程合并说明

合入远程已有源码发布提交，保留其历史文档隐私清理与版本配置：app/app/.gitignore、app/app/tauri.properties、docs/v5.2.0-porting.md、docs/v5.2.1-frontend.md。冲突仅按已经审查的风险整改保留，未强制覆盖远程。合并没有改变已构建的 Rust、前端及 Android 业务代码；原生版本属性与生成工程的 5.2.1 / 5002001 一致。
