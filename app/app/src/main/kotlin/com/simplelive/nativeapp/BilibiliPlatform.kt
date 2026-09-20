package com.simplelive.nativeapp

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

const val BILI_LIST_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36"

// Opaque verification data is held in memory and bound to this exact list request.
class BiliListChallenge(val requestUrl: String, val voucher: String) :
    PlatformException("B站要求安全验证（-352），请完成验证后重试", -352)

class BilibiliPlatform(private val context: Context, private val net: Network, private val credentials: Credentials) : LivePlatform {
    private val headers = platformHeaders(Platform.BILIBILI)
    private val listHeaders = mapOf("User-Agent" to BILI_LIST_UA, "Referer" to "https://www.bilibili.com/", "Cookie" to "buvid3=i;")
    private val listClient = net.client.newBuilder().protocols(listOf(okhttp3.Protocol.HTTP_1_1)).build()
    private val sessionMutex = Mutex()
    private var guestCookie = ""
    suspend fun authHeaders(): Map<String, String> = sessionMutex.withLock {
        var cookie = credentials.get(Platform.BILIBILI)
        if (!cookie.contains("buvid3=") || !cookie.contains("buvid4=")) {
            if (guestCookie.isBlank()) {
                val fingerprint = net.json("https://api.bilibili.com/x/frontend/finger/spi", headers + ("Cookie" to cookie)).obj("data")
                if (fingerprint.str("b_3").isBlank() || fingerprint.str("b_4").isBlank()) throw PlatformException("B站未返回设备标识，请登录后重试")
                guestCookie = "buvid3=${fingerprint.str("b_3")}; buvid4=${fingerprint.str("b_4")}"
            }
            cookie = (cookie.split(';').filterNot { it.trim().startsWith("buvid3=") || it.trim().startsWith("buvid4=") } + guestCookie).filter { it.isNotBlank() }.joinToString("; ")
        }
        headers + ("Cookie" to cookie)
    }
    suspend fun signed(parameters: Map<String, String>, auth: Map<String, String>): String {
        val data = net.json("https://api.bilibili.com/x/web-interface/nav", auth).path("data", "wbi_img")
        val keys = listOf("img_url", "sub_url").joinToString("") { data.str(it).substringAfterLast('/').substringBefore('.') }
        if (keys.length != 64) throw PlatformException("B站签名参数缺失")
        val indices = intArrayOf(46,47,18,2,53,8,23,32,15,50,10,31,58,3,45,35,27,43,5,49,33,9,42,19,29,28,14,39,12,38,41,13)
        val mixin = indices.map { keys[it] }.joinToString("")
        val values = (parameters + ("wts" to (System.currentTimeMillis() / 1000).toString())).toSortedMap()
        val query = values.entries.joinToString("&") { "${encoded(it.key)}=${encoded(it.value.filterNot { char -> char in "!'()*" })}" }
        return "$query&w_rid=${md5(query + mixin)}"
    }
    private suspend fun data(url: String, auth: Map<String, String> = headers): JSONObject {
        val result = net.json(url, auth)
        val code = result.optInt("code", -1)
        if (code == -352) throw PlatformException("B站要求安全验证（-352），可登录后重试；若仍失败，请稍后再试", code)
        if (code == -101) throw PlatformException("B站需要登录（-101）", code)
        if (code != 0) throw PlatformException("B站请求失败（$code）：${result.str("message", result.str("msg"))}", code)
        return result.obj("data")
    }
    override suspend fun categories(): List<Category> = JSONArray(context.assetText("platform/bilibili_categories.json")).objects().flatMap { parent ->
        parent.arr("subcategories").objects().map { Category(it.str("id"), it.str("title"), parent.str("id")) }
    }
    override suspend fun rooms(category: Category?, page: Int): List<Room> {
        val selected = category ?: throw PlatformException("请先选择B站分类")
        val html = net.request("https://live.bilibili.com/lol", listHeaders - "Cookie", requestClient=listClient).toString(Charsets.UTF_8)
        val webId = Regex("\"access_id\"\\s*:\\s*\"([^\"]+)\"").find(html)?.groupValues?.get(1)
            ?: throw PlatformException("B站列表标识获取失败，请重试")
        val values = linkedMapOf("area_id" to selected.id, "page" to "$page", "parent_area_id" to selected.parent,
            "platform" to "web", "sort_type" to "", "vajra_business_key" to "", "w_webid" to webId,
            "web_location" to "444.253", "wts" to (System.currentTimeMillis() / 1000).toString())
        // Public list-protocol salt and anonymous cookie marker match DTV; neither is an account credential.
        val signature = md5(values.entries.joinToString("&") { "${it.key}=${it.value}" } + "ea1db124af3c7062474693fa704f4ff8")
        return listResponse("https://api.live.bilibili.com/xlive/web-interface/v1/second/getList?${query(*values.toList().toTypedArray())}&w_rid=$signature", true)
    }
    suspend fun verifiedRooms(challenge: BiliListChallenge, token: String): List<Room> {
        require(token.isNotBlank() && token.length <= 8192) { "验证结果无效" }
        return listResponse(challenge.requestUrl + "&gaia_vtoken=${encoded(token)}", false)
    }
    private suspend fun listResponse(url: String, allowVerification: Boolean): List<Room> {
        var voucher = ""
        val result = JSONObject(net.request(url, listHeaders, onHeaders={ voucher=it["x-bili-gaia-vvoucher"].orEmpty() }, requestClient=listClient).toString(Charsets.UTF_8))
        val code = result.optInt("code", -1)
        if(code == -352) {
            if(voucher.isBlank()) voucher=result.obj("data").str("v_voucher")
            if(allowVerification && voucher.isNotBlank() && voucher.length<=16384) throw BiliListChallenge(url,voucher)
            throw PlatformException(if(allowVerification) "B站限制访问（-352），未提供验证入口，请稍后重试" else "验证后B站仍限制访问（-352），请稍后重试",code)
        }
        if(code != 0) throw PlatformException("B站列表请求失败（$code）",code)
        val response = result.obj("data")
        return response.arr("list").objects().map { Room(Platform.BILIBILI, it.str("roomid"), it.str("uname"), it.str("title"), webUrl(it.str("cover", it.str("user_cover"))), webUrl(it.str("face")), LiveStatus.LIVE, it.str("online"), userId = it.str("uid")) }
    }
    override suspend fun search(keyword: String, page: Int): List<Room> {
        val response = data("https://api.bilibili.com/x/web-interface/search/type?search_type=live&keyword=${encoded(keyword)}&page=$page", authHeaders()).obj("result")
        return (response.arr("live_room").objects().map { it.put("native_live_result",true) } + response.arr("live_user").objects()).map {
            Room(Platform.BILIBILI, it.str("roomid"), cleanText(it.str("uname")), cleanText(it.str("title")), webUrl(it.str("user_cover")), webUrl(it.str("uface")),
                if (it.optInt("live_status", if (it.optBoolean("is_live")) 1 else 0) == 1 || it.optBoolean("native_live_result")) LiveStatus.LIVE else LiveStatus.OFFLINE, it.str("online"), userId = it.str("uid"))
        }.filter { it.id.isNotBlank() && it.id != "0" }.distinctBy { it.key }
    }
    override suspend fun detail(room: Room): Room {
        require(room.id.matches(Regex("[0-9]+"))) { "B站房间号必须是数字" }
        val auth = authHeaders()
        val init = data("https://api.live.bilibili.com/room/v1/Room/room_init?id=${room.id}", auth)
        val realId = init.str("room_id")
        if (realId.isBlank()) throw PlatformException("B站房间号缺失")
        val info = data("https://api.live.bilibili.com/xlive/web-room/v1/index/getInfoByRoom?" + signed(mapOf("room_id" to realId), auth), auth)
        val roomInfo = info.obj("room_info")
        val anchor = info.path("anchor_info", "base_info")
        return room.copy(realId = realId, userId = init.str("uid"), name = anchor.str("uname", room.name), avatar = webUrl(anchor.str("face", room.avatar)),
            title = roomInfo.str("title", room.title), cover = webUrl(roomInfo.str("cover", room.cover)),
            status = when (init.optInt("live_status")) { 1 -> LiveStatus.LIVE; 2 -> LiveStatus.REPLAY; else -> LiveStatus.OFFLINE })
    }
    override suspend fun playback(room: Room, quality: String?, line: String?): Playback {
        val current = detail(room)
        if (current.status != LiveStatus.LIVE) throw PlatformException("主播未开播")
        val auth = authHeaders()
        suspend fun play(selected: String?): JSONObject = data("https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo?room_id=${current.realId}&protocol=0,1&format=0,1,2&codec=0&platform=html5&dolby=5" + (selected?.let { "&qn=${encoded(it)}" } ?: ""), auth).path("playurl_info", "playurl")
        val initial = play(null)
        val descriptions = initial.arr("g_qn_desc").objects().associate { it.str("qn") to it.str("desc") }
        val accepted = initial.arr("stream").objects().flatMap { it.arr("format").objects() }.flatMap { it.arr("codec").objects() }.flatMap { it.arr("accept_qn").strings() }.distinct().sortedByDescending { it.toIntOrNull() ?: 0 }
        val qualities = accepted.map { Choice(it, descriptions[it] ?: it) }
        val chosen = qualities.firstOrNull { it.id == quality || it.name == quality } ?: qualities.firstOrNull() ?: throw PlatformException("B站没有返回画质")
        val result = play(chosen.id)
        data class Source(val id: String, val name: String, val url: String, val quality: String)
        val sources = result.arr("stream").objects().flatMap { stream -> stream.arr("format").objects().flatMap { format ->
            format.arr("codec").objects().filter { it.str("codec_name") == "avc" }.flatMap { codec ->
                codec.arr("url_info").objects().mapIndexed { index, cdn ->
                    Source("${format.str("format_name")}:$index", "${format.str("format_name").uppercase()} ${index + 1}", "${cdn.str("host")}${codec.str("base_url")}${cdn.str("extra")}", codec.str("current_qn"))
                }
            }
        } }.sortedBy { if (it.id.startsWith("flv")) 0 else 1 }
        val selected = sources.firstOrNull { it.id == line } ?: sources.firstOrNull() ?: throw PlatformException("B站没有返回可播放的 AVC 线路")
        return Playback(current, validateStream(Platform.BILIBILI, selected.url), headers, qualities, sources.map { Choice(it.id, it.name) }, selected.quality, selected.id)
    }
}
