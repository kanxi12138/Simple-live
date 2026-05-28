import { invoke } from '@tauri-apps/api/core';
import { isTauriRuntime, openExternal } from './host';
import { RELEASES_PAGE, type LatestReleaseInfo } from '../services/updateChecker';

export interface ApkDownloadResult {
  version: string;
  fileName: string;
  filePath: string;
}

interface AndroidUpdateBridge {
  installApk(filePath: string): void;
  canRequestPackageInstalls(): boolean;
  openUnknownAppSourcesSettings(): void;
}

const getAndroidUpdateBridge = (): AndroidUpdateBridge | null => {
  if (typeof window === 'undefined') {
    return null;
  }

  return window.DTVUpdate ?? null;
};

export const canUseInAppUpdate = (): boolean => isTauriRuntime() && getAndroidUpdateBridge() !== null;

export const fetchLatestReleaseInfo = async (): Promise<LatestReleaseInfo> => {
  return await invoke<LatestReleaseInfo>('fetch_latest_release_info_cmd');
};

export const downloadLatestReleaseApk = async (): Promise<ApkDownloadResult> => {
  return await invoke<ApkDownloadResult>('download_release_apk_cmd');
};

export const ensureInstallPermission = (): boolean => {
  const bridge = getAndroidUpdateBridge();
  if (!bridge) {
    return false;
  }

  if (bridge.canRequestPackageInstalls()) {
    return true;
  }

  bridge.openUnknownAppSourcesSettings();
  return false;
};

export const installDownloadedApk = (filePath: string): boolean => {
  const bridge = getAndroidUpdateBridge();
  if (!bridge || !filePath.trim()) {
    return false;
  }

  bridge.installApk(filePath);
  return true;
};

export const fallbackToReleasesPage = async (url?: string): Promise<void> => {
  await openExternal(url || RELEASES_PAGE);
};
