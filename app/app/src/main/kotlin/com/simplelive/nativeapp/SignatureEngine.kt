package com.simplelive.nativeapp

import android.content.Context
import org.json.JSONObject

/** Platform signing scripts have no Java bridge and run only on worker threads. */
class SignatureEngine(private val context: Context) {
    suspend fun douyin(parameter: String): String = ScriptSandbox.evaluate(
        listOf(context.assetText("platform/douyin-sign.js")),
        "get_sign(${JSONObject.quote(parameter)})",
        littleEndian = true,
    )
}
