/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

interface Window {
  DTVDebug?: {
    log(level: string, message: string): void;
  };
  DTVUpdate?: {
    installApk(filePath: string): void;
    canRequestPackageInstalls(): boolean;
    openUnknownAppSourcesSettings(): void;
  };
  DTVDouyuDanmaku?: {
    start(roomId: string): void;
    stop(): void;
  };
}
