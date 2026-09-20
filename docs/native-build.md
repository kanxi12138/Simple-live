# 原生 Android 构建

源码入口为 `app/`，应用代码在 `app/app/src/main/kotlin/com/simplelive/nativeapp/`。

环境：JDK 17 或兼容的 JDK 21、Android SDK 36、Gradle Wrapper 8.14.3。无需 Node、Rust、Tauri 或 NDK 工具链来编译应用自有代码。

在 `app/local.properties` 中配置 `sdk.dir`，以及既有 release 签名的 `release.keystore.path`、`release.keystore.storePassword`、`release.keystore.keyAlias`、`release.keystore.keyPassword`。本地文件和签名密钥不提交；密钥路径沿用原有相对于应用模块的解析规则。

从仓库根目录运行：

```powershell
Push-Location app
.\gradlew.bat assembleRelease --console=plain -x lint -x lintVitalRelease -x lintVitalAnalyzeRelease -x lintVitalReportRelease
Pop-Location
```

输出：`app/app/build/outputs/apk/release/app-release.apk`。交付副本：`Simple-live-v7.0.8-native-release.apk`。

本机 Google Maven 域名解析为回环地址，项目因此配置了阿里云 Google Maven 镜像，Maven Central 继续使用官方仓库；不修改系统 DNS 或 hosts。

安装前以 Android SDK 的 `aapt` 与 `apksigner` 核对新包名 `com.simplelive.nativeapp`、版本 `7.0.8 / 7000008` 和现有 release 证书。ADB 必须显式指定目标设备：

```powershell
& "$env:LOCALAPPDATA/Android/Sdk/platform-tools/adb.exe" -s A4UF6R6206008108 install -r Simple-live-v7.0.8-native-release.apk
& "$env:LOCALAPPDATA/Android/Sdk/platform-tools/adb.exe" -s A4UF6R6206008108 shell pm path com.simplelive.nativeapp
```

不卸载 `www.sp.com`，不清除手机数据，不自动启动应用或进行运行验收。没有发布远程 Release。
