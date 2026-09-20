package com.simplelive.nativeapp

import com.neovisionaries.ws.client.*
import kotlinx.coroutines.*
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.util.concurrent.atomic.AtomicBoolean
import java.util.zip.GZIPInputStream
import org.json.JSONObject

/** Protocol pinned to DTV e44388b24622860b9b0642d5ee29374cb89ed3c0. No Android dependency. */
object DouyinDanmaku {
    val hosts = listOf("webcast3-ws-web-lq.douyin.com", "webcast5-ws-web-lf.douyin.com",
        "webcast5-ws-web-hl.douyin.com", "webcast3-ws-web-hl.douyin.com", "webcast3-ws-web-lf.douyin.com")
    data class Prepared(val query: String, val headers: Map<String, String>)
    class Failure(val phase: String, val host: String, val http: Int? = null, cause: Throwable? = null,
        val responseKind: String = "unknown", val businessCode: Long? = null) :
        Exception("弹幕${phase}失败（$host${http?.let { "，HTTP $it" }.orEmpty()}）" +
            when (responseKind) { "verification_html" -> "：平台要求安全验证"; "html" -> "：返回普通页面"
                "json" -> "：返回JSON错误${businessCode?.let { "（$it）" }.orEmpty()}"; "empty" -> "：响应为空，原因未识别"
                else -> if (http != null) "：拒绝原因未识别" else "" }, cause)

    private fun rejection(error: OpeningHandshakeException?): Pair<String, Long?> {
        if (error == null) return "unknown" to null
        val bytes = error.body ?: return "empty" to null
        if (bytes.isEmpty()) return "empty" to null
        val sample = bytes.copyOfRange(0, minOf(bytes.size, 8192)).toString(Charsets.UTF_8).trimStart()
        val contentType = error.headers.entries.firstOrNull { it.key.equals("Content-Type", true) }
            ?.value?.firstOrNull()?.substringBefore(';').orEmpty()
        if (sample.startsWith("{")) {
            val json = try { JSONObject(sample) } catch (_: org.json.JSONException) { null }
            if (json != null) {
                val code = listOf("code", "error_code", "status_code").firstNotNullOfOrNull { key ->
                    json.opt(key)?.toString()?.toLongOrNull()
                }
                return "json" to code
            }
        }
        if (contentType.equals("text/html", true) || sample.startsWith("<!doctype html", true) || sample.startsWith("<html", true)) {
            val verification = sample.contains("secsdk-captcha") || sample.contains("verifycenter")
            return (if (verification) "verification_html" else "html") to null
        }
        return "unknown" to null
    }

    suspend fun prepare(roomId: String, visitorId: String, headers: Map<String, String>, sign: suspend (String) -> String): Prepared {
        require(roomId.matches(Regex("[0-9]{17,20}")) && visitorId.matches(Regex("[0-9]+")))
        val now = System.currentTimeMillis()
        val params = linkedMapOf("app_name" to "douyin_web", "version_code" to "180800",
            "webcast_sdk_version" to "1.0.14-beta.0", "update_version_code" to "1.0.14-beta.0",
            "compress" to "gzip", "device_platform" to "web", "cookie_enabled" to "true",
            "screen_width" to "1536", "screen_height" to "864", "browser_language" to "zh-CN",
            "browser_platform" to "Win32", "browser_name" to "Mozilla",
            "browser_version" to "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            "browser_online" to "true", "tz_name" to "Asia/Shanghai",
            "cursor" to "d-1_u-1_fh-7392091211001140287_t-${now}_r-1",
            "internal_ext" to "internal_src:dim|wss_push_room_id:$roomId|wss_push_did:$visitorId|first_req_ms:${now-100}|fetch_time:$now|seq:1|wss_info:0-$now-0-0|wrds_v:7392094459690748497",
            "host" to "https://live.douyin.com", "aid" to "6383", "live_id" to "1", "did_rule" to "3",
            "endpoint" to "live_pc", "support_wrds" to "1", "user_unique_id" to visitorId,
            "im_path" to "/webcast/im/fetch/", "identity" to "audience", "need_persist_msg_count" to "15",
            "insert_task_id" to "", "live_reason" to "", "room_id" to roomId, "heartbeatDuration" to "0")
        val fields = listOf("live_id", "aid", "version_code", "webcast_sdk_version", "room_id", "sub_room_id",
            "sub_channel_id", "did_rule", "user_unique_id", "device_platform", "device_type", "ac", "identity")
        val signature = sign(md5(fields.joinToString(",") { "$it=${params[it].orEmpty()}" }))
        require(signature.isNotBlank() && signature != "undefined") { "弹幕签名为空" }
        params["signature"] = signature
        return Prepared(query(*params.toList().toTypedArray()), headers + mapOf(
            "Origin" to "https://live.douyin.com", "Accept" to "application/json, text/plain, */*",
            "Accept-Language" to "zh-CN,zh;q=0.9,en;q=0.8", "Cache-Control" to "no-cache", "Pragma" to "no-cache"))
    }

    /** Five handshakes per round; only an established connection permits another round. */
    suspend fun listen(prepare: suspend () -> Prepared, message: (String, String) -> Unit,
        status: (String) -> Unit, diagnostic: (String) -> Unit) {
        var preferred: String? = null
        var last: Exception? = null
        for (round in 0..2) {
            val config = prepare()
            var established = false
            for (host in listOfNotNull(preferred) + hosts.filter { it != preferred }) {
                currentCoroutineContext().ensureActive()
                status("正在连接弹幕（$host）")
                try {
                    session(config, host, { established = true; preferred = host }, message, status, diagnostic)
                } catch (error: CancellationException) { throw error }
                catch (error: Exception) {
                    last = if (error is Failure) error else Failure("连接", host, cause = error)
                    status(last.message.orEmpty())
                    if (established) break
                }
            }
            if (!established || round == 2) break
            delay(1500L * (round + 1))
        }
        status((last?.message ?: "弹幕连接已关闭") + "；可点击重连弹幕")
    }

    suspend fun session(config: Prepared, host: String, connected: () -> Unit,
        message: (String, String) -> Unit, status: (String) -> Unit, diagnostic: (String) -> Unit) = coroutineScope {
        require(host in hosts)
        val opened = CompletableDeferred<Unit>()
        val ended = CompletableDeferred<Unit>()
        val active = AtomicBoolean(true)
        val failed = AtomicBoolean(false)
        var receivedChat = false
        val socket = WebSocketFactory().setConnectionTimeout(12_000)
            .createSocket("wss://$host/webcast/im/push/v2/?${config.query}")
            .addExtension("permessage-deflate; client_max_window_bits")
        config.headers.forEach { (name, value) -> if (value.isNotBlank()) socket.addHeader(name, value) }
        fun fail(phase: String, error: Throwable) {
            if (!active.get() || !failed.compareAndSet(false, true)) return
            val http = (error as? OpeningHandshakeException)?.statusLine?.statusCode
            val refusal = rejection(error as? OpeningHandshakeException)
            diagnostic("phase=failure host=$host http=${http ?: 0} response=${refusal.first} business_code=${refusal.second ?: "none"} type=${error.javaClass.simpleName}")
            val failure = Failure(phase, host, http, error, refusal.first, refusal.second)
            opened.completeExceptionally(failure)
            ended.completeExceptionally(failure)
        }
        socket.addListener(object : WebSocketAdapter() {
            override fun onSendingHandshake(websocket: WebSocket, requestLine: String, headers: List<Array<String>>) {
                fun value(name: String) = headers.firstOrNull { it[0].equals(name, true) }?.get(1).orEmpty()
                val upgrade = value("Upgrade").equals("websocket", true)
                val connection = value("Connection").split(',').any { it.trim().equals("upgrade", true) }
                val path = requestLine == "GET /webcast/im/push/v2/?${config.query} HTTP/1.1"
                diagnostic("phase=request host=$host upgrade=$upgrade connection=$connection path_matches=$path")
            }
            override fun onConnected(websocket: WebSocket, headers: Map<String, List<String>>) {
                if (!active.get()) { websocket.disconnect(); return }
                diagnostic("phase=handshake host=$host http=101")
                opened.complete(Unit)
                status("弹幕握手成功，等待消息")
            }
            override fun onConnectError(websocket: WebSocket, error: WebSocketException) = fail("握手", error)
            override fun onError(websocket: WebSocket, error: WebSocketException) = fail(
                if (!opened.isCompleted) "握手" else if (error.error == WebSocketError.DECOMPRESSION_ERROR) "WebSocket解压" else "连接", error)
            override fun handleCallbackError(websocket: WebSocket, error: Throwable) = fail("回调", error)
            override fun onDisconnected(websocket: WebSocket, server: WebSocketFrame?, client: WebSocketFrame?, byServer: Boolean) {
                fail("连接关闭", java.io.IOException())
            }
            // nv-websocket-client automatically replies to server Ping with the same payload.
            override fun onFrameSent(websocket: WebSocket, frame: WebSocketFrame) {
                if (!active.get()) return
                if (frame.isPingFrame) diagnostic("phase=heartbeat_sent host=$host")
                if (frame.isBinaryFrame) diagnostic("phase=ack_sent host=$host")
            }
            override fun onBinaryMessage(websocket: WebSocket, binary: ByteArray) {
                if (!active.get() || ended.isCompleted) return
                var phase = "外层帧解码"
                try {
                    require(binary.size <= 4 * 1024 * 1024)
                    diagnostic("phase=binary host=$host")
                    val frame = Proto(binary)
                    val raw = frame.bytes(8)
                    if (frame.text(7) != "msg" || raw.isEmpty()) return
                    phase = "解压"
                    val payload = if (frame.text(6) == "gzip" || (raw.size >= 2 && raw[0] == 0x1f.toByte() && raw[1] == 0x8b.toByte())) {
                        GZIPInputStream(ByteArrayInputStream(raw)).use { input ->
                            val output = ByteArrayOutputStream()
                            val buffer = ByteArray(8192)
                            while (true) {
                                val count = input.read(buffer)
                                if (count < 0) break
                                require(output.size() + count <= 4 * 1024 * 1024)
                                output.write(buffer, 0, count)
                            }
                            output.toByteArray()
                        }
                    } else raw
                    phase = "消息解码"
                    val response = Proto(payload)
                    if (response.number(9) != 0L) websocket.sendBinary(Proto.encode(
                        mapOf(2 to frame.number(2)), mapOf(7 to "ack".toByteArray(), 8 to response.bytes(5))))
                    response.fields[1].orEmpty().forEach { item ->
                        val chat = try {
                            val envelope = Proto(item)
                            if (envelope.text(1) == "WebcastChatMessage") Proto(envelope.bytes(2)) else null
                        } catch (error: Exception) {
                            diagnostic("phase=chat_decode host=$host type=${error.javaClass.simpleName}")
                            null
                        }
                        if (chat != null) {
                            val decoded = try { chat.child(2).text(3) to chat.text(3) }
                            catch (error: Exception) {
                                diagnostic("phase=chat_decode host=$host type=${error.javaClass.simpleName}")
                                null
                            }
                            if (decoded != null && decoded.second.isNotBlank() && active.get()) {
                                message(decoded.first, decoded.second)
                                diagnostic("phase=chat host=$host")
                                if (!receivedChat) { receivedChat = true; status("弹幕已连接，已收到弹幕") }
                            }
                        }
                    }
                } catch (error: Exception) { fail(phase, error) }
            }
        })
        try {
            socket.connectAsynchronously()
            try { withTimeout(12_000) { opened.await() } }
            catch (error: TimeoutCancellationException) {
                diagnostic("phase=failure stage=handshake_timeout host=$host type=TimeoutCancellationException")
                throw Failure("握手超时", host, cause = error)
            }
            connected()
            val heartbeat = launch {
                while (isActive && !ended.isCompleted) {
                    socket.sendPing(Proto.encode(emptyMap(), mapOf(7 to "hb".toByteArray())))
                    delay(5000)
                }
            }
            try { ended.await() } finally { heartbeat.cancel() }
        } finally {
            active.set(false)
            socket.disconnect(1000, null, 0)
            try { socket.socket?.close() } catch (_: java.io.IOException) { /* Already closed by the reader. */ }
        }
    }
}
