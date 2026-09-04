package www.sp.com

import android.content.Intent
import android.util.Log
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionParameters
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

internal const val ACTION_PLAY_BACKGROUND = "www.sp.com.action.PLAY_BACKGROUND"
internal const val EXTRA_STREAM_URL = "stream_url"
internal const val EXTRA_STREAM_TYPE = "stream_type"
internal const val EXTRA_TITLE = "title"
internal const val EXTRA_ANCHOR_NAME = "anchor_name"
private const val HLS_MIME_TYPE = "application/x-mpegURL"
private const val FLV_MIME_TYPE = "video/x-flv"

/**
 * Hosts audio-only live-stream playback while the application is backgrounded.
 */
class BackgroundPlaybackService : MediaSessionService() {
  private var exoPlayer: ExoPlayer? = null
  private var mediaSession: MediaSession? = null

  /**
   * Creates the ExoPlayer and media session used by Android system controls.
   */
  override fun onCreate() {
    super.onCreate()
    val audioAttributes = AudioAttributes.Builder()
      .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
      .setUsage(C.USAGE_MEDIA)
      .build()
    val player = ExoPlayer.Builder(this)
      .setAudioAttributes(audioAttributes, true)
      .setWakeMode(C.WAKE_MODE_NETWORK)
      .build()
    player.trackSelectionParameters = TrackSelectionParameters.Builder(this)
      .setTrackTypeDisabled(C.TRACK_TYPE_VIDEO, true)
      .build()
    player.addListener(object : Player.Listener {
      override fun onPlayerError(error: PlaybackException) {
        Log.e("BackgroundPlayback", "Playback failed", error)
        stopSelf()
      }
    })
    exoPlayer = player
    mediaSession = MediaSession.Builder(this, player).build()
  }

  /**
   * Starts playback when the activity hands its current stream to the service.
   *
   * @param intent Intent containing the stream and media metadata.
   * @param flags Android service restart flags.
   * @param startId Identifier for this service start request.
   * @return The restart mode selected by MediaSessionService.
   */
  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action == ACTION_PLAY_BACKGROUND) {
      playStream(intent)
    }
    return super.onStartCommand(intent, flags, startId)
  }

  /**
   * Returns the active media session to authorized system controllers.
   *
   * @param controllerInfo Information about the requesting controller.
   * @return The service media session.
   */
  override fun onGetSession(
    controllerInfo: MediaSession.ControllerInfo,
  ): MediaSession? = mediaSession

  /**
   * Stops background audio when the application task is dismissed.
   *
   * @param rootIntent Intent that originally launched the removed task.
   */
  override fun onTaskRemoved(rootIntent: Intent?) {
    exoPlayer?.stop()
    stopSelf()
    super.onTaskRemoved(rootIntent)
  }

  /**
   * Releases all native playback resources.
   */
  override fun onDestroy() {
    mediaSession?.release()
    mediaSession = null
    exoPlayer?.release()
    exoPlayer = null
    super.onDestroy()
  }

  private fun playStream(intent: Intent) {
    val streamUrl = intent.getStringExtra(EXTRA_STREAM_URL)?.trim().orEmpty()
    if (streamUrl.isEmpty()) {
      Log.e("BackgroundPlayback", "Cannot play an empty stream URL")
      stopSelf()
      return
    }
    val streamType = intent.getStringExtra(EXTRA_STREAM_TYPE).orEmpty()
    val title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
    val anchorName = intent.getStringExtra(EXTRA_ANCHOR_NAME).orEmpty()
    val metadata = MediaMetadata.Builder()
      .setTitle(title.ifEmpty { getString(R.string.app_name) })
      .setArtist(anchorName)
      .build()
    val mediaItem = MediaItem.Builder()
      .setUri(streamUrl)
      .setMimeType(if (streamType == "hls") HLS_MIME_TYPE else FLV_MIME_TYPE)
      .setMediaMetadata(metadata)
      .build()
    exoPlayer?.apply {
      setMediaItem(mediaItem)
      prepare()
      play()
    }
  }
}
