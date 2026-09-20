package com.simplelive.nativeapp

import kotlinx.coroutines.*
import org.mozilla.javascript.Context as JsContext
import org.mozilla.javascript.ContextFactory
import org.json.JSONObject

/** Shared by Android signing and the bounded desktop connection diagnosis. */
object ScriptSandbox {
    suspend fun evaluate(scripts: List<String>, expression: String, littleEndian: Boolean = false): String = withContext(Dispatchers.Default) {
        require(scripts.sumOf { it.length } <= 2 * 1024 * 1024) { "签名脚本过大" }
        val deadline = System.nanoTime() + 3_000_000_000L
        val task = currentCoroutineContext()
        val factory = object : ContextFactory() {
            // Douyin's MD5 shares Uint8Array/Uint32Array storage and assumes V8 byte order.
            override fun hasFeature(cx: JsContext, featureIndex: Int): Boolean =
                if (featureIndex == JsContext.FEATURE_LITTLE_ENDIAN && littleEndian) true else super.hasFeature(cx, featureIndex)
            override fun makeContext(): JsContext = super.makeContext().apply {
                optimizationLevel = -1
                languageVersion = JsContext.VERSION_ES6
                instructionObserverThreshold = 10_000
                maximumInterpreterStackDepth = 256
                setClassShutter { false }
            }
            override fun observeInstructionCount(cx: JsContext, count: Int) {
                task.ensureActive()
                if (System.nanoTime() > deadline || Thread.currentThread().isInterrupted) throw PlatformException("签名计算超时")
            }
        }
        factory.call<String> { js ->
            val scope = js.initSafeStandardObjects()
            listOf("Packages", "java", "javax", "org", "com", "edu", "net", "getClass", "JavaAdapter", "JavaImporter").forEach { scope.delete(it) }
            js.evaluateString(scope, "var globalThis=this; var window=this; var self=this; var document={}; var navigator={userAgent:${JSONObject.quote(DESKTOP_UA)}};", "environment", 1, null)
            scripts.forEachIndexed { index, source -> task.ensureActive(); js.evaluateString(scope, source, "sign$index", 1, null) }
            JsContext.toString(js.evaluateString(scope, expression, "result", 1, null))
        }
    }
}
