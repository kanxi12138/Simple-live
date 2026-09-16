/** Returns only a fixed category and numeric codes, never the original error text. */
export const diagnosticDetails = (error: unknown): { category: string; code?: string } => {
  const message = typeof error === 'string' ? error : error instanceof Error ? error.message : '';
  const code = message.match(/(?:HTTP\s+|code\s*[=:]\s*)(-?\d{1,6})\b/i)?.[1];
  const category = code === '401' || code === '403' ? 'rejected'
    : /timeout|超时/i.test(message) ? 'timeout'
    : /认证|签名|拒绝|forbidden/i.test(message) ? 'rejected'
      : /存储|写入|保存|恢复|quota/i.test(message) ? 'storage'
        : /格式|解析|invalid|parse/i.test(message) ? 'invalid-data'
          : /连接|网络|network|fetch/i.test(message) ? 'network' : 'operation-failed';
  return code ? { category, code } : { category };
};
