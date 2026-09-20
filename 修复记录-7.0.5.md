# 7.0.5 抖音弹幕签名字节序修复

## 结论

发现并修正了 Rhino 与 V8 的 TypedArray 字节序差异，原先导致抖音签名中的 MD5 摘要错误。修正后，三个固定输入的签名均与 DTV 原脚本在 V8 中的结果一致；复用原生代码的在线诊断获得 HTTP 101，并在一个公开开播样本中解码出 35 条聊天消息，达到安装门槛。

这证明本次原生实现至少在该电脑端样本中可完成握手、心跳、ACK 和聊天解码，不代表所有房间、手机网络及长期稳定性已经验证。没有进行手机直播、交互或视觉验收。

## 原因与修正依据

DTV 固定参考提交：`e44388b24622860b9b0642d5ee29374cb89ed3c0`。

- 上游 sign.js 的 MD5 实现让 Uint8Array 和 Uint32Array 共用 ArrayBuffer。V8 使用小端读写；Rhino 1.7.15 的 ContextFactory 默认禁用 FEATURE_LITTLE_ENDIAN，按大端读写。
- 固定同一虚拟时间、随机数序列、浏览器环境和非敏感输入后，两引擎生成了不同签名。进一步跟踪发现：输入字节完全相同，二次 MD5 摘要已经不同，并非仅签名字符串随机变化。
- 对空内容的 MD5 字节再次求 MD5：V8 得到 `59adb24ef3cdbe0297f05b395827453f`，旧 Rhino 得到 `45f0e771ff42e5bef201e906d58b2915`。桌面标准 MD5 也确认前者正确。
- 原版与仅修改作用域后的脚本在 V8 中结果一致；原版在 Rhino 中仍有已知的作用域错误，因此保留原有作用域兼容修改。
- 仅在抖音签名调用中显式启用 FEATURE_LITTLE_ENDIAN。保留 Rhino 隔离、3 秒执行时限、取消和栈限制；斗鱼仍走默认配置。

参考源码：
https://github.com/mozilla/rhino/blob/Rhino1_7_15_Release/src/org/mozilla/javascript/ContextFactory.java
https://github.com/chen-zeong/DTV/blob/e44388b24622860b9b0642d5ee29374cb89ed3c0/src-tauri/src/platforms/douyin/danmu/sign.js

## 本次其他改动

- 新增共用 DouyinSession 解析器，优先读取 URL 编码 RENDER_DATA 中 app.odin 的访客 ID，兼容已有页面字段。获取失败重新准备一次会话；仍失败则显示会话准备错误，移除随机访客 ID 回退。
- Cookie Token 不作为访客 ID。使用当前响应的 Cookie、页面访客 ID、房间真实 ID 和 UA 准备连接。会话刷新只重建匿名访客 Cookie，不删除平台登录凭证。
- 本地补齐 msToken 使用 DTV 的字母数字集合；平台实际返回的 Token 保持原值。
- 握手拒绝信息增加响应分类：明确标记的验证 HTML、普通 HTML、可解析 JSON、空响应或未知。只提取数字业务错误码，不输出完整响应或凭证。未知 HTTP 200 不自动认定为风控。
- 实际发送握手时仅记录 Upgrade、Connection 及请求路径是否与准备结果相符的布尔值，不记录签名 URL。
- 未改 FLV、播放线路、播放器或其他平台弹幕协议；无新运行时依赖。

## 诊断结果

离线记录：`artifacts/douyin-705-signature-comparison.log`。

三个非敏感输入在修正后的 Rhino 与 V8 原脚本/V8 修改脚本之间得到相同的签名摘要。原版 Rhino 的作用域失败也保留在记录中。

在线记录：`artifacts/douyin-7.0.5-diagnosis.log`。

诊断程序 `artifacts/DouyinHandshakeDiagnosis705.java` 复用应用编译出的 ScriptSandbox、DouyinSession、DouyinDanmaku 和 Proto，使用同一签名资产、Rhino 和 WebSocket 库。电脑端用匿名 HTTP 会话代替 Android 的凭证存储，不读取手机私有数据。

| 项目 | 结果 |
| --- | --- |
| 公开房间详情状态 | 2，正在开播 |
| 访客 ID 来源 | page_field，使用共用解析器 |
| Upgrade / Connection / 请求路径 | 三项均匹配 |
| 服务器 | webcast3-ws-web-lq.douyin.com |
| 握手 | HTTP 101 |
| 累计在线诊断时间 | 29004 ms |
| hb Ping 成功发送 | 6 |
| 二进制消息 | 87 |
| ACK 成功发送 | 87 |
| 聊天消息解码 | 35 |

本轮仅使用一个在线样本，成功后停止；未播放视频、未发送聊天。字节序单项对照在离线固定条件下完成，在线未再开启旧配置连接做额外对照。因此可确认签名错误及新实现样本成功，不能声称历史上每一次 HTTP 200 都已被单独证明由这一原因导致。

本次成功路径未产生 HTTP 200，拒绝响应分类、RENDER_DATA 分支、会话缺失、超时重连与快速换房仅作代码审查，未宣称在线验证。

## 检查和交付边界

先编译诊断所需的应用类，完成离线和限定在线诊断；达到 HTTP 101 及聊天解码门槛后才更新版本并进行 release 打包。没有运行测试套件、lint 或独立类型检查。必要编译自然包含 Kotlin 编译器检查。

审查覆盖：抖音专用字节序配置不会改变其他签名调用；日志不泄露响应正文或凭证；取消关闭 socket 和心跳；房间代次过滤迟到回调；会话最多重取一次；握手服务器切换和重连仍有上限。

版本 `7.0.5 / 7000005`，包名 `com.simplelive.nativeapp`；沿用本地 release 签名。安装目标为 `A4UF6R6206008108`，使用 install -r 保留数据与旧应用，不自动启动应用，不提交、推送或发布。


### 最终产物与安装结果

- Release 构建成功，记录：`artifacts/native-7.0.5-build.log`。
- APK：`Simple-live-v7.0.5-native-release.apk`。
- APK SHA-256：`FADDF64CD5462F7A94DDA75570B7F0FF52EE009C9A962CFDB9F8B545FEE11E28`。
- apksigner 校验通过；证书 SHA-256：`00581020a3b8faaf38fbf2c6541c6e2bdca5fbeff1f2cf53e1ac19d307433b68`，与旧版本一致。
- `adb -s A4UF6R6206008108 install -r` 返回 Success；安装后核对版本为 `7.0.5 / 7000005`。
- 手机旧包 `www.sp.com` 仍安装；没有清除数据或启动应用。
