import { invoke } from '../../services/platformInvoke';
import type { FollowedStreamer, LiveStatus } from '../common/types';

// This interface should match the DouyuFollowInfo struct returned by Rust
interface DouyuFollowRoomInfo {
  room_id: string;      
  room_name?: string | null; 
  nickname?: string | null;
  avatar_url?: string | null;
  video_loop?: number | null; // Rust i64 maps to number in TS
  show_status?: number | null; // Changed to number
}

// Define what this function returns - it's a partial update for FollowedStreamer
// focusing on the fields this function is responsible for.
interface DouyuRefreshUpdate extends Partial<Omit<FollowedStreamer, 'liveStatus'>> {
  liveStatus: LiveStatus; // Use the common LiveStatus type
}

export async function refreshDouyuFollowedStreamer(
  streamer: FollowedStreamer
): Promise<DouyuRefreshUpdate> {
  try {
    const roomInfo = await invoke<DouyuFollowRoomInfo>('fetch_douyu_room_info', {
      roomId: streamer.id,
    });

    if (roomInfo && roomInfo.room_id) {
      let currentLiveStatus: LiveStatus = 'OFFLINE'; // Default to OFFLINE

      const sStatus = typeof roomInfo.show_status === 'number' ? roomInfo.show_status : null;
      const vLoop = typeof roomInfo.video_loop === 'number' ? roomInfo.video_loop : null;

      if (sStatus === 1) {
        if (vLoop === 1) {
          currentLiveStatus = 'REPLAY';
        } else if (vLoop === 0 || vLoop === null) { 
          currentLiveStatus = 'LIVE';
        } else {
          currentLiveStatus = 'OFFLINE'; 
          console.warn('Diagnostic: followListHelper.ts:41 (details omitted)');
        }
      } else { // Any show_status other than 1 (e.g., 2 or null/missing)
        currentLiveStatus = 'OFFLINE';
      }
      
      console.log('Diagnostic: followListHelper.ts:47 (details omitted)');

      return {
        liveStatus: currentLiveStatus,
        nickname: roomInfo.nickname ?? streamer.nickname,
        roomTitle: roomInfo.room_name ?? streamer.roomTitle,
        avatarUrl: roomInfo.avatar_url ?? streamer.avatarUrl,
      };
    } else { 
      console.warn('Diagnostic: followListHelper.ts:58 (details omitted)');
      return { liveStatus: 'OFFLINE' }; 
    }
  } catch (e: any) { 
    console.error('Diagnostic: followListHelper.ts:65 (details omitted)');
    return { liveStatus: 'UNKNOWN' }; // Or 'OFFLINE' - UNKNOWN signals an error state more clearly
  }
} 
