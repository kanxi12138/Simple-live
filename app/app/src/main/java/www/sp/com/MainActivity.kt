package www.sp.com

import android.content.Intent
import android.content.pm.ActivityInfo
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebSettings
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.core.content.FileProvider
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.charset.StandardCharsets
import java.util.concurrent.TimeUnit

private const val DOUYU_DANMAKU_URL = "wss://danmuproxy.douyu.com:8506"
private const val HEARTBEAT_INTERVAL_MS = 45_000L
private const val PACKET_HEADER_SIZE = 12
private const val MIN_PACKET_BODY_SIZE = 9

class MainActivity : TauriActivity() {
  private var playerWebView: WebView? = null
  private lateinit var backPressedCallback: OnBackPressedCallback
  private val douyuDanmakuBridge = DouyuDanmakuBridge()

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    backPressedCallback = object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val webView = playerWebView
        if (webView == null) {
          fallbackBackPressed()
          return
        }

        webView.post {
          webView.evaluateJavascript(
            """
              (function () {
                try {
                  return !!(window.__DTV_HANDLE_ANDROID_BACK__ && window.__DTV_HANDLE_ANDROID_BACK__());
                } catch (e) {
                  return false;
                }
              })();
            """.trimIndent(),
          ) { result ->
            if (result != "true") {
              fallbackBackPressed()
            }
          }
        }
      }
    }
    onBackPressedDispatcher.addCallback(this, backPressedCallback)
  }

  override fun onDestroy() {
    douyuDanmakuBridge.stop()
    super.onDestroy()
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    playerWebView = webView
    WebView.setWebContentsDebuggingEnabled(true)
    webView.settings.mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
    webView.settings.allowFileAccessFromFileURLs = true
    webView.settings.allowUniversalAccessFromFileURLs = true
    webView.addJavascriptInterface(OrientationBridge(), "DTVOrientation")
    webView.addJavascriptInterface(UpdateBridge(), "DTVUpdate")
    webView.addJavascriptInterface(DebugBridge(), "DTVDebug")
    webView.addJavascriptInterface(douyuDanmakuBridge, "DTVDouyuDanmaku")
  }

  private fun fallbackBackPressed() {
    backPressedCallback.isEnabled = false
    onBackPressedDispatcher.onBackPressed()
    backPressedCallback.isEnabled = true
  }

  private inner class OrientationBridge {
    @JavascriptInterface
    fun setLandscape() {
      runOnUiThread {
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
      }
    }

    @JavascriptInterface
    fun setPortrait() {
      runOnUiThread {
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
      }
    }
  }

  private inner class UpdateBridge {
    @JavascriptInterface
    fun canRequestPackageInstalls(): Boolean {
      return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        packageManager.canRequestPackageInstalls()
      } else {
        true
      }
    }

    @JavascriptInterface
    fun openUnknownAppSourcesSettings() {
      val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
        data = Uri.parse("package:$packageName")
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      }
      startActivity(intent)
    }

    @JavascriptInterface
    fun installApk(filePath: String) {
      if (filePath.isBlank()) {
        return
      }

      val apkFile = File(filePath)
      if (!apkFile.exists() || !apkFile.isFile) {
        return
      }

      val apkUri = FileProvider.getUriForFile(
        this@MainActivity,
        "$packageName.fileprovider",
        apkFile,
      )

      val installIntent = Intent(Intent.ACTION_VIEW).apply {
        setDataAndType(apkUri, "application/vnd.android.package-archive")
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      }

      startActivity(installIntent)
    }
  }

  private inner class DebugBridge {
    @JavascriptInterface
    fun log(level: String?, message: String?) {
      val safeMessage = (message ?: "").take(3000)
      when ((level ?: "d").lowercase()) {
        "e" -> Log.e("DTVDebug", safeMessage)
        "w" -> Log.w("DTVDebug", safeMessage)
        "i" -> Log.i("DTVDebug", safeMessage)
        else -> Log.d("DTVDebug", safeMessage)
      }
    }
  }

  private inner class DouyuDanmakuBridge {
    private val httpClient = OkHttpClient.Builder()
      .readTimeout(0, TimeUnit.MILLISECONDS)
      .build()
    private val mainHandler = Handler(Looper.getMainLooper())

    private var webSocket: WebSocket? = null
    private var currentRoomId: String? = null
    private var pendingBuffer = ByteArrayOutputStream()

    private val heartbeatRunnable = object : Runnable {
      override fun run() {
        val roomId = currentRoomId ?: return
        sendPacket("type@=mrkl/")
        Log.d("DouyuDanmaku", "heartbeat room=$roomId")
        mainHandler.postDelayed(this, HEARTBEAT_INTERVAL_MS)
      }
    }

    @JavascriptInterface
    fun start(roomId: String?) {
      try {
        val normalizedRoomId = roomId?.trim().orEmpty()
        if (normalizedRoomId.isEmpty()) {
          dispatchDanmakuStatus("error", "", "empty room id")
          return
        }

        if (currentRoomId == normalizedRoomId && webSocket != null) {
          dispatchDanmakuStatus("skip", normalizedRoomId, "already connected")
          return
        }

        stop()
        currentRoomId = normalizedRoomId
        pendingBuffer = ByteArrayOutputStream()
        Log.i("DouyuDanmaku", "start room=$normalizedRoomId")
        dispatchDanmakuStatus("start", normalizedRoomId, "starting native websocket")

        val request = Request.Builder()
          .url(DOUYU_DANMAKU_URL)
          .build()

        webSocket = httpClient.newWebSocket(request, object : WebSocketListener() {
          override fun onOpen(webSocket: WebSocket, response: Response) {
            Log.i("DouyuDanmaku", "open room=$normalizedRoomId")
            dispatchDanmakuStatus("open", normalizedRoomId, "websocket opened")
            sendPacket("type@=loginreq/roomid@=$normalizedRoomId/")
            sendPacket("type@=joingroup/rid@=$normalizedRoomId/gid@=-9999/")
            mainHandler.removeCallbacks(heartbeatRunnable)
            mainHandler.postDelayed(heartbeatRunnable, HEARTBEAT_INTERVAL_MS)
          }

          override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
            dispatchDanmakuStatus("message", normalizedRoomId, "bytes=${bytes.size}")
            handleIncomingBytes(normalizedRoomId, bytes.toByteArray())
          }

          override fun onMessage(webSocket: WebSocket, text: String) {
            dispatchDanmakuStatus("message", normalizedRoomId, "text=${text.length}")
            handleIncomingBytes(normalizedRoomId, text.toByteArray(StandardCharsets.UTF_8))
          }

          override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            Log.e("DouyuDanmaku", "failure room=$normalizedRoomId message=${t.message}")
            dispatchDanmakuStatus("error", normalizedRoomId, t.message ?: "unknown")
          }

          override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
            Log.w("DouyuDanmaku", "closed room=$normalizedRoomId code=$code reason=$reason")
            dispatchDanmakuStatus("closed", normalizedRoomId, "code=$code reason=$reason")
            mainHandler.removeCallbacks(heartbeatRunnable)
          }
        })
      } catch (error: Throwable) {
        Log.e("DouyuDanmaku", "start exception room=${roomId ?: ""} message=${error.message}")
        dispatchDanmakuStatus("error", roomId?.trim().orEmpty(), error.message ?: "unknown start exception")
      }
    }

    @JavascriptInterface
    fun stop() {
      mainHandler.removeCallbacks(heartbeatRunnable)
      webSocket?.close(1000, "client-stop")
      webSocket = null
      currentRoomId = null
      pendingBuffer = ByteArrayOutputStream()
      dispatchDanmakuStatus("stop", "", "client stop")
    }

    private fun sendPacket(body: String) {
      webSocket?.send(ByteString.of(*encodePacket(body)))
    }

    private fun handleIncomingBytes(roomId: String, incoming: ByteArray) {
      pendingBuffer.write(incoming, 0, incoming.size)
      val data = pendingBuffer.toByteArray()
      var cursor = 0

      while (cursor + PACKET_HEADER_SIZE <= data.size) {
        val packetLength = readLittleEndianInt(data, cursor)
        if (packetLength < MIN_PACKET_BODY_SIZE) {
          break
        }

        val frameEnd = cursor + 4 + packetLength
        if (frameEnd > data.size) {
          break
        }

        val bodyLength = packetLength - 9
        val bodyStart = cursor + PACKET_HEADER_SIZE
        val body = String(data, bodyStart, bodyLength, StandardCharsets.UTF_8).trimEnd('\u0000')
        for (part in body.split("//")) {
          val parsed = parseStt(part)
          if (parsed["type"] == "chatmsg" && !parsed["txt"].isNullOrBlank() && !parsed["dms"].isNullOrBlank()) {
            emitDanmaku(roomId, parsed)
          }
        }

        cursor = frameEnd
      }

      val remaining = if (cursor < data.size) data.copyOfRange(cursor, data.size) else ByteArray(0)
      pendingBuffer = ByteArrayOutputStream()
      if (remaining.isNotEmpty()) {
        pendingBuffer.write(remaining, 0, remaining.size)
      }
    }

    private fun emitDanmaku(roomId: String, payload: Map<String, String>) {
      val nickname = payload["nn"].orEmpty()
      val content = payload["txt"].orEmpty()
      val userLevel = payload["level"]?.toIntOrNull() ?: 0
      val fansClubLevel = payload["bl"]?.toIntOrNull() ?: 0
      Log.d("DouyuDanmaku", "chat room=$roomId user=$nickname len=${content.length}")

      val json = JSONObject().apply {
        put("room_id", roomId)
        put("user", nickname)
        put("content", content)
        put("user_level", userLevel)
        put("fans_club_level", fansClubLevel)
      }
      dispatchDanmakuPayload(json.toString())
    }

    private fun dispatchDanmakuStatus(type: String, roomId: String, message: String) {
      val json = JSONObject().apply {
        put("type", type)
        put("room_id", roomId)
        put("message", message)
      }
      val script = """
        window.dispatchEvent(new CustomEvent('dtv-douyu-danmaku-status', {
          detail: ${JSONObject.quote(json.toString())}
        }));
      """.trimIndent()
      playerWebView?.post {
        playerWebView?.evaluateJavascript(script, null)
      }
    }

    private fun dispatchDanmakuPayload(jsonPayload: String) {
      val script = """
        window.dispatchEvent(new CustomEvent('dtv-douyu-danmaku', {
          detail: ${JSONObject.quote(jsonPayload)}
        }));
      """.trimIndent()
      playerWebView?.post {
        playerWebView?.evaluateJavascript(script, null)
      }
    }

    private fun parseStt(message: String): Map<String, String> {
      val result = linkedMapOf<String, String>()
      for (field in message.split("/")) {
        if (field.isBlank()) {
          continue
        }
        val splitIndex = field.indexOf("@=")
        if (splitIndex <= 0) {
          continue
        }
        val key = field.substring(0, splitIndex)
        val value = field.substring(splitIndex + 2)
          .replace("@S", "/")
          .replace("@A", "@")
        result[key] = value
      }
      return result
    }

    private fun encodePacket(body: String): ByteArray {
      val bodyBytes = body.toByteArray(StandardCharsets.UTF_8)
      val packetLength = bodyBytes.size + 9
      val buffer = ByteArray(packetLength + 4)
      writeLittleEndianInt(buffer, 0, packetLength)
      writeLittleEndianInt(buffer, 4, packetLength)
      buffer[8] = 0xB1.toByte()
      buffer[9] = 0x02
      buffer[10] = 0
      buffer[11] = 0
      System.arraycopy(bodyBytes, 0, buffer, PACKET_HEADER_SIZE, bodyBytes.size)
      buffer[PACKET_HEADER_SIZE + bodyBytes.size] = 0
      return buffer
    }

    private fun readLittleEndianInt(buffer: ByteArray, offset: Int): Int {
      return (buffer[offset].toInt() and 0xFF) or
        ((buffer[offset + 1].toInt() and 0xFF) shl 8) or
        ((buffer[offset + 2].toInt() and 0xFF) shl 16) or
        ((buffer[offset + 3].toInt() and 0xFF) shl 24)
    }

    private fun writeLittleEndianInt(buffer: ByteArray, offset: Int, value: Int) {
      buffer[offset] = (value and 0xFF).toByte()
      buffer[offset + 1] = ((value shr 8) and 0xFF).toByte()
      buffer[offset + 2] = ((value shr 16) and 0xFF).toByte()
      buffer[offset + 3] = ((value shr 24) and 0xFF).toByte()
    }
  }
}
