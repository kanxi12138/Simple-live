package com.simplelive.nativeapp

import android.content.Context
import android.util.Base64
import org.json.JSONArray
import org.json.JSONObject
import java.net.URLDecoder
import java.nio.ByteBuffer

private const val HUYA_UA = "HYSDK(Windows,30000002)_APP(pc_exe&7080000&official)_SDK(trans&2.34.0.5795)"
class HuyaPlatform(private val context: Context, private val net: Network) : LivePlatform {
    override suspend fun categories(): List<Category> = JSONArray(context.assetText("platform/huya_categories.json")).objects().flatMap { parent ->
        parent.arr("subcategories").objects().map { Category(it.str("id"), it.str("title"), parent.str("title")) }
    }
    override suspend fun rooms(category: Category?, page: Int): List<Room> {
        val result = net.json("https://live.huya.com/liveHttpUI/getLiveList?iGid=${encoded(category?.id ?: "0")}&iPageNo=$page&iPageSize=20")
        val list=result.optJSONArray("vList") ?: result.obj("data").optJSONArray("vList") ?: throw PlatformException("虎牙没有返回直播列表")
        return list.objects().map { Room(Platform.HUYA, it.str("lProfileRoom"), it.str("sNick"), it.str("sIntroduction"), webUrl(it.str("sScreenshot")), webUrl(it.str("sAvatar180")), LiveStatus.LIVE, it.str("lUserCount")) }
    }
    override suspend fun search(keyword: String, page: Int): List<Room> = net.json("https://search.cdn.huya.com/?" + query(
        "m" to "Search", "do" to "getSearchContent", "q" to keyword, "uid" to "0", "v" to "1", "typ" to "-5", "livestate" to "0", "rows" to "20", "start" to ((page - 1) * 20).toString()), platformHeaders(Platform.HUYA))
        .path("response", "1").arr("docs").objects().filter { it.str("room_id").isNotEmpty() }.map {
            Room(Platform.HUYA, it.str("room_id"), cleanText(it.str("game_nick")), cleanText(it.str("live_intro")), avatar = webUrl(it.str("game_avatarUrl180")),
                status = if (it.optBoolean("gameLiveOn")) LiveStatus.LIVE else LiveStatus.OFFLINE)
        }
    suspend fun page(room: Room): Pair<JSONObject, Map<String, String>> {
        require(room.id.matches(Regex("[A-Za-z0-9]+"))) { "虎牙房间号无效" }
        val html = net.text("https://m.huya.com/${room.id}", mapOf("User-Agent" to MOBILE_UA))
        val raw = Regex("window\\.HNF_GLOBAL_INIT\\s*=\\s*(\\{.*?\\})\\s*;?\\s*</script>", RegexOption.DOT_MATCHES_ALL)
            .find(html)?.groupValues?.get(1) ?: throw PlatformException("虎牙页面缺少房间数据")
        val value = JSONObject(raw.replace(Regex("function\\s*[^({]*\\([^)]*\\)\\s*\\{.*?\\}", RegexOption.DOT_MATCHES_ALL), "\"\""))
        val ids = listOf("lChannelId", "lSubChannelId").associateWith { field ->
            Regex("\"$field\"\\s*:\\s*\"?(\\d+)").find(html)?.groupValues?.get(1).orEmpty()
        }
        return value.obj("roomInfo") to ids
    }
    private fun metadata(room: Room, info: JSONObject, ids: Map<String, String>): Room = room.copy(
        name = info.obj("tProfileInfo").str("sNick", room.name), avatar = webUrl(info.obj("tProfileInfo").str("sAvatar180", room.avatar)),
        title = info.obj("tLiveInfo").str("sIntroduction", room.title),
        status = if (info.optInt("eLiveStatus") == 2) LiveStatus.LIVE else LiveStatus.OFFLINE,
        extra = ids + ("yyid" to info.obj("tLiveInfo").str("lYyid")),
    )
    override suspend fun detail(room: Room): Room { val (info, ids) = page(room); return metadata(room, info, ids) }
    private suspend fun token(base: String, stream: String): String {
        val request = TarsWriter().struct(0) {
            text(0, base); text(1, stream); number(2, 0)
            struct(3) { number(0, 0); text(1, ""); text(2, ""); text(3, "pc_exe&7060000&official"); text(4, ""); number(5, 0); text(6, ""); text(7, "") }
            number(4, 66)
        }.build()
        val payload = TarsWriter().stringBytesMap(0, mapOf("tReq" to request)).build()
        val packet = TarsWriter().number(1, 3).number(2, 0).number(3, 0).number(4, 1)
            .text(5, "liveui").text(6, "getCdnTokenInfoEx").blob(7, payload).number(8, 0)
            .stringBytesMap(9, emptyMap()).stringBytesMap(10, emptyMap()).build()
        val body = ByteBuffer.allocate(packet.size + 4).putInt(packet.size + 4).put(packet).array()
        val response = net.request("http://wup.huya.com", mapOf("User-Agent" to HUYA_UA, "Referer" to "https://m.huya.com/", "Origin" to "https://m.huya.com"), body, "application/octet-stream")
        val offset = if (response.size >= 4 && ByteBuffer.wrap(response).int in 4..response.size) 4 else 0
        val fields = TarsReader(response.copyOfRange(offset, response.size)).fields()
        val data = fields.blob(7).let { if(it.isEmpty()) fields.blob(6) else it }
        val values = (TarsReader(data).fields()[0] as? TarsValue.MapValue)?.value ?: throw PlatformException("虎牙令牌响应无效")
        val result = values.firstOrNull { (it.first as? TarsValue.Text)?.value == "tRsp" }?.second
        val bytes = when (result) {
            is TarsValue.Bytes -> result.value
            is TarsValue.MapValue -> (result.value.firstOrNull()?.second as? TarsValue.Bytes)?.value
            else -> null
        } ?: throw PlatformException("虎牙令牌缺失")
        return TarsReader(bytes).fields().struct(0).text(0).ifBlank { throw PlatformException("虎牙令牌为空") }
    }
    private fun sign(stream: String, presenter: Long, token: String): String {
        val params = token.replace("&amp;", "&").trimStart('?', '&').split('&').associate {
            val parts = it.split('=', limit = 2); URLDecoder.decode(parts[0], "UTF-8") to URLDecoder.decode(parts.getOrElse(1) { "" }, "UTF-8")
        }
        val encodedPrefix = params["fm"] ?: return token
        val prefix = Base64.decode(URLDecoder.decode(encodedPrefix, "UTF-8"), Base64.DEFAULT).toString(Charsets.UTF_8).substringBefore('_')
        val type = params["ctype"] ?: "huya_pc_exe"
        val platform = params["t"] ?: "0"
        val time = params["wsTime"] ?: throw PlatformException("虎牙签名时间缺失")
        val sequence = presenter + System.currentTimeMillis()
        val rotated = (presenter and -4294967296L) or (Integer.rotateLeft(presenter.toInt(), 8).toLong() and 0xffffffffL)
        val uid = if (platform == "103") presenter else rotated
        val secret = md5("${prefix}_${uid}_${stream}_${md5("$sequence|$type|$platform")}_$time")
        val pairs = mutableListOf("wsSecret" to secret, "wsTime" to time, "seqid" to "$sequence", "ctype" to type, "ver" to "1", "fs" to params.getValue("fs"), "fm" to encodedPrefix, "t" to platform)
        if (platform == "103") { pairs += "uid" to "$presenter"; pairs += "uuid" to (java.security.SecureRandom().nextInt().toLong() and 0xffffffffL).toString() } else pairs += "u" to "$rotated"
        return query(*pairs.toTypedArray())
    }
    override suspend fun playback(room: Room, quality: String?, line: String?): Playback {
        val (info, ids) = page(room)
        val current = metadata(room, info, ids)
        if (current.status != LiveStatus.LIVE) throw PlatformException("主播未开播")
        val live = info.path("tLiveInfo", "tLiveStreamInfo")
        val sources = live.obj("vStreamInfo").arr("value").objects().filter { it.str("sFlvUrl").isNotBlank() && it.str("sStreamName").isNotBlank() }
            .sortedBy { if (it.str("sCdnType").startsWith("TX")) 0 else 1 }
        val lines = sources.map { Choice(it.str("sCdnType"), it.str("sCdnType")) }.distinctBy { it.id }
        val qualities = live.obj("vBitRateInfo").arr("value").objects().filterNot { it.str("sDisplayName").contains("HDR") }
            .map { Choice(it.str("iBitRate"), it.str("sDisplayName")) }
        val selected = sources.firstOrNull { it.str("sCdnType") == line } ?: sources.firstOrNull() ?: throw PlatformException("虎牙没有返回线路")
        val chosen = qualities.firstOrNull { it.id == quality || it.name == quality } ?: qualities.firstOrNull() ?: throw PlatformException("虎牙没有返回画质")
        val base = webUrl(selected.str("sFlvUrl")).replace("http://", "https://").trimEnd('/')
        val stream = selected.str("sStreamName")
        validateStream(Platform.HUYA, "$base/$stream.flv")
        val presenter = selected.str("lChannelId").toLongOrNull() ?: ids["lChannelId"]?.toLongOrNull() ?: throw PlatformException("虎牙频道标识缺失")
        val signed = sign(stream, presenter, token(base, stream))
        val url = "$base/$stream.flv?$signed&codec=264" + if (chosen.id != "0") "&ratio=${encoded(chosen.id)}" else ""
        return Playback(current, validateStream(Platform.HUYA, url), platformHeaders(Platform.HUYA) + ("User-Agent" to HUYA_UA), qualities, lines, chosen.id, selected.str("sCdnType"))
    }
}
