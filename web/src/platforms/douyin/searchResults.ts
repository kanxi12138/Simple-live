import { Platform } from '../common/types';

/** Shared display result; account metadata is optional for other platforms. */
export interface SearchResultItem {
  platform: Platform;
  roomId: string;
  webId?: string | null;
  userName: string;
  roomTitle?: string | null;
  avatar: string | null;
  liveStatus: boolean;
  rawStatus?: number | null;
  accountId?: string | null;
  secUid?: string | null;
  douyinId?: string | null;
  followers?: number | null;
  roomAliases?: string[];
  kind?: '账号' | '直播间' | '账号·直播间';
}

/** Official user-search result; account IDs are never playback IDs. */
export interface DouyinAccountItem {
  account_id: string;
  sec_uid: string | null;
  douyin_id: string | null;
  nickname: string;
  avatar: string;
  follower_count: number | null;
  live_status: 'live' | 'offline' | 'unknown';
  room_id: string | null;
  web_rid: string | null;
}

/** Maps official account metadata to a display row without fabricating a room. */
export const mapAccount = (account: DouyinAccountItem): SearchResultItem => ({
  platform: Platform.DOUYIN,
  roomId: account.web_rid || account.room_id || '',
  webId: account.web_rid,
  userName: account.nickname || '抖音账号',
  avatar: account.avatar || null,
  accountId: account.account_id,
  secUid: account.sec_uid,
  douyinId: account.douyin_id,
  followers: account.follower_count,
  liveStatus: account.live_status === 'live',
  rawStatus: account.live_status === 'unknown' ? null : account.live_status === 'live' ? 2 : 4,
  roomAliases: [account.web_rid, account.room_id].filter((value): value is string => !!value && value !== '0'),
  kind: '账号',
});

const identityKeys = (item: SearchResultItem): string[] => [
  ...(item.accountId ? [`user:${item.accountId}`] : []),
  ...(item.secUid ? [`sec:${item.secUid}`] : []),
  ...[item.roomId, item.webId, ...(item.roomAliases || [])]
    .filter((value): value is string => !!value && value !== '0')
    .map((value) => `room:${value}`),
];

const combine = (first: SearchResultItem, next: SearchResultItem): SearchResultItem => {
  const playable = next.liveStatus && next.roomId ? next : first;
  return {
    ...first,
    roomId: playable.roomId,
    webId: playable.webId,
    liveStatus: playable.liveStatus,
    rawStatus: playable.rawStatus ?? next.rawStatus,
    roomTitle: first.roomTitle || next.roomTitle,
    accountId: first.accountId || next.accountId,
    secUid: first.secUid || next.secUid,
    douyinId: first.douyinId || next.douyinId,
    followers: first.followers ?? next.followers,
    roomAliases: [...new Set([...identityKeys(first), ...identityKeys(next)]
      .filter((key) => key.startsWith('room:')).map((key) => key.slice(5)))],
    kind: first.kind === next.kind ? first.kind : '账号·直播间',
  };
};

/** Merges proven identities and stably sorts known follower counts descending. */
export const mergeDouyinResults = (
  accounts: SearchResultItem[], rooms: SearchResultItem[],
): SearchResultItem[] => {
  const merged: SearchResultItem[] = [];
  [...accounts, ...rooms].forEach((item) => {
    const matches = new Set(identityKeys(item));
    let combined = item;
    let insertionIndex = merged.length;
    for (let i = merged.length - 1; i >= 0; i -= 1) {
      if (identityKeys(merged[i]).some((key) => matches.has(key))) {
        combined = combine(merged[i], combined);
        identityKeys(combined).forEach((key) => matches.add(key));
        insertionIndex = i;
        merged.splice(i, 1);
      }
    }
    merged.splice(insertionIndex, 0, combined);
  });
  return merged.sort((first, next) => (next.followers ?? -1) - (first.followers ?? -1));
};

/** Formats only actual follower numbers; unknown counts are explicit. */
export const followerLabel = (count: number | null | undefined): string => (
  count == null ? '粉丝数未知' : `${count.toLocaleString('zh-CN')} 粉丝`
);

/** Describes playback availability without treating unknown status as offline. */
export const statusLabel = (item: SearchResultItem): string => {
  if (item.liveStatus) return item.roomId ? '直播中' : '直播中，暂无房间号';
  return item.rawStatus == null ? '状态未知' : '未开播';
};
