# Build Notes

## Frontend

```powershell
cd <PROJECT_ROOT>\web
npm install
npm run build
```

## Sync Rust core

```powershell
cd <PROJECT_ROOT>\web
npm run android:sync
```

## Initialize Android project

```powershell
cd <PROJECT_ROOT>\web
npm run android:init
powershell -ExecutionPolicy Bypass -File ..\scripts\sync-generated-android.ps1
```

## Debug run

```powershell
cd <PROJECT_ROOT>\web
npm run android:dev
```

## Current limitations

- This workspace now contains the mobile-oriented source split and Android staging files.
- Final Android Gradle files still depend on `tauri android init` because the local SDK/NDK and signing environment are machine-specific.
