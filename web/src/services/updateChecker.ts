const REPOSITORY_OWNER = 'kanxi12138';
const REPOSITORY_NAME = 'Simple-live';
const RELEASES_API = `https://api.github.com/repos/${REPOSITORY_OWNER}/${REPOSITORY_NAME}/releases/latest`;
export const RELEASES_PAGE = `https://github.com/${REPOSITORY_OWNER}/${REPOSITORY_NAME}/releases`;

export interface ReleaseAssetInfo {
  name: string;
  contentType: string;
  downloadUrl: string;
  size: number;
}

export interface LatestReleaseInfo {
  currentVersion: string;
  latestVersion: string;
  releaseTag: string;
  publishedAt: string;
  hasUpdate: boolean;
  htmlUrl: string;
  apkAsset: ReleaseAssetInfo | null;
}

interface GithubReleaseAssetPayload {
  name?: string;
  content_type?: string;
  browser_download_url?: string;
  size?: number;
}

interface GithubLatestReleasePayload {
  tag_name?: string;
  name?: string;
  html_url?: string;
  published_at?: string;
  assets?: GithubReleaseAssetPayload[];
}

export const normalizeVersion = (version: string): string => version.trim().replace(/^v/i, '');

const compareVersions = (left: string, right: string): number => {
  const leftParts = normalizeVersion(left).split('.').map((part) => Number(part) || 0);
  const rightParts = normalizeVersion(right).split('.').map((part) => Number(part) || 0);
  const maxLength = Math.max(leftParts.length, rightParts.length);

  for (let index = 0; index < maxLength; index += 1) {
    const delta = (leftParts[index] || 0) - (rightParts[index] || 0);
    if (delta !== 0) {
      return delta;
    }
  }

  return 0;
};

const selectApkAsset = (
  assets: GithubReleaseAssetPayload[] | null | undefined,
): ReleaseAssetInfo | null => {
  if (!Array.isArray(assets) || assets.length === 0) {
    return null;
  }

  const matchedAsset = assets.find((asset) => asset.content_type === 'application/vnd.android.package-archive')
    ?? assets.find((asset) => (asset.name || '').toLowerCase().endsWith('.apk'));

  if (!matchedAsset?.name || !matchedAsset.browser_download_url) {
    return null;
  }

  return {
    name: matchedAsset.name,
    contentType: matchedAsset.content_type || 'application/octet-stream',
    downloadUrl: matchedAsset.browser_download_url,
    size: matchedAsset.size || 0,
  };
};

export const checkLatestRelease = async (currentVersion: string): Promise<LatestReleaseInfo> => {
  const response = await fetch(RELEASES_API, {
    headers: {
      Accept: 'application/vnd.github+json',
    },
  });

  if (!response.ok) {
    throw new Error(`Release check failed: ${response.status}`);
  }

  const payload = await response.json() as GithubLatestReleasePayload;
  const latestVersion = normalizeVersion(payload.tag_name || payload.name || '');

  if (!latestVersion) {
    throw new Error('Latest release version is missing.');
  }

  return {
    currentVersion: normalizeVersion(currentVersion),
    latestVersion,
    releaseTag: payload.tag_name || `v${latestVersion}`,
    publishedAt: payload.published_at || '',
    hasUpdate: compareVersions(latestVersion, currentVersion) > 0,
    htmlUrl: payload.html_url || RELEASES_PAGE,
    apkAsset: selectApkAsset(payload.assets),
  };
};
