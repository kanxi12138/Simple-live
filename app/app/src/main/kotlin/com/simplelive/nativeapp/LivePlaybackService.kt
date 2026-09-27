package com.simplelive.nativeapp

import android.app.PendingIntent
import android.content.ComponentName
import android.content.Intent
import android.os.Bundle
import android.os.SystemClock
import androidx.core.content.ContextCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.hls.HlsMediaSource
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaController
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit

data class PlaybackStatus(val player: Player?=null,val playing: Boolean=false,val landscape: Boolean=false,val audioOnly: Boolean=false,val error: String="",val audioStatus: String="")

// Application-scoped connection keeps the session bound when the Activity is recreated.
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
class PlaybackConnection(private val app: LiveApplication) {
    val state=MutableStateFlow(PlayerState())
    val status=MutableStateFlow(PlaybackStatus())
    var service: LivePlaybackService?=null
        private set
    private var controller: ListenableFuture<MediaController>?=null
    private var pending: Playback?=null

    fun attach(value: LivePlaybackService) { service=value }

    fun reset() {
        pending=null
        service?.resetPlayback()
        status.value=PlaybackStatus(player=service?.player)
    }

    fun play(playback: Playback) {
        pending=playback
        if(controller?.isDone==true && service!=null) {
            service?.play(playback)
            return
        }
        if(controller!=null) return
        val future=MediaController.Builder(app,SessionToken(app,ComponentName(app,LivePlaybackService::class.java))).buildAsync()
        controller=future
        future.addListener({
            if(controller===future) {
                try {
                    future.get()
                    pending?.let { service?.play(it) }
                } catch(error: ExecutionException) {
                    val failedState=state.value
                    stop()
                    state.value=failedState.copy(playback=null,loading=false,error="无法连接播放服务，请重新加载直播",revision=state.value.revision)
                }
            }
        },ContextCompat.getMainExecutor(app))
    }

    fun stop() {
        pending=null
        service?.resetPlayback()
        service?.pauseAllPlayersAndStopSelf()
        service=null
        controller?.let { MediaController.releaseFuture(it) }
        controller=null
        status.value=PlaybackStatus()
        state.value=PlayerState(revision=state.value.revision+1)
    }

    fun detached(value: LivePlaybackService) {
        if(service===value) {
            service=null
            stop()
        }
    }
}

@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
class LivePlaybackService : MediaSessionService() {
    private val app get()=application as LiveApplication
    private val scope=CoroutineScope(SupervisorJob()+Dispatchers.Main.immediate)
    private var session: MediaSession?=null
    lateinit var player: ExoPlayer
        private set
    private val stopCommand=SessionCommand("stop_live",Bundle.EMPTY)
    private var currentPlayback: Playback?=null
    private var streamAccess: DouyinStreamAccess?=null
    private var playbackGeneration=0
    private var audioMonitor: Job?=null
    private var roomRefresh: Job?=null
    private var audioStarted=false
    private var audioError=""
    private var audioStatus=""
    private var lastAudioBuffers=0
    private var lastAudioPosition=0L
    private var lastAudioProgress=0L

    override fun onCreate() {
        super.onCreate()
        player=ExoPlayer.Builder(this).build().apply {
            setAudioAttributes(AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build(),true)
            setHandleAudioBecomingNoisy(true)
            setWakeMode(C.WAKE_MODE_NETWORK)
            addListener(object: Player.Listener {
                override fun onEvents(player: Player,events: Player.Events) { publishStatus() }
            })
        }
        player.addAnalyticsListener(object: AnalyticsListener {
            override fun onAudioPositionAdvancing(eventTime: AnalyticsListener.EventTime,playoutStartSystemTimeMs: Long) {
                audioStarted=true
            }
            override fun onAudioSinkError(eventTime: AnalyticsListener.EventTime,audioSinkError: Exception) {
                audioError="音频输出失败，请刷新或切换线路";publishStatus()
            }
            override fun onAudioCodecError(eventTime: AnalyticsListener.EventTime,audioCodecError: Exception) {
                audioError="音频解码失败，请刷新或切换线路";publishStatus()
            }
        })
        val activity=PendingIntent.getActivity(this,0,Intent(this,MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP),PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        session=MediaSession.Builder(this,player).setSessionActivity(activity)
            .setCallback(object: MediaSession.Callback {
                override fun onConnect(session: MediaSession,controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
                    return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                        .setAvailableSessionCommands(MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon().add(stopCommand).build())
                        .setAvailablePlayerCommands(Player.Commands.Builder().addAll(Player.COMMAND_PLAY_PAUSE,Player.COMMAND_STOP,Player.COMMAND_GET_CURRENT_MEDIA_ITEM,Player.COMMAND_GET_TIMELINE,Player.COMMAND_GET_METADATA).build())
                        .build()
                }
                override fun onCustomCommand(session: MediaSession,controller: MediaSession.ControllerInfo,customCommand: SessionCommand,args: Bundle): ListenableFuture<SessionResult> {
                    if(customCommand==stopCommand) {
                        app.playback.stop()
                        return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_ERROR_NOT_SUPPORTED))
                }
                override fun onPlayerCommandRequest(session: MediaSession,controller: MediaSession.ControllerInfo,playerCommand: Int): Int {
                    if(playerCommand==Player.COMMAND_STOP) {
                        android.os.Handler(mainLooper).post { app.playback.stop() }
                    }
                    return SessionResult.RESULT_SUCCESS
                }
            })
            .setMediaButtonPreferences(listOf(CommandButton.Builder(CommandButton.ICON_STOP).setDisplayName("停止直播").setSessionCommand(stopCommand).build()))
            .build()
        app.playback.attach(this)
        scope.launch {
            app.storage.preferences.collect { preferences ->
                player.volume=preferences["dtv_player_volume_v1"]?.toFloatOrNull()?.coerceIn(0f,1f) ?: 1f
            }
        }
    }

    fun play(playback: Playback) {
        resetPlayback()
        currentPlayback=playback
        val generation=playbackGeneration
        setAudioMode(playback.room.isAudioLive)
        val access=if(playback.room.platform==Platform.DOUYIN) DouyinStreamAccess(playback.url) else null
        streamAccess=access
        val clientBuilder=app.network.client.newBuilder().callTimeout(0,TimeUnit.MILLISECONDS)
        if(access!=null) {
            clientBuilder.followRedirects(false).followSslRedirects(false).dns(access.dns).addInterceptor(access)
        } else clientBuilder.followRedirects(true).followSslRedirects(true).addNetworkInterceptor { chain ->
                validateStream(playback.room.platform,chain.request().url.toString())
                chain.proceed(chain.request().newBuilder().removeHeader("Cookie").removeHeader("Authorization").build())
            }
        val client=clientBuilder.build()
        val factory=OkHttpDataSource.Factory(client).setDefaultRequestProperties(playback.headers)
        val item=MediaItem.Builder().setUri(playback.url).setMediaId(playback.room.key)
            .setMediaMetadata(MediaMetadata.Builder().setTitle(playback.room.name).setArtist(playback.room.title).build()).build()
        val source=if(access!=null && playback.line.substringBefore(':')=="hls") {
            HlsMediaSource.Factory(factory).setPlaylistParserFactory(access).createMediaSource(item)
        } else DefaultMediaSourceFactory(factory).createMediaSource(item)
        player.setMediaSource(source)
        player.prepare()
        player.play()
        lastAudioProgress=SystemClock.elapsedRealtime()
        audioMonitor=scope.launch {
            while(isActive && generation==playbackGeneration) {
                monitorAudio()
                delay(1000)
            }
        }
        if(playback.room.platform==Platform.DOUYIN && playback.room.isAudioLive) roomRefresh=scope.launch {
            while(isActive && generation==playbackGeneration) {
                delay(30_000)
                if(!player.playWhenReady) continue
                try {
                    val updated=app.platforms.getValue(Platform.DOUYIN).detail(playback.room)
                    if(generation!=playbackGeneration) return@launch
                    if(updated.status!=LiveStatus.LIVE || !updated.extra.containsKey("live_type_audio")) continue
                    currentPlayback=currentPlayback?.copy(room=updated)
                    setAudioMode(updated.isAudioLive)
                    app.playback.state.update { state ->
                        if(state.playback?.room?.key==playback.room.key) state.copy(room=updated,playback=currentPlayback) else state
                    }
                    publishStatus()
                } catch(error: CancellationException) { throw error }
                catch(error: Exception) {
                    // Metadata refresh must not interrupt an already playing stream.
                    android.util.Log.w("LivePlayback","房间类型刷新失败：${error.javaClass.simpleName}")
                }
            }
        }
    }

    private fun setAudioMode(audioOnly: Boolean) {
        player.trackSelectionParameters=player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_VIDEO,audioOnly)
            .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO,false).build()
    }

    fun resetPlayback() {
        playbackGeneration++
        streamAccess?.close();streamAccess=null
        audioMonitor?.cancel();audioMonitor=null
        roomRefresh?.cancel();roomRefresh=null
        currentPlayback=null
        audioStarted=false;audioError="";audioStatus=""
        lastAudioBuffers=0;lastAudioPosition=0L;lastAudioProgress=0L
        player.stop();player.clearMediaItems()
    }

    private fun monitorAudio() {
        val now=SystemClock.elapsedRealtime()
        val counters=player.audioDecoderCounters
        counters?.ensureUpdated()
        val buffers=counters?.renderedOutputBufferCount ?: 0
        val position=player.currentPosition
        val advancing=audioStarted && position>lastAudioPosition && (counters==null || buffers>lastAudioBuffers)
        val active=player.isPlaying && player.currentTracks.isTypeSelected(C.TRACK_TYPE_AUDIO)
        if(!active || advancing) lastAudioProgress=now
        if(advancing) audioError=""
        audioStatus=when {
            audioError.isNotBlank() || player.playerError!=null -> ""
            !player.playWhenReady -> "音频已暂停"
            player.playbackSuppressionReason!=Player.PLAYBACK_SUPPRESSION_REASON_NONE -> "音频播放暂时中断"
            player.playbackState==Player.STATE_ENDED -> "音频播放已结束"
            player.playbackState!=Player.STATE_READY -> "正在加载音频…"
            !player.currentTracks.isTypeSelected(C.TRACK_TYPE_AUDIO) -> "未检测到可播放音频，请刷新或切换线路"
            now-lastAudioProgress>=10_000 -> "暂未检测到音频输出，请刷新或切换线路"
            advancing -> "正在播放音频"
            else -> "等待音频输出…"
        }
        lastAudioBuffers=buffers;lastAudioPosition=position
        publishStatus()
    }

    private fun publishStatus() {
        if(app.playback.service!==this) return
        val tracks=player.currentTracks
        val hasVideo=tracks.groups.any { it.type==C.TRACK_TYPE_VIDEO }
        val hasAudio=tracks.isTypeSelected(C.TRACK_TYPE_AUDIO)
        val size=player.videoSize
        val error=player.playerError
        val audioOnly=currentPlayback?.room?.isAudioLive==true || (hasAudio && !hasVideo && player.playbackState==Player.STATE_READY && error==null)
        val message=if(error==null) audioError else when {
            generateSequence<Throwable>(error) { it.cause }.any { it is StreamAddressException } -> "播放地址不受支持，请切换画质或线路"
            error.errorCode in 4000..4999 -> "设备无法解码当前画质，请切换画质或线路"
            error.errorCode in 2000..2999 -> "播放网络请求失败，请刷新或切换线路"
            else -> "播放失败（${error.errorCodeName}），可刷新或切换线路"
        }
        app.playback.status.value=PlaybackStatus(player,player.playWhenReady,
            !audioOnly && hasVideo && size.width>0 && size.height>0 && size.width*size.pixelWidthHeightRatio>=size.height,
            audioOnly,message,if(audioOnly) audioStatus else "")
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession?=session

    override fun onStartCommand(intent: Intent?,flags: Int,startId: Int): Int {
        super.onStartCommand(intent,flags,startId)
        return START_NOT_STICKY
    }

    override fun onTaskRemoved(rootIntent: Intent?) { app.playback.stop() }

    override fun onDestroy() {
        streamAccess?.close();streamAccess=null
        scope.cancel()
        app.playback.detached(this)
        session?.release();session=null
        player.release()
        super.onDestroy()
    }
}
