package com.simplelive.nativeapp

import org.json.JSONArray
import org.json.JSONObject

enum class Platform(val label: String, val home: String) {
    DOUYU("斗鱼", "https://www.douyu.com/"),
    HUYA("虎牙", "https://www.huya.com/"),
    DOUYIN("抖音", "https://live.douyin.com/"),
    BILIBILI("哔哩哔哩", "https://live.bilibili.com/");
}
data class Category(val id: String, val name: String, val parent: String = "", val level: Int = 2, val queryId: String = id)
enum class LiveStatus { LIVE, OFFLINE, REPLAY, UNKNOWN }
data class Room(
    val platform: Platform, val id: String, val name: String, val title: String = "",
    val cover: String = "", val avatar: String = "", val status: LiveStatus = LiveStatus.UNKNOWN,
    val viewers: String = "", val realId: String = id, val userId: String = "",
    val extra: Map<String, String> = emptyMap(),
) {
    val key: String get() = "${platform.name}:$id"
}
data class Choice(val id: String, val name: String)
data class Playback(
    val room: Room, val url: String, val headers: Map<String, String>,
    val qualities: List<Choice>, val lines: List<Choice>, val quality: String, val line: String,
)
private val danmakuSequence=java.util.concurrent.atomic.AtomicLong()
data class Danmaku(val user: String, val text: String, val arrivedAt: Long = android.os.SystemClock.elapsedRealtime(), val id: Long=danmakuSequence.incrementAndGet())
interface LivePlatform {
    suspend fun categories(): List<Category>
    suspend fun rooms(category: Category?, page: Int): List<Room>
    suspend fun search(keyword: String, page: Int): List<Room>
    suspend fun detail(room: Room): Room
    suspend fun playback(room: Room, quality: String?, line: String?): Playback
}
fun JSONObject.obj(key: String): JSONObject = optJSONObject(key) ?: JSONObject()
fun JSONObject.arr(key: String): JSONArray = optJSONArray(key) ?: JSONArray()
fun JSONObject.str(key: String, fallback: String = ""): String =
    if (isNull(key)) fallback else optString(key, fallback)
fun JSONArray.objects(): List<JSONObject> = (0 until length()).mapNotNull { optJSONObject(it) }
fun JSONArray.strings(): List<String> = (0 until length()).map { optString(it) }
fun JSONObject.path(vararg keys: String): JSONObject = keys.fold(this) { result, key -> result.obj(key) }
fun cleanText(value: String): String = org.jsoup.Jsoup.parse(value).text()
fun webUrl(value: String): String = if (value.startsWith("//")) "https:$value" else value
open class PlatformException(message: String, val code: Int?=null, val httpStatus: Int?=null) : Exception(message)

fun requiresLogin(platform: Platform, error: Exception): Boolean =
    platform in listOf(Platform.DOUYIN,Platform.BILIBILI) && error is PlatformException &&
        (error.httpStatus==401 || error.code == -101)
