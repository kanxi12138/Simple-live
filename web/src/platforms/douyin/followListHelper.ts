import { invoke } from '../../services/platformInvoke';
import type { FollowedStreamer, LiveStreamInfo, LiveStatus } from '../common/types';

export async function refreshDouyinFollowedStreamer(
  streamer: FollowedStreamer
): Promise<Partial<FollowedStreamer>> {
  try {
    // The payload for 'get_douyin_live_stream_url' expects { payload: { args: { room_id_str: string } } }
    const payloadData = { args: { room_id_str: streamer.id } };
    const data = await invoke<LiveStreamInfo>('fetch_douyin_streamer_info', {
      payload: payloadData,
    });

    // Check if data is valid and there are no errors from the backend
    if (data && !data.error_message) {
      const isLive = data.status === 2;
      const liveStatus: LiveStatus = isLive ? 'LIVE' : 'OFFLINE';
      const nextId = data.web_rid || streamer.id;

      if (data.web_rid && data.web_rid !== streamer.id) {
        console.info('Diagnostic: followListHelper.ts:21 (details omitted)');
      }

      return {
        id: nextId,
        isLive,
        liveStatus,
        nickname: data.anchor_name || streamer.nickname,
        roomTitle: data.title || streamer.roomTitle,
        avatarUrl: data.avatar || streamer.avatarUrl,
      };
    } else {
      if (data && data.error_message) {
        console.warn('Diagnostic: followListHelper.ts:34 (details omitted)');
      } else {
        console.warn('Diagnostic: followListHelper.ts:38 (details omitted)');
      }
      return { isLive: false, liveStatus: 'OFFLINE' }; // Ensure these are set on error too
    }
  } catch (e) {
    console.error('Diagnostic: followListHelper.ts:46 (details omitted)');
    return { isLive: false, liveStatus: 'OFFLINE' }; // Ensure these are set on error too
  }
} 
