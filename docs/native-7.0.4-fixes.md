# 7.0.4 抖音弹幕握手改动与诊断

## 结论：弹幕连接仍未解决

已完成 DTV 协议兼容迁移，但限定诊断的两个公开开播样本均未成功握手，不能将本版描述为弹幕恢复。HTTP 200 仅表示收到普通 HTTP 响应；WebSocket 升级必须返回 101。当前证据无法确定平台拒绝与签名、访客会话或风控之间的具体因果关系。

## 改动

- 固定参考 DTV 提交 `e44388b24622860b9b0642d5ee29374cb89ed3c0` 的 `src-tauri/src/platforms/douyin/danmu/websocket_connection.rs`、`signature.rs`、`sign.js`。
- 采用同一版本的 SDK 1.0.14-beta.0、cursor、internal_ext、签名字段顺序和 get_sign 入口；不再反复生成并筛除含特定字符的签名。房间、访客、Cookie、UA 和时间戳组成一次连接快照，服务器切换复用该快照。
- 签名脚本保留 Rhino 作用域兼容修改。共用 ScriptSandbox 保留禁止 Java 桥接、3 秒执行时限、协程取消、脚本大小与栈深限制；斗鱼签名环境与入口保持原样。
- 新增 nv-websocket-client 2.14，仅承担抖音弹幕。使用公开 sendPing 接口，成功连接后立即发送 Protobuf hb Ping，之后间隔 5 秒。由库协商 permessage-deflate 并自动以原负载回复 Pong；消息内部 gzip 独立处理。
- 五个 DTV 服务器顺序尝试，每次握手最多 12 秒，失败关闭 socket。只有已经连接后断开才允许最多两轮自动重连，优先上次成功服务器；五个服务器全部握手失败则停止。手动重连不刷新视频。
- 保留 4 MiB 回调消息及 gzip 解压输出限制、Protobuf 字段检查、ACK log_id/internal_ext 和单条聊天解析隔离。WebSocket 压缩解码在库中完成，回调大小限制不等同于库解压过程的内存硬上限。
- 连接和心跳由协程管理，取消关闭底层 socket；沿用 ViewModel 房间/弹幕代次过滤迟到状态。错误回调去重，显示阶段、服务器及可用的 HTTP 状态。
- 不改 FLV、播放源或其他平台协议；增加依赖许可及 APK 离线说明。

## 电脑端诊断证据

2026-09-19，使用两个房间详情接口返回 status=2 的公开样本。列表本身不带有效 status，故通过详情确认后才连接。

诊断程序 `artifacts/DouyinHandshakeDiagnosis.java` 直接使用 release 编译产物中的 ScriptSandbox、DouyinDanmaku.prepare/session 和 Proto，以及 APK 使用的同一 sign.js、Rhino 1.7.15 和 nv-websocket-client 2.14。电脑端仅替换 Android 凭证/HTTP 环境为匿名会话获取，不读取手机凭证。

| 样本 | 五服务器尝试总耗时 | 握手响应 | 成功握手 | hb Ping | 二进制帧 | ACK | 聊天 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 1754 ms | 全部 HTTP 200 | 0 | 0 | 0 | 0 | 0 |
| 2 | 936 ms | 全部 HTTP 200 | 0 | 0 | 0 | 0 | 0 |

两次签名计算均返回结果，但服务端接受性没有得到验证。每个样本连接累计低于 30 秒，没有播放视频或发送聊天。达到两个房间的范围后停止，不再扩大直播诊断。

原始结果：`artifacts/douyin-7.0.4-diagnosis.log`。Windows 输出编码造成中文阶段显示乱码，但域名、HTTP 状态、计数和最终 NOT_CONFIRMED 结果可读。原始日志中同一握手失败被库的两个回调重复记录，10 条失败记录对应 5 次握手；交付代码已去重，未为这项日志修正新增直播连接。

## 代码审查与边界

已审查签名异常、五服务器切换、握手超时、ACK 编码、单消息失败、重连耗尽及换房/退出资源释放。上述成功握手后路径仅作代码审查；由于没有 101，心跳发送、ACK、聊天解码和断线恢复并未取得在线执行证据。

按约定未运行测试套件、lint、独立类型检查，也未进行手机直播、交互或视觉验收。执行了必要 release 构建及电脑端诊断。构建或安装成功不代表弹幕恢复。

## 构建与安装

版本 `7.0.4 / 7000004`，包名 `com.simplelive.nativeapp`。沿用本地 release 签名，不提交或输出密钥。设备 `A4UF6R6206008108`，安装使用 `adb install -r`，不卸载旧应用、不清数据、不自动启动应用。

产物及安装结果见本记录末尾。


### 交付结果

- Release 构建成功：`artifacts/native-7.0.4-final-build.log`。
- APK：`Simple-live-v7.0.4-native-release.apk`。
- APK SHA-256：`4A10DF6070E4CCCDE35518FEB3836886EEBDE553CFE6C2D58F71C0AEA4CA3F3F`。
- apksigner 校验通过；证书 SHA-256：`00581020a3b8faaf38fbf2c6541c6e2bdca5fbeff1f2cf53e1ac19d307433b68`，与原版本相同。
- `adb -s A4UF6R6206008108 install -r` 返回 `Success`；安装后读取版本为 `7.0.4 / 7000004`。
- 已确认原生包与旧包 `www.sp.com` 均仍安装；未清数据、未启动应用。
- 弹幕恢复成功标准未达到。后续需要进一步判明 HTTP 200 拒绝的服务端原因，不能宣称本版已解决握手。
