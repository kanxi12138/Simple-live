package com.simplelive.nativeapp

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import java.security.SecureRandom

const val DOUYIN_UA = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.5845.97 Safari/537.36 Core/1.116.567.400 QQBrowser/19.7.6764.400"
fun cookieValue(cookie: String, name: String): String = cookie.split(';').map { it.trim() }.firstOrNull { it.startsWith("$name=") }?.substringAfter('=').orEmpty()

class DouyinPlatform(private val context: Context, private val net: Network, private val credentials: Credentials) : LivePlatform {
    private var guestCookie: String = ""
    suspend fun authHeaders(refreshGuest: Boolean = false): Map<String, String> {
        val saved = credentials.get(Platform.DOUYIN)
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
            if (cookieValue(guestCookie,"ttwid").isBlank()) throw PlatformException("抖音未返回会话信息，请登录后重试",-101)
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
    override suspend fun categories(): List<Category> = JSONArray(context.assetText("platform/douyin_categories.json")).objects().flatMap { parent ->
        parent.arr("subcategories").objects().map { Category(it.str("href").substringAfterLast('/'), it.str("title"), parent.str("title")) }
    }
    private suspend fun signedData(endpoint: String, values: List<Pair<String,String>>): JSONObject {
        val parameters = query(*values.toTypedArray())
        val signature = withContext(Dispatchers.Default) { DouyinSigning.generate(parameters, DOUYIN_UA) }
        val result = net.json("$endpoint?$parameters&a_bogus=${encoded(signature)}", authHeaders())
        val code=result.optInt("status_code", -1)
        if (code != 0) throw PlatformException("抖音接口请求失败（$code），请稍后重试",code)
        return result.obj("data")
    }
    private fun metadata(value: JSONObject, webId: String): Room {
        val owner=value.optJSONObject("owner") ?: value.obj("anchor")
        return Room(Platform.DOUYIN,owner.str("web_rid").ifBlank { webId },owner.str("nickname",value.str("anchor_name")),value.str("title"),
            value.obj("cover").arr("url_list").optString(0),owner.obj("avatar_thumb").arr("url_list").optString(0),
            when(value.optInt("status")) { 2 -> LiveStatus.LIVE; 4 -> LiveStatus.OFFLINE; else -> LiveStatus.UNKNOWN },
            value.obj("stats").str("user_count_str",value.obj("stats").str("total_user_str")),
            value.str("id_str",value.str("id",webId)),owner.str("id_str",owner.str("id")),
            mapOf("sec_uid" to owner.str("sec_uid"),"douyin_id" to owner.str("unique_id",owner.str("short_id"))))
    }
    override suspend fun rooms(category: Category?, page: Int): List<Room> {
        val parts=(category?.id ?: "1_1_1_1010032").split('_')
        val values=listOf("aid" to "6383","app_name" to "douyin_web","live_id" to "1","device_platform" to "web","language" to "zh-CN","enter_from" to "web_homepage_hot","cookie_enabled" to "true","screen_width" to "1920","screen_height" to "1080","browser_language" to "zh-CN","browser_platform" to "MacIntel","browser_name" to "Chrome","browser_version" to "120.0.0.0","count" to "20","offset" to ((page-1)*20).toString(),"partition" to parts.last(),"partition_type" to parts.getOrElse(parts.size-2){"1"},"req_from" to "2","msToken" to "")
        return signedData("https://live.douyin.com/webcast/web/partition/detail/room/v2/",values).arr("data").objects().map { metadata(it.obj("room"),it.str("web_rid")) }
    }
    override suspend fun search(keyword: String, page: Int): List<Room> = searchMode(keyword,page,false)
    suspend fun searchMode(keyword: String, page: Int, accounts: Boolean): List<Room> {
        val auth=authHeaders()
        val webId=cookieValue(auth["Cookie"].orEmpty(),"webid").ifBlank { "738${System.currentTimeMillis()}0" }
        val values=listOf("device_platform" to "webapp","aid" to "6383","channel" to "channel_pc_web","search_channel" to if(accounts) "aweme_user_web" else "aweme_live","keyword" to keyword,"search_source" to if(accounts) "normal_search" else "switch_tab","query_correct_type" to "1","is_filter_search" to "0","from_group_id" to "","offset" to ((page-1)*10).toString(),"count" to "10","pc_client_type" to "1","version_code" to "170400","version_name" to "17.4.0","cookie_enabled" to "true","screen_width" to "1980","screen_height" to "1080","browser_language" to "zh-CN","browser_platform" to "Win32","browser_name" to "Edge","browser_version" to "125.0.0.0","browser_online" to "true","engine_name" to "Blink","engine_version" to "125.0.0.0","os_name" to "Windows","os_version" to "10","cpu_core_num" to "12","device_memory" to "8","platform" to "PC","webid" to webId,"disable_rs" to "0","need_filter_settings" to "1","list_type" to "single")
        val endpoint=if(accounts) "discover/search" else "live/search"
        val result=net.json("https://www.douyin.com/aweme/v1/web/$endpoint/?${query(*values.toTypedArray())}",auth + ("Referer" to "https://www.douyin.com/search/${encoded(keyword)}?type=${if(accounts) "user" else "live"}"))
        if(result.optInt("status_code",-1)!=0) throw PlatformException("抖音搜索失败，请登录后重试")
        if(!accounts) {
            if(!result.has("data")) throw PlatformException("抖音没有返回搜索列表，请登录后重试")
            return result.arr("data").objects().mapNotNull { entry -> val raw=entry.obj("lives").str("rawdata"); if(raw.isBlank()) null else metadata(JSONObject(raw),"") }.filter { it.id.isNotBlank() }
        }
        if(!result.has("user_list")) throw PlatformException("抖音没有返回账号列表，请登录后重试")
        return result.arr("user_list").objects().map { entry ->
            val user=entry.obj("user_info")
            val raw=user.opt("room_data")
            val info=when(raw) { is JSONObject -> raw; is String -> if(raw.isNotBlank()) JSONObject(raw) else JSONObject(); else -> JSONObject() }
            val webIdForRoom=info.obj("owner").str("web_rid")
            val roomId=user.str("room_id_str",user.str("room_id")).takeUnless { it=="0" }.orEmpty()
            Room(Platform.DOUYIN,webIdForRoom.ifBlank { roomId },user.str("nickname"),info.str("title"),avatar=user.obj("avatar_thumb").arr("url_list").optString(0),
                status=if(info.optInt("status")==2) LiveStatus.LIVE else if(roomId.isBlank()) LiveStatus.OFFLINE else LiveStatus.UNKNOWN,
                realId=roomId,userId=user.str("uid"),extra=mapOf("sec_uid" to user.str("sec_uid"),"douyin_id" to user.str("unique_id",user.str("short_id")),"followers" to user.str("follower_count")))
        }
    }
    suspend fun roomData(room: Room): JSONObject {
        require(room.id.matches(Regex("[0-9]+"))) { "抖音房间号必须是数字" }
        var webId=room.id
        if(webId.length>16) {
            val result=net.json("https://webcast.amemv.com/webcast/room/reflow/info/?type_id=0&live_id=1&room_id=${encoded(webId)}&sec_user_id=&app_id=6383",mapOf("User-Agent" to DOUYIN_UA)).path("data","room")
            if(result.optInt("status")!=4 && result.has("id")) return result
            webId=result.obj("owner").str("web_rid").ifBlank { throw PlatformException("抖音房间已失效") }
        }
        val data=signedData("https://live.douyin.com/webcast/room/web/enter/",listOf("aid" to "6383","app_name" to "douyin_web","live_id" to "1","device_platform" to "web","language" to "zh-CN","browser_language" to "zh-CN","browser_platform" to "Win32","browser_name" to "Chrome","browser_version" to "125.0.0.0","web_rid" to webId,"msToken" to ""))
        val result=data.arr("data").optJSONObject(0) ?: throw PlatformException("抖音没有返回房间数据")
        if(!result.has("owner")) result.put("owner",data.obj("user"))
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
