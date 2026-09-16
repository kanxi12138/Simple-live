# Windows arm64 release 构建

所有命令从仓库根目录执行。使用 Node/npm、Rust Android arm64 target、JDK、Android SDK 和 NDK；SDK、NDK 路径由当前机器配置。`core` 是 Rust 修改源，`web/src-tauri` 为同步镜像。

## 前端和 Android 工程

```powershell
npm.cmd --prefix web ci
npm.cmd --prefix web run build
powershell -ExecutionPolicy Bypass -File scripts/sync-core.ps1
npm.cmd --prefix web run android:init
```

`android:init` 完成后自动将 `app` 中 Git 跟踪的源码和配置应用到生成工程；已有生成工程执行 `npm.cmd --prefix web run android:apply`。同步不反向覆盖 `app`，不复制签名配置、JNI 或构建产物。

将本机 SDK 配置及以下签名属性写入生成工程根目录的 `local.properties`，不要提交该文件或密钥：`release.keystore.path`、`release.keystore.storePassword`、`release.keystore.keyAlias`、`release.keystore.keyPassword`。使用已有 release keystore。

## Rust release 与普通 JNI 文件复制

先配置 `ANDROID_HOME` 和 `NDK_HOME`，后者指向已安装 NDK 目录。以下流程与已完成的 Android API 24 arm64 构建一致：

```powershell
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$env:NDK_HOME/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android24-clang.cmd"
$env:CC_aarch64_linux_android = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$env:CXX_aarch64_linux_android = "$env:NDK_HOME/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android24-clang++.cmd"
$env:AR_aarch64_linux_android = "$env:NDK_HOME/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-ar.exe"
$env:TAURI_ANDROID_PACKAGE_UNESCAPED = 'www.sp.com'
cargo build --manifest-path web/src-tauri/Cargo.toml --target aarch64-linux-android --release --lib --features tauri/custom-protocol
New-Item -ItemType Directory -Force web/src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a
Copy-Item web/src-tauri/target/aarch64-linux-android/release/libdtv_lib.so web/src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/libdtv_lib.so -Force
```

JNI 目标必须是普通文件，不能是旧符号链接。`tauri/custom-protocol` 用于嵌入生产前端资源；前端构建应先完成。

## Android 打包

```powershell
Push-Location web/src-tauri/gen/android
.\gradlew.bat assembleArm64Release --console=plain '-Pkotlin.incremental=false' -x rustBuildArm64Release -x lint -x lintArm64Release -x lintVitalArm64Release -x lintVitalAnalyzeArm64Release -x lintVitalReportArm64Release
Pop-Location
```

输出位于 `web/src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release.apk`。使用 aapt 和 apksigner 核对包名 `www.sp.com`、版本 `5.2.2/5002002`、仅 arm64-v8a 及原 release 证书。安装时使用 `adb install -r`，签名不一致则停止，不卸载或清数据。

APK、JNI、构建目录和本地配置不属于源码提交。定向验证入口见 `docs/repair-validation.md`；不运行全仓 Lint 或直播实测。
