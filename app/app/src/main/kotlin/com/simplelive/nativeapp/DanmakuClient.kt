package com.simplelive.nativeapp

import kotlinx.coroutines.*
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import org.brotli.dec.BrotliInputStream
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.TimeUnit
import java.util.zip.InflaterInputStream

class DanmakuClient(
    private val net: Network, private val signer: SignatureEngine,
    private val bili: BilibiliPlatform, private val douyin: DouyinPlatform,
) {
    private data class Connection(
        val url: String, val headers: Map<String,String>, val registration: List<ByteArray>,
        val heartbeat: ByteArray?, val interval: Long,
    )
    private class ConnectionFailure(val stage: String,val httpCode: Int?=null,cause: Throwable?=null) :
        Exception("弹幕${stage}失败"+(httpCode?.let { "（HTTP $it）" } ?: ""),cause)
    private suspend fun <T> stage(name: String,block: suspend ()->T): T = try { block() }
        catch(error: CancellationException) { throw error }
        catch(error: Exception) { throw ConnectionFailure(name,(error as? PlatformException)?.httpStatus,error) }
    /** Caller owns the job; cancellation closes the socket and heartbeat before returning. */
    suspend fun listen(room: Room, message: (Danmaku) -> Unit, status: (String) -> Unit) {
        if (room.platform == Platform.DOUYIN) {
            try {
                DouyinDanmaku.listen({ prepareDouyin(room) }, { author,text -> message(Danmaku(author,text)) }, status,
                    { android.util.Log.i("Danmaku", "DOUYIN $it") })
            } catch (error: CancellationException) { throw error }
            catch (error: Exception) {
                android.util.Log.w("Danmaku", "DOUYIN phase=prepare type=${error.javaClass.simpleName}")
                status((error.message ?: "弹幕会话准备失败")+"；可点击重连弹幕")
            }
            return
        }
        var lastFailure="弹幕连接已关闭"
        for (attempt in 0..2) {
            currentCoroutineContext().ensureActive()
            try {
                status(if(attempt==0) "正在连接弹幕" else "弹幕重连中（$attempt/2）")
                session(room,connection(room),message,status)
            } catch (error: CancellationException) { throw error }
            catch (error: Exception) {
                lastFailure=when(error) {
                    is ConnectionFailure -> error.message.orEmpty()
                    is PlatformException -> error.message.orEmpty()
                    else -> "弹幕连接失败（${error.javaClass.simpleName}）"
                }
                android.util.Log.w("Danmaku", "${room.platform.name} attempt=$attempt stage=${(error as? ConnectionFailure)?.stage ?: "connection"} type=${error.cause?.javaClass?.simpleName ?: error.javaClass.simpleName} http=${(error as? ConnectionFailure)?.httpCode ?: 0}")
                status(lastFailure)
                if(attempt==2 || (room.platform!=Platform.DOUYIN && error is PlatformException)) break
            }
            delay((attempt+1)*1500L)
        }
        status(lastFailure+if(room.platform==Platform.DOUYIN) "；可点击重连弹幕" else "；可刷新重试")
    }
    private suspend fun connection(room: Room): Connection = when(room.platform) {
        Platform.DOUYU -> Connection("wss://danmuproxy.douyu.com:8506",platformHeaders(room.platform),listOf(
            douyuPacket("type@=loginreq/roomid@=${room.realId}/"),douyuPacket("type@=joingroup/rid@=${room.realId}/gid@=-9999/")),douyuPacket("type@=mrkl/"),45_000)
        Platform.HUYA -> {
            val registration=TarsWriter().number(0,room.extra.getValue("yyid").toLong()).number(1,1).text(2,"").text(3,"")
                .number(4,room.extra.getValue("lChannelId").toLong()).number(5,room.extra.getValue("lSubChannelId").toLong()).number(6,0).number(7,0).build()
            Connection("wss://cdnws.api.huya.com",platformHeaders(room.platform),listOf(TarsWriter().number(0,1).blob(1,registration).build()),byteArrayOf(0,20,29,0,12,44,54,0,76),60_000)
        }
        Platform.BILIBILI -> {
            val auth=bili.authHeaders()
            val signed=bili.signed(mapOf("id" to room.realId,"type" to "0","web_location" to "444.8"),auth)
            val response=net.json("https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?$signed",auth)
            if(response.optInt("code",-1)!=0) throw PlatformException("B站弹幕认证参数获取失败，请登录后重试")
            val data=response.obj("data"); val host=data.arr("host_list").optJSONObject(0) ?: throw PlatformException("B站弹幕服务器缺失")
            val hostname=host.str("host")
            if(!(hostname.endsWith(".bilibili.com") || hostname.endsWith(".chat.bilibili.com"))) throw PlatformException("B站弹幕服务器无效")
            val cookie=auth["Cookie"].orEmpty()
            val registration=JSONObject().put("uid",cookieValue(cookie,"DedeUserID").toLongOrNull() ?: 0).put("roomid",room.realId.toLong())
                .put("protover",3).put("platform","web").put("type",2).put("buvid",cookieValue(cookie,"buvid3")).put("key",data.getString("token"))
            Connection("wss://$hostname:${host.optInt("wss_port",443)}/sub",auth,listOf(biliPacket(7,registration.toString().toByteArray())),biliPacket(2,"[object Object]".toByteArray()),30_000)
        }
        Platform.DOUYIN -> error("抖音使用独立连接层")
    }
    private suspend fun prepareDouyin(room: Room): DouyinDanmaku.Prepared {
        val prepared=stage("会话准备") {
            require(room.realId.matches(Regex("[0-9]{17,20}"))) { "直播间真实标识无效" }
            var result: Pair<Map<String,String>,DouyinSession.Visitor>?=null
            for(attempt in 0..1) {
                val initial=douyin.authHeaders(refreshGuest=attempt>0)
                var cookie=initial["Cookie"].orEmpty()
                val headers=initial+mapOf("Accept" to "text/html,application/xhtml+xml", "Referer" to "https://live.douyin.com/")
                val html=net.request("https://live.douyin.com/${room.id}",headers,onHeaders={
                    cookie=DouyinSession.mergeCookies(cookie,it.values("Set-Cookie"))
                }).toString(Charsets.UTF_8)
                val visitor=DouyinSession.visitor(html)
                if(visitor!=null && cookieValue(cookie,"ttwid").isNotBlank()) {
                    android.util.Log.i("Danmaku","DOUYIN phase=session identity_source=${visitor.source}")
                    result=(initial+mapOf("Cookie" to cookie,"Referer" to "https://live.douyin.com/${room.id}")) to visitor
                    break
                }
            }
            result ?: throw PlatformException("抖音未返回有效访客会话")
        }
        return stage("签名") { DouyinDanmaku.prepare(room.realId,prepared.second.id,prepared.first) { signer.douyin(it) } }
    }

    private suspend fun session(room: Room, config: Connection, message: (Danmaku)->Unit, status: (String)->Unit) = coroutineScope {
        val disconnected=CompletableDeferred<Unit>()
        var heartbeat: Job?=null
        var pending=byteArrayOf()
        val request=Request.Builder().url(config.url).apply { config.headers.forEach { (name,value)-> if(value.isNotBlank()) header(name,value) } }.build()
        val opened=java.util.concurrent.atomic.AtomicBoolean(false)
        val websocket=net.client.newBuilder().callTimeout(0,TimeUnit.MILLISECONDS).pingInterval(20,TimeUnit.SECONDS).build().newWebSocket(request,object: WebSocketListener() {
            override fun onOpen(socket: WebSocket, response: Response) {
                if(!isActive || disconnected.isCompleted) { socket.cancel();return }
                opened.set(true)
                config.registration.forEach { if(!socket.send(it.toByteString())) disconnected.completeExceptionally(ConnectionFailure("注册")) }
                status("弹幕已连接")
                if(config.heartbeat!=null) heartbeat=launch { while(isActive) { delay(config.interval); config.heartbeat?.let { if(!socket.send(it.toByteString())) disconnected.completeExceptionally(java.io.IOException("心跳发送失败")) } } }
            }
            override fun onMessage(socket: WebSocket, bytes: ByteString) {
                if(!isActive || disconnected.isCompleted) return
                try {
                    if(bytes.size>4*1024*1024) throw PlatformException("弹幕数据过大")
                    when(room.platform) {
                        Platform.DOUYU -> {
                            pending+=bytes.toByteArray()
                            require(pending.size<=4*1024*1024)
                            while(pending.size>=12) {
                                val length=ByteBuffer.wrap(pending).order(ByteOrder.LITTLE_ENDIAN).int
                                require(length in 9..1_048_576)
                                if(pending.size<length+4) break
                                val body=pending.copyOfRange(12,length+3).toString(Charsets.UTF_8)
                                val fields=body.split('/').filter { it.contains("@=") }.associate { it.substringBefore("@=") to it.substringAfter("@=").replace("@S","/").replace("@A","@") }
                                if(fields["type"]=="chatmsg") message(Danmaku(fields["nn"].orEmpty(),fields["txt"].orEmpty()))
                                pending=pending.copyOfRange(length+4,pending.size)
                            }
                        }
                        Platform.HUYA -> {
                            val outer=TarsReader(bytes.toByteArray()).fields()
                            if(outer.number(0)==7L) {
                                val inner=TarsReader(outer.blob(1)).fields()
                                if(inner.number(1)==1400L) { val payload=TarsReader(inner.blob(2)).fields(); val text=payload.text(3); if(text.isNotBlank()) message(Danmaku(payload.struct(0).text(2),text)) }
                            }
                        }
                        Platform.BILIBILI -> decodeBili(bytes.toByteArray(),message,status)
                        Platform.DOUYIN -> Unit
                    }
                } catch(error: Exception) { disconnected.completeExceptionally(error) }
            }
            override fun onFailure(socket: WebSocket, error: Throwable, response: Response?) {
                disconnected.completeExceptionally(ConnectionFailure(if(opened.get()) "连接/心跳" else "握手",response?.code,error)) }
            override fun onClosing(socket: WebSocket, code: Int, reason: String) { socket.close(code,null); disconnected.completeExceptionally(ConnectionFailure("连接关闭（$code）")) }
            override fun onClosed(socket: WebSocket, code: Int, reason: String) { disconnected.completeExceptionally(ConnectionFailure("连接关闭（$code）")) }
        })
        try { disconnected.await() } finally { heartbeat?.cancel(); websocket.cancel() }
    }
    private fun decodeBili(data: ByteArray, message: (Danmaku)->Unit, status: (String)->Unit, depth: Int=0) {
        require(depth<5)
        val input=ByteBuffer.wrap(data).order(ByteOrder.BIG_ENDIAN)
        while(input.remaining()>=16) {
            val start=input.position(); val length=input.int; val header=input.short.toInt() and 65535; val version=input.short.toInt() and 65535; val operation=input.int; input.int
            require(length>=header && header>=16 && length<=data.size-start)
            val body=data.copyOfRange(start+header,start+length); input.position(start+length)
            when {
                version==2 -> decodeBili(inflate(InflaterInputStream(ByteArrayInputStream(body))),message,status,depth+1)
                version==3 -> decodeBili(inflate(BrotliInputStream(ByteArrayInputStream(body))),message,status,depth+1)
                operation==8 -> { if(JSONObject(body.toString(Charsets.UTF_8)).optInt("code",-1)!=0) throw PlatformException("B站弹幕认证失败，请重新登录"); status("弹幕已连接") }
                operation==5 -> {
                    val json=JSONObject(body.toString(Charsets.UTF_8))
                    if(json.str("cmd").substringBefore(':')=="DANMU_MSG") { val info=json.arr("info"); message(Danmaku(info.optJSONArray(2)?.optString(1).orEmpty(),info.optString(1))) }
                }
            }
        }
    }
    private fun inflate(input: InputStream): ByteArray = input.use {
        val output=ByteArrayOutputStream(); val buffer=ByteArray(8192)
        while(true) { val count=it.read(buffer); if(count<0) break; require(output.size()+count<=4*1024*1024); output.write(buffer,0,count) }
        output.toByteArray()
    }
    private fun douyuPacket(text: String): ByteArray { val body=text.toByteArray(); return ByteBuffer.allocate(body.size+13).order(ByteOrder.LITTLE_ENDIAN).putInt(body.size+9).putInt(body.size+9).putShort(689).put(0).put(0).put(body).put(0).array() }
    private fun biliPacket(operation: Int, body: ByteArray): ByteArray = ByteBuffer.allocate(16+body.size).putInt(16+body.size).putShort(16).putShort(1).putInt(operation).putInt(1).put(body).array()
}
