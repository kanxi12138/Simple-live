package com.simplelive.nativeapp

import android.content.Context
import org.json.JSONObject

/** Platform signing scripts have no Java bridge and run only on worker threads. */
class SignatureEngine(private val context: Context) {
    suspend fun evaluate(scripts: List<String>, expression: String): String = ScriptSandbox.evaluate(scripts, expression)
    suspend fun douyu(source: String, room: String, device: String, seconds: Long): String = evaluate(
        listOf(context.assetText("platform/cryptojs.min.js"), source),
        "ub98484234(${JSONObject.quote(room)},${JSONObject.quote(device)},$seconds)",
    )
    suspend fun douyin(parameter: String): String = ScriptSandbox.evaluate(
        listOf(context.assetText("platform/douyin-sign.js")),
        "get_sign(${JSONObject.quote(parameter)})",
        littleEndian = true,
    )
}
