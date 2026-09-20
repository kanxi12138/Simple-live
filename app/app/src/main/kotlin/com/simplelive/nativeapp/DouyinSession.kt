package com.simplelive.nativeapp

import org.json.JSONObject
import org.jsoup.Jsoup
import java.net.URLDecoder

/** Extract identity from the room page, never from a cookie token or a random substitute. */
object DouyinSession {
    data class Visitor(val id: String, val source: String)

    fun visitor(html: String): Visitor? {
        val render = Jsoup.parse(html).getElementById("RENDER_DATA")?.data().orEmpty()
        if (render.isNotBlank()) {
            val decoded = try { URLDecoder.decode(render.replace("+", "%2B"), "UTF-8") }
            catch (_: IllegalArgumentException) { "" }
            val data = try { JSONObject(decoded) } catch (_: org.json.JSONException) { null }
            val odin = data?.optJSONObject("app")?.optJSONObject("odin")
            val id = odin?.optString("user_unique_id").orEmpty()
                .ifBlank { odin?.optString("user_unique_id_str").orEmpty() }
            if (validId(id)) return Visitor(id, "render_data")
        }
        val match = Regex("\"user_unique_id(?:_str)?\"\\s*:\\s*\"?([0-9]{1,20})(?![0-9])")
            .find(html.replace("\\\"", "\""))?.groupValues?.get(1)
        return match?.takeIf(::validId)?.let { Visitor(it, "page_field") }
    }

    private fun validId(value: String): Boolean = value.matches(Regex("[0-9]{1,20}")) && value.any { it != '0' }

    fun mergeCookies(initial: String, setCookies: List<String>): String {
        val cookies = initial.split(';').map { it.trim() }.filter { it.contains('=') }
            .associate { it.substringBefore('=') to it.substringAfter('=') }.toMutableMap()
        setCookies.forEach { value ->
            val pair = value.substringBefore(';').trim()
            val name = pair.substringBefore('=')
            if (name in listOf("ttwid", "msToken", "__ac_nonce", "tt_scid", "s_v_web_id") && pair.contains('=')) {
                cookies[name] = pair.substringAfter('=')
            }
        }
        return cookies.entries.joinToString("; ") { "${it.key}=${it.value}" }
    }
}
