const RELEASES_API = 'https://api.github.com/repos/kanxi12138/SLR/releases/latest';
export const RELEASES_PAGE = 'https://github.com/kanxi12138/SLR/releases';

export interface UpdateCheckResult {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  htmlUrl: string;
}

const normalizeVersion = (version: string) => version.trim().replace(/^v/i, '');

const compareVersions = (left: string, right: string) => {
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

export const checkLatestRelease = async (currentVersion: string): Promise<UpdateCheckResult> => {
  const response = await fetch(RELEASES_API, {
    headers: {
      Accept: 'application/vnd.github+json',
    },
  });
  if (!response.ok) {
    throw new Error(`Release check failed: ${response.status}`);
  }
  const payload = await response.json();
  const latestVersion = normalizeVersion(payload.tag_name || payload.name || '');
  return {
    currentVersion: normalizeVersion(currentVersion),
    latestVersion,
    hasUpdate: compareVersions(latestVersion, currentVersion) > 0,
    htmlUrl: payload.html_url || RELEASES_PAGE,
  };
};
