# DTV Mobile Architecture

## Layout

- `web/`: Vue 3 + Vite + Pinia + xgplayer frontend
- `core/`: Rust/Tauri commands, proxy, platform integrations
- `app/`: fixed landing zone for Android packaging files and generated Gradle project
- `scripts/`: sync helpers

## Build flow

1. Edit frontend in `web/`
2. Edit Rust in `core/`
3. Run `web/npm run android:sync` to mirror `core` into `web/src-tauri`
4. Run Tauri Android commands from `web/`
5. Mirror generated Android files into `app/`

## Mobile UX goals

- Phone-first shell with bottom navigation and sheets
- Fast route changes with low-motion transitions
- Search debouncing and request invalidation
- Reuse existing list virtualization and incremental loading
- Keep player route focused on fast first frame and minimal chrome
