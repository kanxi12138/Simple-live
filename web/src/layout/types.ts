export type Platform =
  | 'custom'
  | 'douyu'
  | 'huya'
  | 'douyin'
  | 'bilibili'
  | 'custom-m3u8';

export interface Streamer {
  id: string;
  platform: Platform;
  nickname: string;
  avatar: string;
  title: string;
  isOnline: boolean;
  category: string;
  viewerCount?: string;
  cover?: string;
}

export interface Category {
  id: string;
  name: string;
  platform: Platform | 'all';
}
