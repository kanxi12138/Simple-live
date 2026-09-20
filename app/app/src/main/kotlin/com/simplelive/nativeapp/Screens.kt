@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class, androidx.compose.foundation.ExperimentalFoundationApi::class)
package com.simplelive.nativeapp

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil.compose.AsyncImage
import java.util.UUID
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

private val LightPalette=lightColorScheme(primary=Color(0xff1764d9),onPrimary=Color.White,background=Color(0xfff5f8fc),surface=Color.White,onSurface=Color(0xff17263c),onBackground=Color(0xff17263c),onSurfaceVariant=Color(0xff52647b),surfaceVariant=Color(0xffeaf0f8),outlineVariant=Color(0xffdce4ef))
private val DarkPalette=darkColorScheme(primary=Color(0xff91baff),onPrimary=Color(0xff062858),background=Color(0xff101a2a),surface=Color(0xff17263c),onSurface=Color(0xffedf3fc),onBackground=Color(0xffedf3fc),onSurfaceVariant=Color(0xffb1bfd3),surfaceVariant=Color(0xff24354d))

@Composable
fun LiveApp(model: LiveViewModel,onLogin: (Platform)->Unit,onFullscreen: (Boolean)->Unit,onVerify: (BiliListChallenge)->Unit) {
    val preferences by model.preferences.collectAsStateWithLifecycle()
    val player by model.player.collectAsStateWithLifecycle()
    val ready by model.ready.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val systemDark=isSystemInDarkTheme()
    val dark=when(preferences["theme_preference"]) { "light" -> false; "dark" -> true; else -> systemDark }
    MaterialTheme(colorScheme=if(dark) DarkPalette else LightPalette,typography=Typography(
        headlineSmall=MaterialTheme.typography.headlineSmall.copy(fontSize=22.sp,fontWeight=FontWeight.Bold),
        bodyLarge=MaterialTheme.typography.bodyLarge.copy(fontSize=16.sp),bodyMedium=MaterialTheme.typography.bodyMedium.copy(fontSize=14.sp))) {
        val snackbar=remember { SnackbarHostState() }
        LaunchedEffect(notice) { if(notice.isNotBlank()) { snackbar.showSnackbar(notice); if(model.notice.value==notice) model.notice.value="" } }
        Scaffold(snackbarHost={ SnackbarHost(snackbar) },bottomBar={
            if(player.room==null) NavigationBar(containerColor=MaterialTheme.colorScheme.surface) {
                listOf(Triple("首页",Icons.Outlined.Home,0),Triple("搜索",Icons.Outlined.Search,1),Triple("关注",Icons.Outlined.FavoriteBorder,2),Triple("设置",Icons.Outlined.Settings,3)).forEach { (label,icon,index)->
                    NavigationBarItem(selected=tab==index,onClick={tab=index},icon={Icon(icon,label)},label={Text(label)})
                }
            }
        }) { padding ->
            if(!ready) Box(Modifier.fillMaxSize().padding(padding),contentAlignment=Alignment.Center) { CircularProgressIndicator() }
            else if(player.room!=null) PlayerScreen(model,onFullscreen,onLogin)
            else Column(Modifier.fillMaxSize().padding(padding)) {
                when(tab) {
                    0 -> HomeScreen(model,onLogin,onVerify)
                    1 -> SearchScreen(model,onLogin)
                    2 -> FollowsScreen(model)
                    else -> SettingsScreen(model,onLogin)
                }
            }
        }
    }
}
@Composable
fun SectionTitle(title: String,subtitle: String="",actions: @Composable RowScope.()->Unit={}) {
    Row(Modifier.fillMaxWidth().padding(start=20.dp,end=12.dp,top=16.dp,bottom=12.dp),verticalAlignment=Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) { Text(title,style=MaterialTheme.typography.headlineSmall); if(subtitle.isNotBlank()) Text(subtitle,style=MaterialTheme.typography.bodySmall,color=MaterialTheme.colorScheme.onSurfaceVariant) }
        actions()
    }
}
@Composable
fun PlatformTabs(selected: Platform,onSelect: (Platform)->Unit) {
    LazyRow(Modifier.fillMaxWidth(),contentPadding=PaddingValues(horizontal=16.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)) {
        items(Platform.entries) { platform -> FilterChip(selected=selected==platform,onClick={onSelect(platform)},label={Text(platform.label)},modifier=Modifier.heightIn(min=48.dp)) }
    }
}
@Composable
private fun HomeScreen(model: LiveViewModel,onLogin: (Platform)->Unit,onVerify: (BiliListChallenge)->Unit) {
    val state by model.browse.collectAsStateWithLifecycle()
    val library by model.library.collectAsStateWithLifecycle()
    var categoryPicker by remember { mutableStateOf(false) }
    var categorySearch by remember { mutableStateOf("") }
    var children by remember { mutableStateOf<List<Category>?>(null) }
    fun closeCategories() { model.cancelSubcategories();categoryPicker=false;children=null }
    DisposableEffect(model) { onDispose { model.cancelSubcategories() } }
    LaunchedEffect(state.platform) { closeCategories() }
    SectionTitle("简直播","找到想看的那一场") {
        RefreshButton(state.loading,"刷新列表",{model.loadRooms()})
    }
    PlatformTabs(state.platform,model::selectPlatform)
    Row(Modifier.fillMaxWidth().padding(horizontal=16.dp),verticalAlignment=Alignment.CenterVertically) {
        TextButton(onClick={model.cancelSubcategories();children=null;categoryPicker=true}) { Icon(Icons.Outlined.Tune,null,Modifier.size(18.dp)); Spacer(Modifier.width(6.dp)); Text(state.category?.name ?: "全部分类",maxLines=1,overflow=TextOverflow.Ellipsis) }
        Spacer(Modifier.weight(1f))
        state.category?.let { category -> TextButton(onClick={model.subscribe(state.platform,category)}) { Text(if(library.subscriptions.any { it.platform==state.platform && it.category.id==category.id && it.category.level==category.level }) "取消订阅" else "订阅分类") } }
    }
    if(library.subscriptions.isNotEmpty()) LazyRow(contentPadding=PaddingValues(horizontal=16.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)) {
        items(library.subscriptions,key={it.key}) { subscription -> InputChip(selected=state.platform==subscription.platform && state.category?.id==subscription.category.id,
            onClick={if(state.platform!=subscription.platform) model.selectPlatform(subscription.platform); model.selectCategory(subscription.category)},label={Text("${subscription.platform.label} · ${subscription.category.name}")},
            trailingIcon={Icon(Icons.Outlined.Close,"取消订阅",Modifier.size(18.dp).clickable { model.operation { model.storage.removeSubscription(subscription.key) } })}) }
    }
    state.verification?.let { challenge -> TextButton(onClick={onVerify(challenge)},modifier=Modifier.padding(horizontal=16.dp)) { Text("完成B站安全验证") } }
    if(state.needsLogin) TextButton(onClick={onLogin(state.platform)},modifier=Modifier.padding(horizontal=16.dp)) { Text("登录${state.platform.label}后重试") }
    RoomGrid(state.rooms,state.loading,state.error,state.more,{model.loadRooms(true)},{model.loadRooms()},model::open)
    if(categoryPicker) ModalBottomSheet(onDismissRequest={closeCategories()}) {
        Column(Modifier.fillMaxWidth().fillMaxHeight(0.85f).padding(horizontal=20.dp)) {
            Text(if(children==null) "直播分类" else "斗鱼子分类",style=MaterialTheme.typography.headlineSmall)
            OutlinedTextField(categorySearch,{categorySearch=it},label={Text("查找分类")},singleLine=true,modifier=Modifier.fillMaxWidth().padding(vertical=12.dp))
            TextButton(onClick={model.selectCategory(null);closeCategories()}) { Text(if(state.platform in listOf(Platform.DOUYIN,Platform.DOUYU,Platform.BILIBILI)) "默认分类" else "全部直播") }
            if(children!=null) TextButton(onClick={model.cancelSubcategories();children=null}) { Text("返回所有分类") }
            LazyColumn {
                items((children ?: state.categories).filter { it.name.contains(categorySearch,true) || it.parent.contains(categorySearch,true) },key={"${it.level}:${it.parent}:${it.id}"}) { category ->
                    ListItem(headlineContent={Text(category.name)},supportingContent={if(category.parent.isNotBlank()) Text(category.parent)},modifier=Modifier.clickable { model.selectCategory(category); closeCategories() },
                        trailingContent={if(state.platform==Platform.DOUYU && category.level==2) TextButton(onClick={model.subcategories(category){children=it}}){Text("细分")} })
                }
                if(state.categories.isEmpty()) item { Text("分类尚未加载，请返回后重新切换平台",Modifier.padding(16.dp)) }
            }
        }
    }
}
@Composable
private fun SearchScreen(model: LiveViewModel,onLogin: (Platform)->Unit) {
    val state by model.search.collectAsStateWithLifecycle()
    SectionTitle("搜索","搜索主播，或直接输入房间号")
    PlatformTabs(state.platform,model::searchPlatform)
    OutlinedTextField(state.keyword,model::searchKeyword,placeholder={Text("主播名称 / 房间号")},singleLine=true,
        trailingIcon={IconButton(onClick={model.searchRooms()}) { Icon(Icons.Outlined.Search,"搜索主播") }},
        modifier=Modifier.fillMaxWidth().padding(horizontal=16.dp,vertical=8.dp),shape=RoundedCornerShape(16.dp))
    Row(Modifier.fillMaxWidth().padding(horizontal=16.dp),horizontalArrangement=Arrangement.spacedBy(8.dp),verticalAlignment=Alignment.CenterVertically) {
        TextButton(onClick=model::openRoomId){Text("进入房间号")}
        if(state.platform==Platform.DOUYIN) {
            FilterChip(selected=state.accounts,onClick={model.searchAccounts(!state.accounts)},label={Text(if(state.accounts) "账号搜索" else "直播搜索")})
        }
        Spacer(Modifier.weight(1f))
        if(state.platform in listOf(Platform.DOUYIN,Platform.BILIBILI)) TextButton(onClick={onLogin(state.platform)}) { Text("登录") }
    }
    RoomGrid(state.rooms,state.loading,state.error,state.more,{model.searchRooms(true)},{model.searchRooms()},model::open,emptyText="输入关键词开始搜索")
}
@Composable
fun RoomGrid(rooms: List<Room>,loading: Boolean,error: String,more: Boolean,onMore: ()->Unit,onRetry: ()->Unit,onRoom: (Room)->Unit,emptyText: String="暂无直播，请选择其他分类") {
    LazyVerticalGrid(columns=GridCells.Fixed(2),contentPadding=PaddingValues(16.dp),horizontalArrangement=Arrangement.spacedBy(12.dp),verticalArrangement=Arrangement.spacedBy(18.dp)) {
        if(error.isNotBlank()) item(span={GridItemSpan(2)}) { ErrorCard(error,onRetry) }
        items(rooms,key={it.key+it.userId}) { room ->
            Column(Modifier.clickable { onRoom(room) }) {
                Box(Modifier.fillMaxWidth().aspectRatio(1.5f).clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.surfaceVariant)) {
                    AsyncImage(model=room.cover.ifBlank { room.avatar },contentDescription=room.title,contentScale=ContentScale.Crop,modifier=Modifier.fillMaxSize())
                    if(room.status==LiveStatus.LIVE) Text("直播中",color=Color.White,fontSize=10.sp,modifier=Modifier.align(Alignment.BottomStart).padding(6.dp).background(Color(0xb31764d9),RoundedCornerShape(4.dp)).padding(horizontal=6.dp,vertical=3.dp))
                    if(room.status==LiveStatus.OFFLINE) Text("未开播",color=Color.White,fontSize=10.sp,modifier=Modifier.align(Alignment.BottomStart).padding(6.dp).background(Color(0xb3000000),RoundedCornerShape(4.dp)).padding(4.dp))
                    if(room.viewers.isNotBlank()) Text(room.viewers,color=Color.White,fontSize=10.sp,modifier=Modifier.align(Alignment.BottomEnd).padding(6.dp).background(Color(0x88000000),RoundedCornerShape(4.dp)).padding(3.dp))
                }
                Text(room.title.ifBlank { room.name },maxLines=2,overflow=TextOverflow.Ellipsis,style=MaterialTheme.typography.bodyMedium,fontWeight=FontWeight.Medium,modifier=Modifier.padding(top=8.dp))
                Text(room.name,maxLines=1,overflow=TextOverflow.Ellipsis,style=MaterialTheme.typography.bodySmall,color=MaterialTheme.colorScheme.onSurfaceVariant,modifier=Modifier.padding(top=3.dp))
            }
        }
        if(loading) item(span={GridItemSpan(2)}) { Box(Modifier.fillMaxWidth().padding(24.dp),contentAlignment=Alignment.Center) { CircularProgressIndicator(Modifier.size(28.dp)) } }
        if(!loading && rooms.isEmpty() && error.isBlank()) item(span={GridItemSpan(2)}) { Text(emptyText,Modifier.padding(vertical=40.dp),color=MaterialTheme.colorScheme.onSurfaceVariant) }
        if(!loading && rooms.isNotEmpty() && more) item(span={GridItemSpan(2)}) { TextButton(onClick=onMore,modifier=Modifier.fillMaxWidth()) { Text("加载更多") } }
    }
}
@Composable
fun ErrorCard(message: String,onRetry: ()->Unit) {
    Surface(color=MaterialTheme.colorScheme.errorContainer,shape=RoundedCornerShape(12.dp)) {
        Row(Modifier.fillMaxWidth().padding(12.dp),verticalAlignment=Alignment.CenterVertically) { Text(message,Modifier.weight(1f),color=MaterialTheme.colorScheme.onErrorContainer); TextButton(onClick=onRetry) { Text("重试") } }
    }
}
@Composable
private fun ColumnScope.FollowsScreen(model: LiveViewModel) {
    val library by model.library.collectAsStateWithLifecycle()
    var selected by remember { mutableStateOf<Follow?>(null) }
    var editFolder by remember { mutableStateOf<Folder?>(null) }
    var newFolder by remember { mutableStateOf(false) }
    val labels=remember { listOf("正在直播","全部平台")+Platform.entries.map { it.label } }
    val refreshing by model.followsRefreshing.collectAsStateWithLifecycle()
    val pager=rememberPagerState(initialPage=1,pageCount={labels.size})
    val tabs=rememberLazyListState()
    // Keep each page's list state outside the lazy pager so eviction does not reset it.
    val lists=List(labels.size) { rememberLazyListState() }
    val scope=rememberCoroutineScope()
    var pageAnimation by remember { mutableStateOf<Job?>(null) }
    var managementPage by remember { mutableIntStateOf(1) }
    fun visibleFollows(page: Int)=library.follows.filter {
        when(page) {
            0 -> it.room.status==LiveStatus.LIVE
            1 -> true
            else -> it.room.platform==Platform.entries[page-2]
        }
    }.sortedByDescending { it.pinned }
    LaunchedEffect(pager.currentPage) { tabs.animateScrollToItem(pager.currentPage) }
    SectionTitle("我的关注","${library.follows.size} 位主播") {
        IconButton(onClick={newFolder=true}){Icon(Icons.Outlined.CreateNewFolder,"新建分组")}
        RefreshButton(refreshing,"刷新直播状态",model::refreshFollows)
        IconButton(onClick={val expand=!library.folders.all { it.expanded };model.operation { library.folders.forEach { model.storage.folder(it.copy(expanded=expand)) } }}){Icon(Icons.Outlined.UnfoldMore,"展开或收起全部分组")}
    }
    LazyRow(state=tabs,contentPadding=PaddingValues(horizontal=16.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)) {
        items(labels.size,key={it}) { page ->
            FilterChip(selected=pager.currentPage==page,onClick={
                pageAnimation?.cancel()
                pageAnimation=scope.launch { pager.animateScrollToPage(page) }
            },label={Text(labels[page])},modifier=Modifier.heightIn(min=48.dp))
        }
    }
    HorizontalPager(state=pager,modifier=Modifier.fillMaxWidth().weight(1f),
        userScrollEnabled=selected==null && editFolder==null && !newFolder,verticalAlignment=Alignment.Top) { page ->
        val visible=visibleFollows(page)
        LazyColumn(state=lists[page],modifier=Modifier.fillMaxSize(),contentPadding=PaddingValues(bottom=24.dp)) {
            if(visible.isEmpty()) item {
                Text(if(library.follows.isEmpty()) "在直播间关注主播，即可在这里找到他们。" else if(page==0) "暂无正在直播的关注" else "当前平台暂无关注",
                    Modifier.padding(24.dp),color=MaterialTheme.colorScheme.onSurfaceVariant)
            }
            val topItems=(visible.filter { it.folder.isBlank() }.map { Triple(it.room.key,if(it.pinned) Int.MIN_VALUE else it.position,false) } + library.folders.map { Triple(it.id,it.position,true) }).sortedBy { it.second }
            items(topItems,key={if(it.third) "folder:${it.first}" else it.first}) { item ->
                if(!item.third) {
                    val follow=visible.first { it.room.key==item.first }
                    FollowRow(follow,{model.open(follow.room)},{managementPage=page;selected=follow})
                } else {
                    val folder=library.folders.first { it.id==item.first }
                    val members=visible.filter { it.folder==folder.id }
                    ListItem(headlineContent={Text(folder.name,fontWeight=FontWeight.SemiBold)},supportingContent={Text("${members.size} 位主播")},leadingContent={Icon(Icons.Outlined.Folder,null)},
                        trailingContent={IconButton(onClick={editFolder=folder}){Icon(Icons.Outlined.MoreVert,"管理分组")}},modifier=Modifier.clickable { model.operation { model.storage.folder(folder.copy(expanded=!folder.expanded)) } })
                    if(folder.expanded) members.forEach { follow -> FollowRow(follow,{model.open(follow.room)},{managementPage=page;selected=follow}) }
                }
            }
        }
    }
    if(newFolder || editFolder!=null) {
        var name by remember(editFolder) { mutableStateOf(editFolder?.name.orEmpty()) }
        AlertDialog(onDismissRequest={newFolder=false;editFolder=null},title={Text(if(editFolder==null) "新建分组" else "管理分组")},
            text={Column { OutlinedTextField(name,{name=it},label={Text("分组名称")}); editFolder?.let { folder ->
                TextButton(onClick={model.operation { model.storage.deleteFolder(folder) }; editFolder=null}){Text("删除分组，保留主播")}
                val index=library.folders.indexOfFirst { it.id==folder.id }
                if(index>0) TextButton(onClick={model.operation { model.storage.swap("folder:${folder.id}","folder:${library.folders[index-1].id}") };editFolder=null}) { Text("分组上移") }
            } }},
            confirmButton={TextButton(enabled=name.isNotBlank(),onClick={val folder=editFolder?.copy(name=name.trim()) ?: Folder(UUID.randomUUID().toString(),name.trim(),position=library.folders.size+library.follows.size);model.operation { model.storage.folder(folder) };newFolder=false;editFolder=null}){Text("保存")}},dismissButton={TextButton(onClick={newFolder=false;editFolder=null}){Text("取消")}})
    }
    selected?.let { selectedFollow ->
        val follow=library.follows.firstOrNull { it.room.key==selectedFollow.room.key } ?: selectedFollow
        var displayName by remember(follow.room.key) { mutableStateOf(follow.displayName) }
        ModalBottomSheet(onDismissRequest={selected=null}) {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal=20.dp)) {
                Text(follow.room.name,style=MaterialTheme.typography.headlineSmall)
                OutlinedTextField(displayName,{displayName=it},label={Text("备注名")},modifier=Modifier.fillMaxWidth().padding(top=12.dp))
                TextButton(onClick={model.operation { model.storage.editFollow(follow.copy(displayName=displayName)) };selected=null}) { Text("保存备注") }
                TextButton(onClick={model.operation { model.storage.editFollow(follow.copy(pinned=!follow.pinned)) };selected=null}) { Text(if(follow.pinned) "取消置顶" else "置顶主播") }
                val siblings=visibleFollows(managementPage).filter { it.folder==follow.folder && it.pinned==follow.pinned }
                val index=siblings.indexOfFirst { it.room.key==follow.room.key }
                Row {
                    TextButton(enabled=index>0,onClick={model.operation { model.storage.swapFollows(follow,siblings[index-1]) };selected=null}){Text("上移")}
                    TextButton(enabled=index>=0 && index<siblings.lastIndex,onClick={model.operation { model.storage.swapFollows(follow,siblings[index+1]) };selected=null}){Text("下移")}
                }
                Text("移动到分组",fontWeight=FontWeight.SemiBold)
                TextButton(onClick={model.operation { model.storage.editFollow(follow.copy(folder="")) };selected=null}){Text("未分组")}
                library.folders.forEach { folder -> TextButton(onClick={model.operation { model.storage.editFollow(follow.copy(folder=folder.id)) };selected=null}){Text(folder.name)} }
                TextButton(onClick={model.operation { model.storage.unfollow(follow.room) };selected=null}){Text("取消关注",color=MaterialTheme.colorScheme.error)}
                Spacer(Modifier.height(24.dp))
            }
        }
    }
}
@Composable
private fun FollowRow(follow: Follow,onOpen: ()->Unit,onManage: ()->Unit) {
    ListItem(headlineContent={Text((if(follow.pinned) "置顶 · " else "")+follow.displayName.ifBlank { follow.room.name },maxLines=1,overflow=TextOverflow.Ellipsis)},
        supportingContent={Text("${follow.room.platform.label} · ${when(follow.room.status) { LiveStatus.LIVE -> "直播中";LiveStatus.OFFLINE -> "未开播";LiveStatus.REPLAY -> "轮播中";else -> "状态待刷新" }}",color=if(follow.room.status==LiveStatus.LIVE) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)},
        leadingContent={AsyncImage(follow.room.avatar,null,Modifier.size(48.dp).clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.surfaceVariant),contentScale=ContentScale.Crop)},
        trailingContent={IconButton(onClick=onManage){Icon(Icons.Outlined.MoreVert,"管理关注")}},modifier=Modifier.combinedClickable(onClick=onOpen,onLongClick=onManage))
}
