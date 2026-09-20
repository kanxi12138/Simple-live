package com.simplelive.nativeapp

import org.json.JSONObject
import java.util.UUID

// Platform behavior ported from the archived DTV/dart_simple_live adapters; see notices.
class DouyuPlatform(private val net: Network, private val signer: SignatureEngine) : LivePlatform {
    private val headers = platformHeaders(Platform.DOUYU)
    private suspend fun data(url: String, form: String? = null): JSONObject {
        val result = net.json(url, headers, form)
        if (result.optInt("error", result.optInt("code", 0)) != 0) throw PlatformException("斗鱼请求失败：${result.str("msg")}")
        return result.obj("data")
    }
    override suspend fun categories(): List<Category> {
        val result = data("https://m.douyu.com/api/cate/list")
        val parents = result.arr("cate1Info").objects().associate { it.str("cate1Id") to it.str("cate1Name") }
        return result.arr("cate2Info").objects().map { Category(it.str("cate2Id"), it.str("cate2Name"), parents[it.str("cate1Id")].orEmpty(),queryId=it.str("shortName",it.str("cate2Id"))) }
    }
    suspend fun children(category: Category): List<Category> {
        val result = net.json("https://capi.douyucdn.cn/api/v1/getThreeCate?tag_id=${encoded(category.id)}&client_sys=android")
        if (result.optInt("error") != 0) throw PlatformException("斗鱼子分类获取失败")
        return result.arr("data").objects().map { Category(it.str("id",it.str("tagId",it.str("cateId"))), it.str("name",it.str("tagName",it.str("cateName"))), category.id, 3, category.queryId) }
    }
    override suspend fun rooms(category: Category?, page: Int): List<Room> {
        if (category?.level == 3) return data("https://www.douyu.com/gapi/rkc/directory/mixListV1/3_${encoded(category.id)}/$page?limit=20")
            .arr("rl").objects().map { Room(Platform.DOUYU, it.str("rid"), it.str("nn"), it.str("rn"), webUrl(it.str("rs16")), webUrl(it.str("av")), LiveStatus.LIVE, it.str("ol")) }
        val selected=category ?: throw PlatformException("请先选择斗鱼分类")
        return data("https://m.douyu.com/hgapi/live/cate/newRecList?offset=${(page - 1) * 20}&cate2=${encoded(selected.queryId)}&limit=20")
            .arr("list").objects().map { Room(Platform.DOUYU, it.str("rid"), it.str("nickname"), it.str("roomName"), webUrl(it.str("roomSrc")), webUrl(it.str("avatar")), LiveStatus.LIVE, it.str("hn")) }
    }
    override suspend fun search(keyword: String, page: Int): List<Room> {
        val cookie = UUID.randomUUID().toString().replace("-", "")
        suspend fun request(endpoint: String): JSONObject {
            val result = net.json("https://www.douyu.com/japi/search/api/$endpoint?kw=${encoded(keyword)}&page=$page&pageSize=20&filterType=1", headers + ("Cookie" to "dy_did=$cookie; acf_did=$cookie"))
            if (result.optInt("error", -1) != 0) throw PlatformException("斗鱼搜索失败")
            return result.obj("data")
        }
        val live = request("searchShow").arr("relateShow").objects().map {
            Room(Platform.DOUYU, it.str("rid"), cleanText(it.str("nickName")), cleanText(it.str("roomName")), webUrl(it.str("roomSrc")), status = LiveStatus.LIVE)
        }
        val anchors = request("searchUser").arr("relateUser").objects().map { it.obj("anchorInfo") }.filter { it.str("rid").isNotBlank() }.map {
            Room(Platform.DOUYU, it.str("rid"), cleanText(it.str("nickName")), cleanText(it.str("roomName")), avatar = webUrl(it.str("avatar")),
                status = if (it.optInt("isLive") == 1 && it.optInt("videoLoop") != 1 && it.optInt("roomType") == 0) LiveStatus.LIVE else LiveStatus.OFFLINE)
        }
        return (live + anchors).distinctBy { it.key }
    }
    override suspend fun detail(room: Room): Room {
        require(room.id.matches(Regex("[A-Za-z0-9]+"))) { "斗鱼房间号无效" }
        val value = net.json("https://www.douyu.com/betard/${room.id}", headers).obj("room")
        if (value.str("room_id").isEmpty()) throw PlatformException("斗鱼房间数据缺失")
        return room.copy(realId = value.str("room_id"), name = value.str("nickname", value.str("owner_name",room.name)), title = value.str("room_name", room.title),
            avatar = webUrl(value.str("avatar_mid", value.str("owner_avatar",room.avatar))), cover = webUrl(value.str("room_pic", room.cover)),
            status = if (value.optInt("show_status") == 1 && value.optInt("videoLoop") != 1) LiveStatus.LIVE else LiveStatus.OFFLINE)
    }
    override suspend fun playback(room: Room, quality: String?, line: String?): Playback {
        val current = detail(room)
        if (current.status != LiveStatus.LIVE) throw PlatformException("主播未开播")
        val roomId = current.realId
        val script = data("https://www.douyu.com/swf_api/homeH5Enc?rids=$roomId").str("room$roomId")
        if (script.isBlank()) throw PlatformException("斗鱼没有返回签名脚本")
        val signature = signer.douyu(script, roomId, UUID.randomUUID().toString().replace("-", ""), System.currentTimeMillis() / 1000)
        val endpoint = "https://www.douyu.com/lapi/live/getH5Play/$roomId"
        val options = data(endpoint, "$signature&cdn=&rate=-1&ver=Douyu_223061205&iar=1&ive=1&hevc=0&fa=0")
        val qualities = options.arr("multirates").objects().map { Choice(it.str("rate"), it.str("name")) }
        val lines = options.arr("cdnsWithName").objects().map { Choice(it.str("cdn"), it.str("name", it.str("cdn"))) }.sortedBy { it.id.startsWith("scdn") }
        val selectedQuality = qualities.firstOrNull { it.id == quality || it.name == quality } ?: qualities.firstOrNull() ?: throw PlatformException("斗鱼没有返回画质")
        val selectedLine = lines.firstOrNull { it.id == line } ?: lines.firstOrNull() ?: throw PlatformException("斗鱼没有返回线路")
        val play = data(endpoint, "$signature&cdn=${encoded(selectedLine.id)}&rate=${encoded(selectedQuality.id)}&hevc=0")
        val url = "${play.str("rtmp_url")}/${org.jsoup.parser.Parser.unescapeEntities(play.str("rtmp_live"), false)}"
        return Playback(current, validateStream(Platform.DOUYU, url), headers, qualities, lines, selectedQuality.id, selectedLine.id)
    }
}
