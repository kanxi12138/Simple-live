# Android build progress

Last updated: 2026-04-28

## Current status

- No Android build processes are currently running.
- Latest user-requested UI changes are implemented and verified on a real Android device over ADB:
  - top-right theme toggle now switches light/dark correctly
  - subscription chips now show a small `-` button on the right
  - tapping that `-` opens the same delete confirmation dialog as left-swipe
  - top-left app mark now shows `简` instead of the old image/`D`
- Final installable signed APK:
  - `<PROJECT_ROOT>\简直播-v2.0.0-arm64-release.apk`
- New release keystore used for this package:
  - `<PROJECT_ROOT>\release\jianzhibo-v200-release.jks`

## Important code changes

- Android web app:
  - `<PROJECT_ROOT>\web\src\stores\theme.ts`
    - added explicit `toggleTheme()`
  - `<PROJECT_ROOT>\web\src\App.vue`
    - now uses store-level theme toggle
  - `<PROJECT_ROOT>\web\src\mobile\MobileTopbar.vue`
    - brand mark changed to `简`
    - added light/dark topbar styling so theme switching is visually obvious
  - `<PROJECT_ROOT>\web\src\pages\CustomHomeView.vue`
    - added right-side `-` remove button for subscription chips
    - wired `-` to the same delete confirmation flow as swipe-delete

- Android packaging workaround:
  - `<PROJECT_ROOT>\app\app\build.gradle.kts`
    - keeps `rust.rootDirRel = "../../web/src-tauri"`
  - `<PROJECT_ROOT>\app\buildSrc\src\main\java\com\dtv\app\kotlin\BuildTask.kt`
    - fixed wrong copy destination from `app/src/main/jniLibs/...` to `src/main/jniLibs/...`
    - this matters because the old path copied the rebuilt Rust lib into an unused nested directory

## Packaging facts

1. `npm run android:build` from `.android/web` still fails at the final symlink step on this Windows machine.
2. Even though that command fails, it successfully rebuilds the correct Android Rust library first:
   - `<PROJECT_ROOT>\web\src-tauri\target\aarch64-linux-android\release\libdtv_lib.so`
3. The reliable release flow is:
   - rebuild frontend in `.android/web`
   - run `npm run android:build -- --target aarch64` once to regenerate the release `.so`
   - copy that `.so` into `<PROJECT_ROOT>\app\app\src\main\jniLibs\arm64-v8a\libdtv_lib.so`
   - build APK from `.android/app` with Gradle
   - sign with the new keystore using `zipalign` + `apksigner`

## Metadata verified

- `application-label`: `简直播`
- `versionName`: `2.0.0`
- `versionCode`: `2000000`
- package id: `com.dtv.app`
- release signature is new and does not match the old installed package
  - device install required uninstalling the old app first

## Real-device verification done

Device tested:
- `DEVICE_SERIAL` (`AAK_AN00`)

Verified on device:
1. Home top-left mark shows `简`
2. Theme toggle switches between light and dark
3. Added a subscription from the category `+`
4. Subscription page shows chip `斗鱼-英雄联盟`
5. Chip has right-side `-`
6. Tapping `-` opens delete confirmation dialog

## Old packages cleanup

- Old APKs in `<PROJECT_ROOT>` were removed.
- Only the latest final package remains there:
  - `<PROJECT_ROOT>\简直播-v2.0.0-arm64-release.apk`

## If the user says “继续执行”

Resume from here:

1. Reuse the existing final APK unless they explicitly want another rebuild.
2. If another rebuild is needed, reuse the keystore at:
   - `<PROJECT_ROOT>\release\jianzhibo-v200-release.jks`
3. If another install test is needed, remember the old signed app must be uninstalled first if the signature changes again.

## Useful commands

Rebuild frontend:

```powershell
cd <PROJECT_ROOT>\web
npm run build
```

Refresh the Android Rust library through Tauri:

```powershell
cd <PROJECT_ROOT>\web
npm run android:build -- --target aarch64
```

Copy the rebuilt Rust lib into the Android app module:

```powershell
Copy-Item `
  <PROJECT_ROOT>\web\src-tauri\target\aarch64-linux-android\release\libdtv_lib.so `
  <PROJECT_ROOT>\app\app\src\main\jniLibs\arm64-v8a\libdtv_lib.so `
  -Force
```

Build the unsigned APK:

```powershell
cd <PROJECT_ROOT>\app
.\gradlew.bat clean assembleArm64Release --stacktrace
```

Sign the APK:

```powershell
$bt = 'ANDROID_SDK_ROOT\build-tools\36.0.0'
& "$bt\zipalign.exe" -f -p 4 `
  <PROJECT_ROOT>\app\app\build\outputs\apk\arm64\release\app-arm64-release-unsigned.apk `
  <PROJECT_ROOT>\简直播-v2.0.0-arm64-release-aligned.apk

& "$bt\apksigner.bat" sign `
  --ks <PROJECT_ROOT>\release\jianzhibo-v200-release.jks `
  --ks-key-alias jianzhibo_v200 `
  --ks-pass pass:REDACTED_SIGNING_PASSWORD `
  --key-pass pass:REDACTED_SIGNING_PASSWORD `
  --out <PROJECT_ROOT>\简直播-v2.0.0-arm64-release.apk `
  <PROJECT_ROOT>\简直播-v2.0.0-arm64-release-aligned.apk
```
