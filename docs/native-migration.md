# Android 原生迁移 7.0.0

## 功能对照

| 功能 | 原生实现 |
| --- | --- |
| 四平台分类、分页直播列表、搜索、房间详情、画质与线路 | `LivePlatform` 与四个 Kotlin 平台适配器 |
| 斗鱼三级分类、抖音直播与账号搜索 | 首页分类面板、搜索模式切换 |
| 播放、暂停、刷新、音量、音频焦点、横竖屏与全屏 | Media3 ExoPlayer、PlayerView、Compose 控制栏 |
| 四平台弹幕连接、心跳、有限重连 | OkHttp WebSocket、Kotlin TARS/Protobuf、zlib/Brotli/gzip 解码 |
| 弹幕列表、滚动/顶部/底部弹幕、区域/密度/速度/透明度/字号、屏蔽词 | Compose 列表和 Canvas、设置面板 |
| 关注、备注、置顶、排序、分组、移动、状态刷新、筛选、展开 | Room、本地关注页 |
| 订阅分类 | Room、首页订阅标签 |
| 浅色、深色、跟随系统、播放偏好 | DataStore |
| 官方登录 | 专用 WebView，无应用 JS 桥；登录凭证保存到 Keystore 加密存储 |
| 配置导入导出 | 系统文件选择器、JSON 粘贴、dtv-config v1、导入前校验、回滚记录 |
| 应用更新 | GitHub Release 查询、下载、包名/版本/签名验证、系统安装器 |
| 版权、隐私、依赖许可证 | APK 内离线 assets/legal |

“全部 Kotlin”指界面、业务、网络、存储、平台协议都由 Kotlin 原生实现。官方网页登录与平台签名脚本是用户确认保留的两个兼容边界；不保留 Vue、Tauri、Rust、网页播放器或本机直播转发服务。AndroidX 本身携带的原生库仍属于标准依赖。

## 数据与包身份

新包名 `com.simplelive.nativeapp`，版本 `7.0.0`，versionCode `7000000`。旧包名 `www.sp.com` 保留在手机上，不读取其私有数据；旧版导出的配置可以在原生版手动导入。凭证不包含在导出中，登录需重新完成。

迁移前的当前工作区源码归档为 `artifacts/migration/pre-native-source.zip`，包括当时的未提交与未跟踪源码，排除本地签名配置、密钥文件、依赖和构建产物。该本地归档不提交 Git。此前提交的代码和修改历史仍保留在 `.git`。

最终清理时，批量永久删除被自动审批策略以 `blocked by policy` 拒绝。因此采用可恢复的归档移动：旧 `web/`、`core/`、Rust 产物、同步脚本、旧 Android 桥接及废弃文档移至 `artifacts/migration/legacy-source/`，不再参与原生构建，没有永久删除这些文件。签名密钥和旧生成工程的本地配置保留在被 Git 忽略的 `app/keystore/` 中。

## 设计

依据 `D:/Claude_Code/前端SKILL/SKILL.md`：直播封面是视觉重点，工具区保持简洁。首页双列封面，关注页使用分组列表；蓝色只用于选中状态和主要操作。

浅色背景 #F5F8FC，表面 #FFFFFF，正文 #17263C，强调色 #1764D9，次要文字 #52647B。深色采用 #101A2A / #17263C。使用系统中文字体、22sp 标题、14–16sp 正文及 Android 字体缩放；主要操作触控目标至少 48dp。

## 验证边界

按本次要求仅进行代码审查、生成 APK 必需的编译/资源处理/打包、APK 身份核验和指定设备的 ADB 安装结果检查。不运行单元测试、独立类型检查、lint、视觉验收或真实直播验收。

代码审查覆盖平台数据字段、签名与协议字节序、房间切换取消、弹幕任务生命周期、关注刷新竞争、导入验证与失败恢复、凭证作用域、流地址域名限制、更新身份校验。审查及编译不等于运行功能验收；第三方接口和签名脚本在真实设备上的行为仍未经验证。

## 本次交付结果

- Release 构建成功，交付文件为根目录 `Simple-live-v7.0.0-native-release.apk`。
- APK 身份为 `com.simplelive.nativeapp / 7.0.0 / 7000000`，minSdk 24、targetSdk 36；签名与旧 6.0.0 release APK 相同。
- APK 中未发现 `libdtv_lib.so`、Tauri 资源或旧网页入口，包含 15 份离线版权资源。
- 已通过 ADB 安装到 `A4UF6R6206008108`（AAK_AN00），安装命令返回 Success，系统查询确认新版本；旧应用 `www.sp.com` 仍然安装。
- 未主动启动新旧应用，未进行运行或视觉验收，未提交、推送或发布远程 Release。
- 构建日志、APK 身份记录和安装记录分别保存在 `artifacts/native-build.log`、`artifacts/native-apk-verification.json`、`artifacts/native-installation.json`。
