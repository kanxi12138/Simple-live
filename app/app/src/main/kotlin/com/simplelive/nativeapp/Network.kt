package com.simplelive.nativeapp

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Response
import org.json.JSONObject
import java.io.IOException
import java.net.URLEncoder
import java.security.MessageDigest
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

const val DESKTOP_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
const val MOBILE_UA = "Mozilla/5.0 (Linux; Android 11; Pixel 5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.91 Mobile Safari/537.36"
fun encoded(value: String): String = URLEncoder.encode(value, "UTF-8").replace("+", "%20")
fun md5(value: String): String = MessageDigest.getInstance("MD5").digest(value.toByteArray()).joinToString("") { "%02x".format(it) }
fun query(vararg pairs: Pair<String, String>): String = pairs.joinToString("&") { "${encoded(it.first)}=${encoded(it.second)}" }

class Network {
    val client: OkHttpClient = OkHttpClient.Builder().connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(25, TimeUnit.SECONDS).callTimeout(35, TimeUnit.SECONDS)
        .followRedirects(false).followSslRedirects(false).build()
    private val limits = Platform.entries.associateWith { Semaphore(2) }

    suspend fun request(url: String, headers: Map<String, String> = emptyMap(), body: ByteArray? = null,
        contentType: String = "application/x-www-form-urlencoded", onHeaders: (okhttp3.Headers)->Unit = {}, requestClient: OkHttpClient = client): ByteArray {
        val host = url.toHttpUrl().host
        val platform = Platform.entries.firstOrNull { host.contains(it.name.lowercase()) }
        suspend fun run(): ByteArray = withContext(Dispatchers.IO) {
            var current = url.toHttpUrl()
            repeat(4) {
                val builder = Request.Builder().url(current).header("User-Agent", DESKTOP_UA)
                    .header("Accept-Language", "zh-CN,zh;q=0.9")
                headers.forEach { (name, value) -> if (value.isNotEmpty()) builder.header(name, value) }
                if (body != null) builder.post(body.toRequestBody(contentType.toMediaType()))
                val response = await(requestClient.newCall(builder.build()))
                response.use {
                    if (it.code in listOf(301, 302, 303, 307, 308)) {
                        val next = current.resolve(it.header("Location").orEmpty()) ?: throw PlatformException("重定向无效")
                        if (next.host != current.host || (current.isHttps && !next.isHttps) || next.port !in listOf(80, 443)) {
                            throw PlatformException("平台重定向不受支持")
                        }
                        current = next
                    } else {
                        if (!it.isSuccessful) throw PlatformException(when(it.code) {
                            401 -> "平台身份验证失败，请重新登录或重试"
                            403 -> "平台拒绝访问，请稍后重试"
                            429 -> "请求过于频繁，请稍后重试"
                            else -> "网络请求失败（${it.code}）"
                        },httpStatus=it.code)
                        onHeaders(it.headers)
                        val source = it.body?.source() ?: throw PlatformException("平台返回空响应")
                        // API payloads and protocol frames are bounded; never buffer an endless live stream.
                        source.request(8L * 1024 * 1024 + 1)
                        if (source.buffer.size > 8L * 1024 * 1024) throw PlatformException("平台响应过大")
                        return@withContext source.readByteArray()
                    }
                }
            }
            throw PlatformException("平台重定向次数过多")
        }
        return if (platform == null) run() else limits.getValue(platform).withPermit { delay(150); run() }
    }
    suspend fun text(url: String, headers: Map<String, String> = emptyMap()): String = request(url, headers).toString(Charsets.UTF_8)
    suspend fun json(url: String, headers: Map<String, String> = emptyMap(), form: String? = null): JSONObject =
        JSONObject(request(url, headers, form?.toByteArray()).toString(Charsets.UTF_8))

    companion object {
        suspend fun await(call: Call): Response = suspendCancellableCoroutine { continuation ->
            continuation.invokeOnCancellation { call.cancel() }
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, error: IOException) { if (continuation.isActive) continuation.resumeWithException(error) }
                override fun onResponse(call: Call, response: Response) {
                    continuation.resume(response) { _, value, _ -> value.close() }
                }
            })
        }
    }
}

fun platformHeaders(platform: Platform): Map<String, String> = mapOf(
    "Referer" to platform.home, "Origin" to platform.home.trimEnd('/'), "User-Agent" to DESKTOP_UA,
)
class StreamAddressException : IOException("平台返回不受支持的播放地址，可切换画质或线路")

fun validateStream(platform: Platform, value: String): String {
    val url = try { webUrl(value).toHttpUrl() } catch(error: IllegalArgumentException) { throw StreamAddressException() }
    val domains = when (platform) {
        Platform.DOUYU -> (1..13).map { if (it == 1) "douyucdn.cn" else "douyucdn$it.cn" }
        Platform.HUYA -> listOf("huya.com", "hycdn.cn", "huyahttpdns.com", "bytefcdnrd.com", "mobgslb.tbcache.com")
        Platform.BILIBILI -> listOf("bilivideo.com", "bilivideo.cn", "biliapi.net")
        Platform.DOUYIN -> listOf("douyincdn.com", "douyincdn.cn", "douyinliving.com", "douyin.com", "bytecdn.cn", "ibytedtos.com", "bytefcdnrd.com")
    }
    if (url.username.isNotEmpty() || url.password.isNotEmpty() || url.port !in listOf(80, 443) ||
        domains.none { url.host == it || url.host.endsWith(".$it") }) {
        android.util.Log.w("StreamPolicy", "${platform.name}: rejected host=${url.host}")
        throw StreamAddressException()
    }
    return url.toString()
}
fun Context.assetText(name: String): String = assets.open(name).bufferedReader().use { it.readText() }
