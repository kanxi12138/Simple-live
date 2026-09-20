package com.simplelive.nativeapp

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import org.json.JSONObject

data class BrowseState(
    val platform: Platform=Platform.DOUYU, val category: Category?=null, val categories: List<Category> = emptyList(),
    val rooms: List<Room> = emptyList(), val loading: Boolean=false, val error: String="", val page: Int=1, val more: Boolean=true,
    val needsLogin: Boolean=false, val verification: BiliListChallenge?=null, val verificationAppend: Boolean=false,
)
data class SearchQuery(val platform: Platform,val keyword: String,val accounts: Boolean)
data class SearchState(val platform: Platform=Platform.DOUYU,val keyword: String="",val accounts: Boolean=false,val rooms: List<Room> = emptyList(),val loading: Boolean=false,val error: String="",val page: Int=1,val more: Boolean=true,val committed: SearchQuery?=null)
data class PlayerState(val room: Room?=null,val playback: Playback?=null,val loading: Boolean=false,val error: String="",val danmakuStatus: String="",val messages: List<Danmaku> = emptyList(),val revision: Int=0,val needsLogin: Boolean=false)

class LiveViewModel(application: Application) : AndroidViewModel(application) {
    val app=application as LiveApplication
    val storage=app.storage
    val preferences=storage.preferences.stateIn(viewModelScope,SharingStarted.Eagerly,emptyMap())
    val library=storage.library.stateIn(viewModelScope,SharingStarted.Eagerly,Library())
    val ready=MutableStateFlow(false)
    val notice=MutableStateFlow("")
    val browse=MutableStateFlow(BrowseState())
    val search=MutableStateFlow(SearchState())
    val player=MutableStateFlow(PlayerState())
    private val updater=Updates(app,app.network)
    private val mutableUpdate=MutableStateFlow(UpdateState(busy=true))
    val update: StateFlow<UpdateState> = mutableUpdate.asStateFlow()
    private var updateJob: Job?=null
    private val screenMessages=MutableSharedFlow<List<Danmaku>>(extraBufferCapacity=8,onBufferOverflow=kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)
    val overlayMessages: SharedFlow<List<Danmaku>> = screenMessages.asSharedFlow()
    private var subcategoryJob: Job?=null
    private var subcategoryGeneration=0
    private var browseJob: Job?=null
    private var categoryJob: Job?=null
    private var searchJob: Job?=null
    private var playerJob: Job?=null
    private var danmakuJob: Job?=null
    @Volatile private var danmakuGeneration=0
    @Volatile private var playerGeneration=0
    private var browseGeneration=0
    private var searchGeneration=0
    private var refreshJob: Job?=null
    private val mutableFollowsRefreshing=MutableStateFlow(false)
    val followsRefreshing: StateFlow<Boolean> = mutableFollowsRefreshing.asStateFlow()
    @Volatile private var foreground=true
    init { operation { storage.recoverImport(); ready.value=true; selectPlatform(Platform.DOUYU) } }
    init {
        updateJob=viewModelScope.launch {
            try { mutableUpdate.value=UpdateState(file=updater.restoredDownload()) }
            catch(error: CancellationException) { throw error }
            catch(error: Exception) { mutableUpdate.value=UpdateState();notice.value=errorText(error) }
        }
    }
    fun checkUpdate() {
        if(updateJob?.isActive==true) return
        mutableUpdate.update { it.copy(busy=true,release=null) }
        updateJob=viewModelScope.launch {
            try {
                val release=updater.latest()
                mutableUpdate.update { it.copy(release=release) }
                if(release==null) notice.value="当前已是最新版本"
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) { notice.value=errorText(error) }
            finally { mutableUpdate.update { it.copy(busy=false) } }
        }
    }
    fun dismissUpdate() { mutableUpdate.update { it.copy(release=null) } }
    fun downloadUpdate() {
        if(updateJob?.isActive==true) return
        val release=update.value.release ?: return
        mutableUpdate.update { it.copy(busy=true,downloading=true,progress=0,release=null) }
        updateJob=viewModelScope.launch {
            try {
                val file=updater.download(release) { progress -> mutableUpdate.update { it.copy(progress=progress) } }
                mutableUpdate.update { it.copy(file=file) };notice.value="更新包已下载并校验，点击安装继续"
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) { notice.value=errorText(error) }
            finally { mutableUpdate.update { it.copy(busy=false,downloading=false) } }
        }
    }
    fun installUpdate() = operation { if(!update.value.busy) update.value.file?.let { updater.install(it) } }
    fun operation(block: suspend ()->Unit) { viewModelScope.launch { try { block() } catch(error: CancellationException) { throw error } catch(error: Exception) { notice.value=errorText(error) } } }
    private fun errorText(error: Exception): String = when(error) {
        is StreamAddressException -> error.message.orEmpty()
        is PlatformException -> error.message ?: "平台暂不可用"
        is IllegalArgumentException -> error.message ?: "数据格式无效"
        is java.io.IOException -> "网络请求失败，请检查网络后重试"
        else -> "操作失败，请重试；若持续失败，请检查登录或配置内容"
    }
    fun selectPlatform(platform: Platform) {
        cancelSubcategories();browseJob?.cancel(); categoryJob?.cancel(); browseGeneration++
        browse.value=BrowseState(platform=platform,category=if(platform==Platform.DOUYIN) Category("1_1_1_1010032","和平精英","射击游戏") else null)
        val requiresCategory=platform in listOf(Platform.DOUYU,Platform.BILIBILI)
        if(requiresCategory) browse.update { it.copy(loading=true) }
        categoryJob=viewModelScope.launch {
            try {
                val categories=app.platforms.getValue(platform).categories()
                if(browse.value.platform==platform) {
                    browse.update { it.copy(categories=categories) }
                    if(platform in listOf(Platform.DOUYU,Platform.BILIBILI) && browse.value.category==null) {
                        val first=categories.firstOrNull { it.level==2 && it.queryId.isNotBlank() }
                            ?: throw PlatformException("${platform.label}分类为空，请重试")
                        selectCategory(first)
                    }
                }
            }
            catch(error: CancellationException) { throw error }
            catch(error: Exception) { if(browse.value.platform==platform) {
                if(platform in listOf(Platform.DOUYU,Platform.BILIBILI) && browse.value.category==null) browse.update { it.copy(loading=false,error="分类加载失败，请重试") }
                else notice.value="分类加载失败，可重试切换平台"
            } }
        }
        if(!requiresCategory) loadRooms()
    }
    fun selectCategory(category: Category?) {
        browseJob?.cancel(); browseGeneration++
        val selected=category ?: when(browse.value.platform) {
            Platform.DOUYIN -> browse.value.categories.firstOrNull() ?: Category("1_1_1_1010032","和平精英","射击游戏")
            Platform.DOUYU,Platform.BILIBILI -> browse.value.categories.firstOrNull { it.level==2 && it.queryId.isNotBlank() }
            else -> null
        }
        browse.update { it.copy(category=selected,rooms=emptyList(),page=1) }; loadRooms()
    }
    fun loadRooms(append: Boolean=false) {
        if(browse.value.platform in listOf(Platform.DOUYU,Platform.BILIBILI) && browse.value.category==null) {
            if(categoryJob?.isActive!=true) selectPlatform(browse.value.platform)
            return
        }
        if(append && (browse.value.loading || !browse.value.more)) return
        browseJob?.cancel(); val generation=++browseGeneration
        val state=browse.value; val page=if(append) state.page+1 else 1
        browse.update { it.copy(loading=true,error="",needsLogin=false,verification=null) }
        browseJob=viewModelScope.launch {
            try {
                val rooms=app.platforms.getValue(state.platform).rooms(state.category,page)
                if(generation==browseGeneration) browse.update { it.copy(loading=false,rooms=((if(append) it.rooms else emptyList())+rooms).distinctBy { room->room.key },page=page,more=rooms.isNotEmpty()) }
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) { if(generation==browseGeneration) browse.update { it.copy(loading=false,error=errorText(error),needsLogin=requiresLogin(state.platform,error),verification=error as? BiliListChallenge,verificationAppend=append) } }
        }
    }
    fun completeVerification(challenge: BiliListChallenge,token: String?) {
        val state=browse.value
        if(state.verification !== challenge) return
        browse.update { it.copy(verification=null) }
        if(token==null) {
            browse.update { it.copy(error="安全验证未完成，可手动重试") }; return
        }
        browseJob?.cancel(); val generation=++browseGeneration
        browse.update { it.copy(loading=true,error="") }
        browseJob=viewModelScope.launch {
            try {
                val rooms=(app.platforms.getValue(Platform.BILIBILI) as BilibiliPlatform).verifiedRooms(challenge,token)
                if(generation==browseGeneration) browse.update { it.copy(loading=false,
                    rooms=((if(state.verificationAppend) state.rooms else emptyList())+rooms).distinctBy { room->room.key },
                    page=if(state.verificationAppend) state.page+1 else 1,more=rooms.isNotEmpty()) }
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) { if(generation==browseGeneration) browse.update { it.copy(loading=false,error=errorText(error)) } }
        }
    }
    fun searchPlatform(platform: Platform) { searchJob?.cancel(); searchGeneration++; search.value=SearchState(platform=platform,keyword=search.value.keyword) }
    fun searchKeyword(keyword: String) { search.update { it.copy(keyword=keyword) } }
    fun searchAccounts(accounts: Boolean) { search.update { it.copy(accounts=accounts) }; searchRooms() }
    fun searchRooms(append: Boolean=false) {
        val state=search.value
        if(append && (state.loading || !state.more)) return
        val query=if(append) state.committed ?: return else SearchQuery(state.platform,state.keyword.trim(),state.accounts)
        if(query.keyword.isBlank()) {
            searchJob?.cancel();searchGeneration++
            search.update { it.copy(rooms=emptyList(),committed=null,loading=false,error="",page=1,more=false) };return
        }
        searchJob?.cancel(); val generation=++searchGeneration; val page=if(append) state.page+1 else 1
        search.update { it.copy(loading=true,error="",committed=query,page=if(append) it.page else 1,rooms=if(append) it.rooms else emptyList()) }
        searchJob=viewModelScope.launch {
            try {
                val adapter=app.platforms.getValue(query.platform)
                val rooms=if(query.platform==Platform.DOUYIN) (adapter as DouyinPlatform).searchMode(query.keyword,page,query.accounts) else adapter.search(query.keyword,page)
                if(generation==searchGeneration) search.update { it.copy(loading=false,rooms=((if(append) it.rooms else emptyList())+rooms).distinctBy { room->room.key+room.userId },page=page,more=rooms.isNotEmpty()) }
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) { if(generation==searchGeneration) search.update { it.copy(loading=false,error=errorText(error)) } }
        }
    }
    fun openRoomId() {
        val state=search.value
        val raw=state.keyword.trim()
        val id=if(raw.startsWith("https://") || raw.startsWith("http://")) android.net.Uri.parse(raw).lastPathSegment.orEmpty() else raw
        if(!id.matches(Regex("[A-Za-z0-9]+"))) { notice.value="请输入有效房间号"; return }
        open(Room(state.platform,id,id))
    }
    fun open(room: Room,quality: String?=null,line: String?=null) {
        if(room.id.isBlank()) { notice.value="该账号未提供直播房间号"; return }
        playerJob?.cancel(); stopDanmaku()
        val generation=++playerGeneration
        val revision=player.value.revision+1
        player.value=PlayerState(room=room,loading=true,revision=revision)
        playerJob=viewModelScope.launch {
            try {
                val adapter=app.platforms.getValue(room.platform)
                val current=adapter.detail(room)
                if(generation!=playerGeneration) return@launch
                if(current.status!=LiveStatus.LIVE) { player.value=PlayerState(room=current,error=if(current.status==LiveStatus.REPLAY) "主播正在轮播，暂无实时直播" else "主播未开播",revision=revision); return@launch }
                val result=adapter.playback(current,quality ?: preferences.value["${room.platform.name}_preferred_quality"],line ?: preferences.value["${room.platform.name}_preferred_line"])
                if(generation!=playerGeneration) return@launch
                player.value=PlayerState(room=result.room,playback=result,revision=revision)
                storage.preference("${room.platform.name}_preferred_quality",result.quality)
                storage.preference("${room.platform.name}_preferred_line",result.line)
                if(generation!=playerGeneration) return@launch
                startDanmaku()
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) { if(generation==playerGeneration) player.update { it.copy(loading=false,error=errorText(error),needsLogin=requiresLogin(room.platform,error)) } }
        }
    }
    fun closePlayer() { playerGeneration++; playerJob?.cancel(); stopDanmaku(); player.value=PlayerState() }
    private fun stopDanmaku() { danmakuGeneration++;danmakuJob?.cancel() }
    fun reconnectDanmaku() {
        if(player.value.playback==null) return
        stopDanmaku()
        player.update { it.copy(danmakuStatus="正在重连弹幕") }
        startDanmaku()
    }
    private fun startDanmaku() {
        if(!foreground || danmakuJob?.isActive==true) return
        val room=player.value.playback?.room ?: return
        val generation=playerGeneration
        val connectionGeneration=++danmakuGeneration
        val previous=danmakuJob
        danmakuJob=viewModelScope.launch(Dispatchers.IO) {
            previous?.join()
            ensureActive()
            coroutineScope {
                val pending=kotlinx.coroutines.channels.Channel<Danmaku>(512,kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)
                fun current()=generation==playerGeneration && connectionGeneration==danmakuGeneration && foreground
                val flush=launch(Dispatchers.Main.immediate) {
                    while(isActive) {
                        delay(50)
                        val batch=ArrayList<Danmaku>()
                        while(batch.size<512) {
                            val message=pending.tryReceive().getOrNull() ?: break
                            batch.add(message)
                        }
                        if(batch.isNotEmpty() && current()) {
                            screenMessages.tryEmit(batch)
                            player.update { if(current()) it.copy(messages=(it.messages+batch).takeLast(200)) else it }
                        }
                    }
                }
                try {
                    app.danmaku.listen(room,{ message -> if(current()) pending.trySend(message) },{ status -> player.update {
                        if(current()) it.copy(danmakuStatus=status) else it
                    } })
                } finally { pending.close();flush.cancelAndJoin() }
            }
        }
    }
    fun foreground(active: Boolean) { foreground=active;if(active) startDanmaku() else stopDanmaku() }
    fun toggleFollow(room: Room) = operation { if(library.value.follows.any { it.room.key==room.key }) storage.unfollow(room) else storage.follow(room) }
    fun refreshFollows() {
        if(refreshJob?.isActive==true) return
        mutableFollowsRefreshing.value=true
        refreshJob=viewModelScope.launch {
            try {
                var failed=0
                for(follow in library.value.follows) {
                    try {
                        val room=app.platforms.getValue(follow.room.platform).detail(follow.room)
                        // Re-read after the request so deleting or moving a follow cannot be undone by refresh.
                        storage.refreshRoom(follow.room.key,room.copy(id=follow.room.id))
                    } catch(error: CancellationException) { throw error } catch(error: Exception) { failed++ }
                }
                notice.value=if(failed==0) "关注状态已刷新" else "刷新完成，$failed 个房间暂不可用"
            } finally { mutableFollowsRefreshing.value=false }
        }
    }

    fun setPreference(key: String,value: String) = operation { storage.preference(key,value) }
    fun subscribe(platform: Platform,category: Category) = operation { storage.subscribe(platform,category) }
    fun cancelSubcategories() { subcategoryGeneration++;subcategoryJob?.cancel();subcategoryJob=null }
    fun subcategories(category: Category,onLoaded: (List<Category>)->Unit) {
        cancelSubcategories()
        if(browse.value.platform!=Platform.DOUYU) return
        val generation=subcategoryGeneration
        subcategoryJob=viewModelScope.launch {
            try {
                val children=(app.platforms.getValue(Platform.DOUYU) as DouyuPlatform).children(category)
                if(generation==subcategoryGeneration && browse.value.platform==Platform.DOUYU) onLoaded(children)
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) { if(generation==subcategoryGeneration && browse.value.platform==Platform.DOUYU) notice.value=errorText(error) }
        }
    }
}
