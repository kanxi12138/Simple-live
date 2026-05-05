export interface WebDavConfig {
  url: string;
  username: string;
  password: string;
  fileName: string;
}

const joinUrl = (baseUrl: string, fileName: string) => {
  const normalizedBase = baseUrl.trim().replace(/\/+$/, '');
  const normalizedFile = fileName.trim().replace(/^\/+/, '');
  return `${normalizedBase}/${normalizedFile}`;
};

const buildAuthHeader = (username: string, password: string) =>
  `Basic ${btoa(`${username}:${password}`)}`;

export const uploadConfigToWebDav = async (config: WebDavConfig, content: string): Promise<void> => {
  const response = await fetch(joinUrl(config.url, config.fileName), {
    method: 'PUT',
    headers: {
      Authorization: buildAuthHeader(config.username, config.password),
      'Content-Type': 'application/json;charset=utf-8',
    },
    body: content,
  });
  if (!response.ok) {
    throw new Error(`WebDAV upload failed: ${response.status}`);
  }
};

export const downloadConfigFromWebDav = async (config: WebDavConfig): Promise<string> => {
  const response = await fetch(joinUrl(config.url, config.fileName), {
    method: 'GET',
    headers: {
      Authorization: buildAuthHeader(config.username, config.password),
    },
  });
  if (!response.ok) {
    throw new Error(`WebDAV download failed: ${response.status}`);
  }
  return response.text();
};
