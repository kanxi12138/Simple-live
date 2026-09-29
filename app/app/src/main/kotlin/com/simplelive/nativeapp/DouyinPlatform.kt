package com.simplelive.nativeapp

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.withContext
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import okhttp3.Cookie
import okhttp3.Request
import okhttp3.HttpUrl.Companion.toHttpUrl
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONException
import java.security.SecureRandom

const val DOUYIN_UA = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.5845.97 Safari/537.36 Core/1.116.567.400 QQBrowser/19.7.6764.400"
fun cookieValue(cookie: String, name: String): String = cookie.split(';').map { it.trim() }.firstOrNull { it.startsWith("$name=") }?.substringAfter('=').orEmpty()

class DouyinPlatform(private val context: Context, private val net: Network, private val credentials: Credentials) : LivePlatform {
    private var guestCookie: String = ""
    private var guestGeneration=-1L
    private data class SearchSession(val generation: Long,val cookies: List<Cookie>,val savedCookies: List<Cookie>,val webId: String="",val fallbackToken: String="")
    private val searchMutex=Mutex()
    @Volatile private var searchSession: SearchSession?=null
    suspend fun authHeaders(refreshGuest: Boolean = false,url: String=Platform.DOUYIN.home,snapshot: DouyinCredentialSnapshot=credentials.douyinSnapshot()): Map<String, String> {
        val target=url.toHttpUrl()
        require(target.isHttps && target.host in listOf("www.douyin.com","live.douyin.com")) { "抖音会话地址不受支持" }
        if(guestGeneration!=snapshot.generation) { guestCookie="";guestGeneration=snapshot.generation }
        val saved = snapshot.header(url)
        if (refreshGuest && saved.isBlank()) guestCookie = ""
        if (saved.isBlank() && guestCookie.isBlank()) {
            guestCookie = withContext(Dispatchers.IO) {
                val request = Request.Builder().url("https://live.douyin.com/").header("User-Agent", DOUYIN_UA).build()
                Network.await(net.client.newCall(request)).use { response ->
                    if (response.code in listOf(401,403,429)) throw PlatformException(when(response.code) {
                        401 -> "抖音身份验证失败，请重新登录或重试"
                        403 -> "抖音拒绝访问，请稍后重试"
                        else -> "抖音请求过于频繁，请稍后重试"
                    },httpStatus=response.code)
                    response.headers("Set-Cookie").map { it.substringBefore(';') }.filter { it.substringBefore('=') in listOf("ttwid","msToken","__ac_nonce","tt_scid","s_v_web_id") }.joinToString("; ")
                }
            }
            if (cookieValue(guestCookie,"ttwid").isBlank()) throw PlatformException("抖音未返回访客会话，请稍后重试")
        }
        var cookie=saved.ifBlank { guestCookie }
        if(cookieValue(cookie,"msToken").isBlank()) {
            val alphabet="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            val random=SecureRandom()
            val token=(0 until 107).map { alphabet[random.nextInt(alphabet.length)] }.joinToString("")
            cookie += "; msToken=$token"
            if(saved.isBlank()) guestCookie=cookie
        }
        return platformHeaders(Platform.DOUYIN) + mapOf("User-Agent" to DOUYIN_UA, "Cookie" to cookie)
    }
    fun clearSession() { credentials.clear(Platform.DOUYIN);guestCookie="";guestGeneration=-1L;searchSession=null }

    private fun checkSearchSession(generation: Long) {
        if(credentials.douyinSessionGeneration()!=generation) throw PlatformException("抖音会话已改变，请重新搜索")
    }

    private fun mergeSearchCookies(cookies: List<Cookie>,updates: List<Cookie>): List<Cookie> {
        val merged=cookies.associateBy { Triple(it.name,it.domain,it.path) }.toMutableMap()
        updates.forEach { merged[Triple(it.name,it.domain,it.path)]=it }
        return merged.values.filter { it.expiresAt>System.currentTimeMillis() }
    }

    private fun searchCookieHeader(session: SearchSession,url: String): String {
        val cookie=DouyinCredentialSnapshot(session.cookies,session.generation).header(url)
        return if(cookieValue(cookie,"msToken").isBlank() && session.fallbackToken.isNotBlank())
            listOf(cookie,"msToken=${session.fallbackToken}").filter { it.isNotBlank() }.joinToString("; ") else cookie
    }

    /** Keep temporary search cookies, but let persisted refreshes and deletions take precedence. */
    private fun currentSearchSession(snapshot: DouyinCredentialSnapshot): SearchSession {
        val previous=searchSession?.takeIf { it.generation==snapshot.generation }
            ?: return SearchSession(snapshot.generation,snapshot.cookies,snapshot.cookies)
        val current=snapshot.cookies.associateBy { Triple(it.name,it.domain,it.path) }
        val saved=previous.savedCookies.associateBy { Triple(it.name,it.domain,it.path) }
        val changed=(saved.keys+current.keys).filter { saved[it]!=current[it] }.toSet()
        val retained=previous.cookies.filter { Triple(it.name,it.domain,it.path) !in changed }
        return previous.copy(cookies=mergeSearchCookies(retained,changed.mapNotNull { current[it] }),savedCookies=snapshot.cookies)
    }

    private suspend fun searchRequest(url: String,session: SearchSession,headers: Map<String,String>): String {
        val snapshot=DouyinCredentialSnapshot(session.savedCookies,session.generation)
        checkSearchSession(session.generation)
        val result=net.request(url,headers+("Cookie" to searchCookieHeader(session,url)),onHeaders={ response ->
            checkSearchSession(session.generation)
            val updates=response.values("Set-Cookie")
            credentials.updateDouyinCookies(snapshot,url,updates)
            // Guest responses also refresh the in-memory jar; they never create an account login.
            val cookies=updates.mapNotNull { Cookie.parse(url.toHttpUrl(),it) }
            searchSession=session.copy(cookies=mergeSearchCookies(session.cookies,cookies))
        }).toString(Charsets.UTF_8)
        checkSearchSession(session.generation)
        return result
    }

    private suspend fun prepareSearchSession(url: String,referer: String): SearchSession {
        var session=currentSearchSession(credentials.douyinSnapshot())
        searchSession=session
        fun webId(): String = cookieValue(searchCookieHeader(session,url),"webid")
            .takeIf { it.matches(Regex("[0-9]{1,20}")) && it.any { digit -> digit!='0' } }.orEmpty()
        session=session.copy(webId=webId().ifBlank { session.webId })
        val cookie=searchCookieHeader(session,url)
        if(session.webId.isBlank() || cookieValue(cookie,"ttwid").isBlank() || cookieValue(cookie,"msToken").isBlank()) {
            val html=searchRequest("https://www.douyin.com/",session,mapOf("User-Agent" to DOUYIN_UA,
                "Accept" to "text/html,application/xhtml+xml","Referer" to referer))
            session=currentSearchSession(credentials.douyinSnapshot())
            session=session.copy(webId=webId().ifBlank { DouyinSession.visitor(html)?.id.orEmpty() }.ifBlank { session.webId })
        }
        if(session.webId.isBlank() || cookieValue(searchCookieHeader(session,url),"ttwid").isBlank()) {
            searchSession=null
            throw PlatformException("抖音搜索会话初始化失败，请稍后重试；如官方页面要求验证，请先完成验证")
        }
        if(cookieValue(searchCookieHeader(session,url),"msToken").isBlank()) {
            // Reuse the existing token fallback for this session, never regenerate it per search.
            val alphabet="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            val random=SecureRandom()
            val token=(0 until 107).map { alphabet[random.nextInt(alphabet.length)] }.joinToString("")
            session=session.copy(fallbackToken=token)
        }
        checkSearchSession(session.generation)
        searchSession=session
        return session
    }

    private suspend fun authenticatedJson(url: String,headers: Map<String,String>,snapshot: DouyinCredentialSnapshot): JSONObject =
        JSONObject(net.request(url,headers,onHeaders={ credentials.updateDouyinCookies(snapshot,url,it.values("Set-Cookie")) }).toString(Charsets.UTF_8))

    suspend fun roomPage(room: Room,refreshGuest: Boolean): Pair<String,Map<String,String>> {
        val url="https://live.douyin.com/${room.id}"
        val snapshot=credentials.douyinSnapshot()
        val initial=authHeaders(refreshGuest,url,snapshot)
        var cookie=initial["Cookie"].orEmpty()
        val headers=initial+mapOf("Accept" to "text/html,application/xhtml+xml","Referer" to Platform.DOUYIN.home)
        val html=net.request(url,headers,onHeaders={ response ->
            credentials.updateDouyinCookies(snapshot,url,response.values("Set-Cookie"))
            cookie=DouyinSession.mergeCookies(cookie,response.values("Set-Cookie"))
        }).toString(Charsets.UTF_8)
        val current=credentials.douyinSnapshot()
        check(current.generation==snapshot.generation) { "抖音会话已改变，请重试" }
        return html to (initial+mapOf("Cookie" to if(current.cookies.isEmpty()) cookie else current.header(url),"Referer" to url))
    }
    override suspend fun categories(): List<Category> = JSONArray(context.assetText("platform/douyin_categories.json")).objects().flatMap { parent ->
        parent.arr("subcategories").objects().map { Category(it.str("href").substringAfterLast('/'), it.str("title"), parent.str("title")) }
    }
    private suspend fun signedData(endpoint: String, values: List<Pair<String,String>>): JSONObject {
        val parameters = query(*values.toTypedArray())
        val signature = withContext(Dispatchers.Default) { DouyinSigning.generate(parameters, DOUYIN_UA) }
        val snapshot=credentials.douyinSnapshot()
        val result = authenticatedJson("$endpoint?$parameters&a_bogus=${encoded(signature)}",authHeaders(url=endpoint,snapshot=snapshot),snapshot)
        val code=result.optInt("status_code", -1)
        if (code != 0) throw PlatformException(if(code == -101) "抖音登录已失效，请重新登录" else "抖音接口请求失败（$code），请稍后重试",code)
        return result.obj("data")
    }
    private fun metadata(value: JSONObject, webId: String): Room {
        val owner=value.optJSONObject("owner") ?: value.obj("anchor")
        return Room(Platform.DOUYIN,owner.str("web_rid").ifBlank { webId },owner.str("nickname",value.str("anchor_name")),value.str("title"),
            value.obj("cover").arr("url_list").optString(0),owner.obj("avatar_thumb").arr("url_list").optString(0),
            when(value.optInt("status")) { 2 -> LiveStatus.LIVE; 4 -> LiveStatus.OFFLINE; else -> LiveStatus.UNKNOWN },
            value.obj("stats").str("user_count_str",value.obj("stats").str("total_user_str")),
            value.str("id_str",value.str("id",webId)),owner.str("id_str",owner.str("id")),
            mapOf("sec_uid" to owner.str("sec_uid"),"douyin_id" to owner.str("unique_id",owner.str("short_id"))) +
                if(value.has("live_type_audio") && !value.isNull("live_type_audio")) mapOf("live_type_audio" to value.optBoolean("live_type_audio").toString()) else emptyMap())
    }
    override suspend fun rooms(category: Category?, page: Int): List<Room> {
        val parts=(category?.id ?: "1_1_1_1010032").split('_')
        val values=listOf("aid" to "6383","app_name" to "douyin_web","live_id" to "1","device_platform" to "web","language" to "zh-CN","enter_from" to "web_homepage_hot","cookie_enabled" to "true","screen_width" to "1920","screen_height" to "1080","browser_language" to "zh-CN","browser_platform" to "MacIntel","browser_name" to "Chrome","browser_version" to "120.0.0.0","count" to "20","offset" to ((page-1)*20).toString(),"partition" to parts.last(),"partition_type" to parts.getOrElse(parts.size-2){"1"},"req_from" to "2","msToken" to "")
        return signedData("https://live.douyin.com/webcast/web/partition/detail/room/v2/",values).arr("data").objects().map { metadata(it.obj("room"),it.str("web_rid")) }
    }
    override suspend fun search(keyword: String, page: Int): List<Room> = searchMode(keyword,page,false)
    suspend fun searchMode(keyword: String, page: Int, accounts: Boolean): List<Room> = searchMutex.withLock {
        try { searchPage(keyword,page,accounts) }
        catch(error: PlatformException) {
            android.util.Log.w("DouyinSearch","mode=${if(accounts) "account" else "live"} http=${error.httpStatus} code=${error.code}")
            throw error
        }
    }
    private suspend fun searchPage(keyword: String,page: Int,accounts: Boolean): List<Room> {
        val endpoint=if(accounts) "discover/search" else "live/search"
        val url="https://www.douyin.com/aweme/v1/web/$endpoint/"
        val referer="https://www.douyin.com/search/${encoded(keyword)}?type=${if(accounts) "user" else "live"}"
        val session=prepareSearchSession(url,referer)
        val token=cookieValue(searchCookieHeader(session,url),"msToken")
        val values=listOf("device_platform" to "webapp","aid" to "6383","channel" to "channel_pc_web","search_channel" to if(accounts) "aweme_user_web" else "aweme_live","keyword" to keyword,"search_source" to if(accounts) "normal_search" else "switch_tab","query_correct_type" to "1","is_filter_search" to "0","from_group_id" to "","offset" to ((page-1)*10).toString(),"count" to "10","pc_client_type" to "1","version_code" to "170400","version_name" to "17.4.0","cookie_enabled" to "true","screen_width" to "1920","screen_height" to "1080","browser_language" to "zh-CN","browser_platform" to "Win32","browser_name" to "QQBrowser","browser_version" to "19.7.6764.400","browser_online" to "true","engine_name" to "Blink","engine_version" to "116.0.5845.97","os_name" to "Windows","os_version" to "10","cpu_core_num" to "12","device_memory" to "8","platform" to "PC","webid" to session.webId,"disable_rs" to "0","need_filter_settings" to "1","list_type" to "single","msToken" to token)
        val parameters=query(*values.toTypedArray())
        val signature=withContext(Dispatchers.Default) { DouyinSigning.generate(parameters,DOUYIN_UA) }
        val body=searchRequest("$url?$parameters&a_bogus=${encoded(signature)}",session,mapOf("User-Agent" to DOUYIN_UA,
            "Accept" to "application/json, text/plain, */*","Origin" to "https://www.douyin.com","Referer" to referer))
        val result=try { JSONObject(body) } catch(error: JSONException) {
            throw PlatformException("抖音搜索返回了无效响应，可能受到平台限制，请稍后重试")
        }
        val code=result.optInt("status_code",-1)
        val list=result.opt(if(accounts) "user_list" else "data")
        val listType=when(list) { null -> "missing"; JSONObject.NULL -> "null"; else -> list.javaClass.simpleName }
        android.util.Log.i("DouyinSearch","mode=${if(accounts) "account" else "live"} code=$code list=$listType received=${(list as? JSONArray)?.length() ?: -1}")
        if(code!=0) throw PlatformException(if(code == -101) "抖音登录已失效，请重新登录" else "抖音搜索被平台拒绝（$code），请稍后重试；如官方页面要求验证，请先完成验证",code)
        val verification=result.opt("verify_check_info")
        val hasVerification=(verification is JSONObject && verification.length()>0) ||
            (verification is String && verification.isNotBlank() && verification.trim() !in listOf("{}","null"))
        if(hasVerification && (list !is JSONArray || list.length()==0)) {
            throw PlatformException("抖音搜索返回了验证信息，暂无法确认结果，请打开官方页面检查后重试")
        }
        if(list !is JSONArray) throw PlatformException("抖音搜索未返回有效结果列表，请稍后重试")
        val rooms=(0 until list.length()).mapNotNull { index ->
            try {
                val entry=list.getJSONObject(index)
                if(accounts) searchAccount(entry) else {
                    val raw=entry.getJSONObject("lives").get("rawdata")
                    val room=when(raw) { is JSONObject -> raw; is String -> JSONObject(raw); else -> throw JSONException("Invalid room") }
                    val realId=searchIdentifier(room,"id_str","id")
                    val webId=searchIdentifier(room.obj("owner"),"web_rid")
                    if(realId.isBlank() && webId.isBlank()) null else metadata(room,webId.ifBlank { realId })
                        .copy(id=webId.ifBlank { realId },realId=realId.ifBlank { webId })
                }
            } catch(error: JSONException) { null }
        }
        android.util.Log.i("DouyinSearch","mode=${if(accounts) "account" else "live"} code=$code received=${list.length()} parsed=${rooms.size}")
        if(list.length()>0 && rooms.isEmpty()) throw PlatformException("抖音搜索结果无法解析，请稍后重试")
        checkSearchSession(session.generation)
        return rooms
    }

    private fun searchIdentifier(value: JSONObject,vararg keys: String): String = keys.asSequence().map { value.str(it) }
        .firstOrNull { it.matches(Regex("[0-9]{1,20}")) && it.any { digit -> digit!='0' } }.orEmpty()

    private fun searchAccount(entry: JSONObject): Room {
        val user=entry.getJSONObject("user_info")
        val userId=searchIdentifier(user,"uid","id_str","id")
        if(userId.isBlank()) throw JSONException("Missing account identity")
        val raw=user.opt("room_data")
        // An optional malformed room payload must not hide an otherwise valid account.
        val info=try { when(raw) { is JSONObject -> raw; is String -> if(raw.isNotBlank()) JSONObject(raw) else JSONObject(); else -> JSONObject() } }
            catch(error: JSONException) { JSONObject() }
        val webId=searchIdentifier(info.obj("owner"),"web_rid")
        val roomId=searchIdentifier(user,"room_id_str","room_id").ifBlank { searchIdentifier(info,"id_str","id") }
        val explicitlyOffline=user.str("room_id_str",user.str("room_id"))=="0" || info.optInt("status")==4
        return Room(Platform.DOUYIN,webId.ifBlank { roomId },user.str("nickname"),info.str("title"),avatar=user.obj("avatar_thumb").arr("url_list").optString(0),
            status=if(info.optInt("status")==2) LiveStatus.LIVE else if(explicitlyOffline) LiveStatus.OFFLINE else LiveStatus.UNKNOWN,
            realId=roomId,userId=userId,extra=mapOf("sec_uid" to user.str("sec_uid"),"douyin_id" to user.str("unique_id",user.str("short_id")),"followers" to user.str("follower_count")))
    }
    private suspend fun sharedRoomData(realId: String): JSONObject = net.json(
        "https://webcast.amemv.com/webcast/room/reflow/info/?type_id=0&live_id=1&room_id=${encoded(realId)}&sec_user_id=&app_id=6383",
        mapOf("User-Agent" to DOUYIN_UA)).path("data","room")

    suspend fun roomData(room: Room): JSONObject {
        require(room.id.matches(Regex("[0-9]+"))) { "抖音房间号必须是数字" }
        var webId=room.id
        if(room.realId.matches(Regex("[0-9]{17,20}"))) {
            val result=sharedRoomData(room.realId)
            if(result.optInt("status")!=4 && result.has("id")) return result
            webId=result.obj("owner").str("web_rid").ifBlank {
                room.id.takeIf { it.length<=16 } ?: throw PlatformException("抖音房间已失效")
            }
        }
        val data=signedData("https://live.douyin.com/webcast/room/web/enter/",listOf("aid" to "6383","app_name" to "douyin_web","live_id" to "1","device_platform" to "web","language" to "zh-CN","browser_language" to "zh-CN","browser_platform" to "Win32","browser_name" to "Chrome","browser_version" to "125.0.0.0","web_rid" to webId,"msToken" to ""))
        val result=data.arr("data").optJSONObject(0) ?: throw PlatformException("抖音没有返回房间数据")
        if(!result.has("owner")) result.put("owner",data.obj("user"))
        val realId=result.str("id_str",result.str("id"))
        if(result.isNull("live_type_audio") && realId.matches(Regex("[0-9]{17,20}"))) {
            try {
                val shared=sharedRoomData(realId)
                if(shared.str("id_str",shared.str("id"))==realId && !shared.isNull("live_type_audio")) {
                    result.put("live_type_audio",shared.optBoolean("live_type_audio"))
                }
            } catch(error: CancellationException) { throw error }
            catch(error: Exception) {
                android.util.Log.w("DouyinRoom","房间类型获取失败：${error.javaClass.simpleName}")
            }
        }
        return result
    }
    override suspend fun detail(room: Room): Room = metadata(roomData(room),room.id)
    override suspend fun playback(room: Room, quality: String?, line: String?): Playback {
        val data=roomData(room); val current=metadata(data,room.id)
        if(current.status!=LiveStatus.LIVE) throw PlatformException("主播未开播")
        val stream=data.obj("stream_url"); val pull=stream.path("live_core_sdk_data","pull_data")
        val sdk=pull.str("stream_data").let { if(it.isBlank()) JSONObject() else JSONObject(it).obj("data") }
        val aliases=mapOf("origin" to "ORIGIN","uhd" to "UHD","hd" to "FULL_HD1","sd" to "HD1","ld" to "SD1","ao" to "AO")
        val qualities=pull.obj("options").arr("qualities").objects().sortedByDescending { it.optInt("level") }.map { Choice(it.str("sdk_key"),it.str("name",it.str("sdk_key"))) }.toMutableList()
        listOf("flv_pull_url","hls_pull_url_map").forEach { name -> stream.obj(name).keys().forEach { key -> if(qualities.none { it.id==key || aliases[it.id]==key }) qualities += Choice(key,key) } }
        data class Source(val quality: Choice,val format: String,val url: String)
        var rejected=false
        val sources=qualities.distinctBy { it.id }.flatMap { candidate ->
            listOf("flv","hls").flatMap { format ->
                val fallback=stream.obj(if(format=="flv") "flv_pull_url" else "hls_pull_url_map")
                val urls=listOf(sdk.path(candidate.id,"main").str(format),
                    sdk.path(candidate.id,"backup").str(format),
                    fallback.str(candidate.id),fallback.str(aliases[candidate.id].orEmpty()))
                urls.filter { it.isNotBlank() }.distinct().mapNotNull { url ->
                    try { Source(candidate,format,validateStream(Platform.DOUYIN,url)) }
                    catch(error: StreamAddressException) { rejected=true;null }
                }
            }
        }
        if(sources.isEmpty()) {
            if(rejected) throw StreamAddressException()
            throw PlatformException("抖音没有返回可用的播放地址，请刷新")
        }
        val available=sources.map { it.quality }.distinctBy { it.id }
        val selected=available.firstOrNull { it.id==quality || it.name==quality } ?: available.first()
        val formats=sources.filter { it.quality.id==selected.id }.groupBy { it.format }.flatMap { (format,variants) ->
            variants.mapIndexed { index,source ->
                Choice(if(index==0) format else "$format:${index+1}",if(index==0) format.uppercase() else "${format.uppercase()} ${index+1}") to source.url
            }
        }
        val chosen=formats.firstOrNull { it.first.id==line } ?: formats.first()
        return Playback(current,chosen.second,platformHeaders(Platform.DOUYIN) + ("User-Agent" to DOUYIN_UA),available,formats.map { it.first },selected.id,chosen.first.id)
    }
}
