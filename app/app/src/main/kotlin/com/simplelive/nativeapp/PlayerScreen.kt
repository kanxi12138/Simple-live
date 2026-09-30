@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.simplelive.nativeapp

import android.view.WindowManager
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.delay
import org.json.JSONArray
import org.json.JSONObject

data class DanmakuSettings(val enabled: Boolean=true,val fontSize: Float=20f,val duration: Float=10000f,val area: Float=0.5f,val opacity: Float=1f,val mode: String="scroll",val density: String="medium") {
    fun json(): String = JSONObject().put("enabled",enabled).put("settings",JSONObject().put("fontSize","${fontSize.toInt()}px").put("duration",duration).put("area",area).put("opacity",opacity).put("mode",mode).put("density",density)).toString()
    companion object {
        fun from(raw: String?): DanmakuSettings {
            if(raw==null) return DanmakuSettings()
            val value=JSONObject(raw);val settings=value.obj("settings")
            return DanmakuSettings(value.optBoolean("enabled",true),settings.str("fontSize","20px").removeSuffix("px").toFloat().coerceIn(10f,48f),settings.optDouble("duration",10000.0).toFloat().coerceIn(3000f,30000f),settings.optDouble("area",0.5).toFloat(),settings.optDouble("opacity",1.0).toFloat(),settings.str("mode","scroll"),settings.str("density","medium"))
        }
    }
}
@Composable
fun PlayerScreen(model: LiveViewModel,onFullscreen: (Boolean,Boolean)->Unit,onLogin: (Platform)->Unit) {
    val state by model.player.collectAsStateWithLifecycle()
    val preferences by model.preferences.collectAsStateWithLifecycle()
    val library by model.library.collectAsStateWithLifecycle()
    val room=state.room ?: return
    val playbackStatus by model.app.playback.status.collectAsStateWithLifecycle()
    val audioOnly=room.isAudioLive || playbackStatus.audioOnly
    var fullscreen by remember { mutableStateOf(false) }
    var landscape by remember(room.key) { mutableStateOf(false) }
    var controlsVisible by remember { mutableStateOf(true) }
    var interaction by remember { mutableIntStateOf(0) }
    val activePlayer=playbackStatus.player
    val playing=playbackStatus.playing
    val playbackFailed=playbackStatus.error.isNotBlank()
    var panel by remember { mutableStateOf("") }
    var details by remember { mutableStateOf(false) }
    val settings=remember(preferences["dtv_danmu_preferences_v1"]) { DanmakuSettings.from(preferences["dtv_danmu_preferences_v1"]) }
    val words=remember(preferences["danmu_block_keywords"]) { JSONArray(preferences["danmu_block_keywords"] ?: "[]").strings() }
    val messages=remember(state.messages,words) { state.messages.filter { message -> words.none { word -> word.isNotBlank() && message.text.contains(word,true) } } }
    fun leave() { onFullscreen(false,false); fullscreen=false;model.closePlayer() }
    BackHandler { if(panel.isNotBlank()) { panel="";interaction++ } else if(fullscreen) { fullscreen=false } else leave() }
    DisposableEffect(Unit) { onDispose { onFullscreen(false,false) } }
    LaunchedEffect(fullscreen,playbackStatus.landscape,audioOnly,room.key) {
        // Reloading clears video dimensions; keep the room's last confirmed orientation.
        if(audioOnly) landscape=false else playbackStatus.landscape?.let { landscape=it }
        onFullscreen(fullscreen,landscape)
    }
    LaunchedEffect(playing,playbackFailed,state.error) {
        if(!playing || playbackFailed || state.error.isNotBlank()) controlsVisible=true
    }
    LaunchedEffect(controlsVisible,panel,playing,playbackFailed,state.loading,state.error,interaction) {
        if(controlsVisible && panel.isBlank() && playing && !playbackFailed && !state.loading && state.error.isBlank()) {
            delay(3000);controlsVisible=false
        }
    }
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize().background(if(fullscreen) Color.Black else MaterialTheme.colorScheme.background).then(if(!fullscreen) Modifier.statusBarsPadding().navigationBarsPadding() else Modifier)) {
            if(!fullscreen) Row(Modifier.fillMaxWidth(),verticalAlignment=Alignment.CenterVertically) {
                IconButton(onClick=::leave){Icon(Icons.Outlined.ArrowBack,"返回")}
                Text(room.platform.label,Modifier.weight(1f),fontWeight=FontWeight.SemiBold)
                IconButton(onClick={details=true}){Icon(Icons.Outlined.Info,"房间详情")}
            }
            Box(if(fullscreen) Modifier.weight(1f).fillMaxWidth() else Modifier.fillMaxWidth().aspectRatio(16f/9f)) {
                NativeVideo(playbackStatus.copy(audioOnly=audioOnly),Modifier.fillMaxSize())
                if(state.playback!=null && settings.enabled) key(state.revision) { DanmakuOverlay(model,state.revision,settings,words,Modifier.fillMaxSize()) }
                Box(Modifier.fillMaxSize().clickable { controlsVisible=!controlsVisible;interaction++ })
                if(state.loading) CircularProgressIndicator(Modifier.align(Alignment.Center),color=Color.White)
                if(state.error.isNotBlank()) Column(Modifier.align(Alignment.Center).padding(20.dp),horizontalAlignment=Alignment.CenterHorizontally) {
                    Text(state.error,color=Color.White)
                    TextButton(onClick={model.open(room)}){Text("重新加载",color=Color(0xff91baff))}
                    if(state.needsLogin) TextButton(onClick={onLogin(room.platform)}){Text("登录平台",color=Color(0xff91baff))}
                }
                if(controlsVisible && panel.isBlank() && state.playback!=null && !playbackFailed) IconButton(
                onClick={if(playing) activePlayer?.pause() else activePlayer?.play();interaction++},
                modifier=Modifier.align(Alignment.Center).size(56.dp).background(Color(0x88000000),androidx.compose.foundation.shape.CircleShape)
                ){Icon(if(playing) Icons.Outlined.Pause else Icons.Outlined.PlayArrow,if(playing) "暂停" else "播放",tint=Color.White)}
                if(controlsVisible && panel.isBlank()) Surface(modifier=Modifier.align(Alignment.BottomCenter),color=Color(0xbb101a2a),contentColor=Color.White) {
                    Row(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal+WindowInsetsSides.Bottom)).padding(horizontal=4.dp),verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.SpaceBetween) {
                        RefreshButton(state.loading,"重新加载直播",{model.open(room)})
                        TextButton(onClick={panel="quality"},enabled=state.playback!=null){Text(state.playback?.qualities?.firstOrNull { it.id==state.playback?.quality }?.name ?: "画质",maxLines=1)}
                        TextButton(onClick={panel="line"},enabled=state.playback!=null){Text("线路",maxLines=1)}
                        IconButton(onClick={panel="danmaku"}){Icon(Icons.Outlined.Forum,"弹幕设置")}
                        IconButton(onClick={panel="volume"}){Icon(Icons.Outlined.VolumeUp,"音量")}
                        IconButton(onClick={fullscreen=!fullscreen;interaction++}){Icon(if(fullscreen) Icons.Outlined.FullscreenExit else Icons.Outlined.Fullscreen,if(fullscreen) "退出全屏" else "全屏")}
                    }
                }
            }
            if(!fullscreen) {
                Row(Modifier.fillMaxWidth().padding(horizontal=20.dp,vertical=14.dp),verticalAlignment=Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) { Text(room.name,fontWeight=FontWeight.Bold,fontSize=20.sp,maxLines=1,overflow=TextOverflow.Ellipsis); Text(room.title,style=MaterialTheme.typography.bodyMedium,maxLines=2,overflow=TextOverflow.Ellipsis,color=MaterialTheme.colorScheme.onSurfaceVariant) }
                    FilledTonalButton(onClick={model.toggleFollow(room)}){Text(if(library.follows.any { it.room.key==room.key }) "已关注" else "关注")}
                }
                Row(Modifier.padding(horizontal=16.dp),verticalAlignment=Alignment.CenterVertically) {
                    Text(state.danmakuStatus.ifBlank { "弹幕" },Modifier.weight(1f),style=MaterialTheme.typography.bodySmall,color=MaterialTheme.colorScheme.onSurfaceVariant)
                    if(room.platform==Platform.DOUYIN) TextButton(onClick=model::reconnectDanmaku,enabled=state.playback!=null){Text("重连弹幕")}
                    TextButton(onClick={model.setPreference("dtv_player_danmu_collapsed",(!(preferences["dtv_player_danmu_collapsed"]=="true")).toString())}){Text(if(preferences["dtv_player_danmu_collapsed"]=="true") "展开" else "收起")}
                }
                if(preferences["dtv_player_danmu_collapsed"]!="true") LazyColumn(Modifier.weight(1f).fillMaxWidth().padding(horizontal=20.dp),reverseLayout=true,contentPadding=PaddingValues(bottom=12.dp)) {
                    items(messages.asReversed(),key={it.id}) { message -> Row(Modifier.padding(vertical=5.dp)) { Text(message.user+"  ",color=MaterialTheme.colorScheme.primary,style=MaterialTheme.typography.bodySmall);Text(message.text,style=MaterialTheme.typography.bodyMedium) } }
                    if(messages.isEmpty()) item { Text("等待直播间弹幕",Modifier.padding(vertical=20.dp),color=MaterialTheme.colorScheme.onSurfaceVariant) }
                }
            }
        }
        if(panel.isNotBlank()) {
            Box(Modifier.fillMaxSize().background(Color(0x66000000)).clickable { panel="";interaction++ })
            Surface(modifier=Modifier.align(if(fullscreen) Alignment.CenterEnd else Alignment.BottomCenter)
            .then(if(fullscreen) Modifier.widthIn(max=380.dp).fillMaxHeight() else Modifier.fillMaxWidth().fillMaxHeight(0.85f))
            .clickable { },tonalElevation=6.dp) {
                Column(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.safeDrawing).verticalScroll(rememberScrollState()).padding(horizontal=20.dp)) {
                    TextButton(onClick={panel="";interaction++},modifier=Modifier.align(Alignment.End)){Text("关闭") }
                    when(panel) {
                        "quality","line" -> {
                            Text(if(panel=="quality") "选择画质" else "选择线路",style=MaterialTheme.typography.headlineSmall)
                            val options=if(panel=="quality") state.playback?.qualities else state.playback?.lines
                            options.orEmpty().forEach { option -> TextButton(onClick={model.open(room,if(panel=="quality") option.id else state.playback?.quality,if(panel=="line") option.id else state.playback?.line);panel=""},modifier=Modifier.fillMaxWidth()){Text(option.name)} }
                        }
                        "volume" -> {
                            Text("播放音量",style=MaterialTheme.typography.headlineSmall)
                            var volume by remember { mutableFloatStateOf(preferences["dtv_player_volume_v1"]?.toFloatOrNull() ?: 1f) }
                            Slider(volume,{volume=it},onValueChangeFinished={model.setPreference("dtv_player_volume_v1",volume.toString())})
                        }
                        else -> {
                            if(room.platform==Platform.DOUYIN) {
                                Text(state.danmakuStatus.ifBlank { "弹幕" })
                                TextButton(onClick=model::reconnectDanmaku,enabled=state.playback!=null){Text("重连弹幕")}
                            }
                            DanmakuSettingsPanel(settings,words,{model.setPreference("dtv_danmu_preferences_v1",it.json())},{model.setPreference("danmu_block_keywords",JSONArray(it).toString())})
                        }
                    }
                    Spacer(Modifier.height(32.dp))
                }
            }
        }
    }
    if(details) AlertDialog(onDismissRequest={details=false},title={Text("房间详情")},text={Text("平台：${room.platform.label}\n主播：${room.name}\n房间号：${room.id}\n${room.title}")},confirmButton={TextButton(onClick={details=false}){Text("关闭")}})

}
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
@Composable
private fun NativeVideo(status: PlaybackStatus,modifier: Modifier) {
    val context=LocalContext.current
    DisposableEffect(context) {
        val activity=context as? MainActivity
        activity?.window?.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        onDispose { activity?.window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON) }
    }
    Box(modifier.background(Color.Black)) {
        AndroidView(factory={PlayerView(it).apply { useController=false }},update={it.player=status.player},onRelease={it.player=null},modifier=Modifier.fillMaxSize())
        if(status.audioOnly) Column(Modifier.align(Alignment.TopCenter).padding(horizontal=16.dp,vertical=12.dp),horizontalAlignment=Alignment.CenterHorizontally) {
            Text("该直播间为音频直播无画面",color=Color.White)
            if(status.audioStatus.isNotBlank()) Text(status.audioStatus,color=Color.LightGray,fontSize=12.sp)
        }
        if(status.error.isNotBlank()) Column(Modifier.align(Alignment.Center).background(Color(0xcc000000)).padding(16.dp)) {
            Text(status.error,color=Color.White)
        }
    }
}
@Composable
fun DanmakuSettingsPanel(initial: DanmakuSettings,blocked: List<String>,onSettings: (DanmakuSettings)->Unit,onBlocked: (List<String>)->Unit) {
    var settings by remember(initial) { mutableStateOf(initial) }
    var keywords by remember(blocked) { mutableStateOf(blocked.joinToString("\n")) }
    Text("弹幕设置",style=MaterialTheme.typography.headlineSmall)
    Row(verticalAlignment=Alignment.CenterVertically) { Text("显示屏幕弹幕",Modifier.weight(1f));Switch(settings.enabled,{settings=settings.copy(enabled=it);onSettings(settings)}) }
    Text("字号 ${settings.fontSize.toInt()}");Slider(settings.fontSize,{settings=settings.copy(fontSize=it)},valueRange=12f..36f,onValueChangeFinished={onSettings(settings)})
    Text("透明度 ${(settings.opacity*100).toInt()}%");Slider(settings.opacity,{settings=settings.copy(opacity=it)},valueRange=0.2f..1f,onValueChangeFinished={onSettings(settings)})
    Text("显示区域 ${(settings.area*100).toInt()}%");Slider(settings.area,{settings=settings.copy(area=it)},valueRange=0.25f..1f,onValueChangeFinished={onSettings(settings)})
    Text("通过屏幕时间 ${(settings.duration/1000).toInt()} 秒");Slider(settings.duration,{settings=settings.copy(duration=it)},valueRange=4000f..20000f,onValueChangeFinished={onSettings(settings)})
    Row(horizontalArrangement=Arrangement.spacedBy(8.dp)) { listOf("scroll" to "滚动","top" to "顶部","bottom" to "底部").forEach { (value,label)->FilterChip(settings.mode==value,{settings=settings.copy(mode=value);onSettings(settings)},label={Text(label)}) } }
    Row(horizontalArrangement=Arrangement.spacedBy(8.dp)) { listOf("sparse" to "稀疏","medium" to "适中","dense" to "密集").forEach { (value,label)->FilterChip(settings.density==value,{settings=settings.copy(density=value);onSettings(settings)},label={Text(label)}) } }
    OutlinedTextField(keywords,{keywords=it},label={Text("屏蔽词，每行一个")},modifier=Modifier.fillMaxWidth(),minLines=3)
    TextButton(onClick={onBlocked(keywords.lines().map { it.trim() }.filter { it.isNotBlank() }.distinct())}){Text("保存屏蔽词")}
}
