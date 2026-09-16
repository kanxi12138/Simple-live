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
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
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

// Shared validation keeps APK identity checks independent of Android UI calls.
internal fun validateUpdateIdentity(
  archivePackage: String, installedPackage: String, archiveVersion: Long, installedVersion: Long,
  versionName: String?, archiveSignatures: Set<String>?, installedSignatures: Set<String>?,
): String {
  if (archivePackage != installedPackage) return "更新安装失败：包名不匹配"
  if (archiveVersion < installedVersion || versionName.isNullOrBlank()) return "更新安装失败：版本不兼容"
  if (archiveSignatures.isNullOrEmpty() || archiveSignatures != installedSignatures) return "更新安装失败：签名不兼容"
  return ""
}

class MainActivity : TauriActivity() {
  private var playerWebView: WebView? = null
  private var isImmersiveFullscreen = false
  private lateinit var backPressedCallback: OnBackPressedCallback
  private val douyuDanmakuBridge = DouyuDanmakuBridge()
  private val douyinLoginBridge by lazy {
    DouyinLoginBridge(this) {
      playerWebView?.evaluateJavascript(
        "window.dispatchEvent(new Event('dtv-douyin-login-closed'));", null,
      )
    }
  }

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
    douyinLoginBridge.close()
    douyuDanmakuBridge.stop()
    super.onDestroy()
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus && isImmersiveFullscreen) {
      hideSystemBars()
    }
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    playerWebView = webView
    WebView.setWebContentsDebuggingEnabled(BuildConfig.DEBUG)
    webView.settings.mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
    webView.settings.allowFileAccessFromFileURLs = false
    webView.settings.allowUniversalAccessFromFileURLs = false
    webView.settings.mediaPlaybackRequiresUserGesture = false
    webView.addJavascriptInterface(OrientationBridge(), "DTVOrientation")
    webView.addJavascriptInterface(UpdateBridge(), "DTVUpdate")
    if (BuildConfig.DEBUG) webView.addJavascriptInterface(DebugBridge(), "DTVDebug")
    webView.addJavascriptInterface(douyuDanmakuBridge, "DTVDouyuDanmaku")
    webView.addJavascriptInterface(douyinLoginBridge, "DTVDouyinLogin")
  }

  private fun fallbackBackPressed() {
    backPressedCallback.isEnabled = false
    onBackPressedDispatcher.onBackPressed()
    backPressedCallback.isEnabled = true
  }

  private fun enterImmersiveFullscreen() {
    isImmersiveFullscreen = true
    hideSystemBars()
  }

  private fun exitImmersiveFullscreen() {
    isImmersiveFullscreen = false
    WindowCompat.getInsetsController(window, window.decorView)
      .show(WindowInsetsCompat.Type.systemBars())
  }

  private fun hideSystemBars() {
    val insetsController = WindowCompat.getInsetsController(window, window.decorView)
    insetsController.systemBarsBehavior =
      WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    insetsController.hide(WindowInsetsCompat.Type.systemBars())
  }

  private inner class OrientationBridge {
    @JavascriptInterface
    fun enterLandscapeFullscreen() {
      runOnUiThread {
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        enterImmersiveFullscreen()
      }
    }

    @JavascriptInterface
    fun enterPortraitFullscreen() {
      runOnUiThread {
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        enterImmersiveFullscreen()
      }
    }

    @JavascriptInterface
    fun exitFullscreen() {
      runOnUiThread {
        exitImmersiveFullscreen()
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
    fun installApk(filePath: String): String {
      try {
        val apkFile = File(filePath).canonicalFile
        val updatesDir = File(cacheDir, "updates").canonicalFile
        if (apkFile.parentFile != updatesDir || apkFile.extension.lowercase() != "apk" || !apkFile.isFile) {
          return "更新安装失败：文件不在更新缓存目录内"
        }
        @Suppress("DEPRECATION")
        val archive = packageManager.getPackageArchiveInfo(apkFile.path, android.content.pm.PackageManager.GET_SIGNATURES)
          ?: return "更新安装失败：APK 无效"
        @Suppress("DEPRECATION")
        val installed = packageManager.getPackageInfo(packageName, android.content.pm.PackageManager.GET_SIGNATURES)
        @Suppress("DEPRECATION")
        val archiveVersion = if (Build.VERSION.SDK_INT >= 28) archive.longVersionCode else archive.versionCode.toLong()
        @Suppress("DEPRECATION")
        val installedVersion = if (Build.VERSION.SDK_INT >= 28) installed.longVersionCode else installed.versionCode.toLong()
        @Suppress("DEPRECATION")
        val archiveSignatures = archive.signatures?.map { it.toCharsString() }?.toSet()
        @Suppress("DEPRECATION")
        val installedSignatures = installed.signatures?.map { it.toCharsString() }?.toSet()
        val identityError = validateUpdateIdentity(archive.packageName, packageName, archiveVersion,
          installedVersion, archive.versionName, archiveSignatures, installedSignatures)
        if (identityError.isNotEmpty()) return identityError

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
        return ""
      } catch (_: Exception) {
        return "更新安装失败：无法打开系统安装器"
      }
    }
  }

  private inner class DebugBridge {
    @JavascriptInterface
    fun log(level: String?, message: String?) {
      val safeMessage = "WebView diagnostic event (details omitted)"
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

    @Volatile private var connectionGeneration = 0L
    private var webSocket: WebSocket? = null
    private var currentRoomId: String? = null
    private var pendingBuffer = ByteArrayOutputStream()

    private val heartbeatRunnable = object : Runnable {
      override fun run() {
        val roomId = currentRoomId ?: return
        sendPacket("type@=mrkl/")
        Log.d("DouyuDanmaku", "Danmaku event (details omitted)")
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
        val generation = connectionGeneration
        currentRoomId = normalizedRoomId
        pendingBuffer = ByteArrayOutputStream()
        Log.i("DouyuDanmaku", "Danmaku event (details omitted)")
        dispatchDanmakuStatus("start", normalizedRoomId, "starting native websocket")

        val request = Request.Builder()
          .url(DOUYU_DANMAKU_URL)
          .build()

        webSocket = httpClient.newWebSocket(request, object : WebSocketListener() {
          override fun onOpen(webSocket: WebSocket, response: Response) {
            if (generation != connectionGeneration) return
            Log.i("DouyuDanmaku", "Danmaku event (details omitted)")
            dispatchDanmakuStatus("open", normalizedRoomId, "websocket opened")
            webSocket.send(ByteString.of(*encodePacket("type@=loginreq/roomid@=$normalizedRoomId/")))
            webSocket.send(ByteString.of(*encodePacket("type@=joingroup/rid@=$normalizedRoomId/gid@=-9999/")))
            mainHandler.removeCallbacks(heartbeatRunnable)
            mainHandler.postDelayed(heartbeatRunnable, HEARTBEAT_INTERVAL_MS)
          }

          override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
            if (generation != connectionGeneration) return
            dispatchDanmakuStatus("message", normalizedRoomId, "bytes=${bytes.size}")
            handleIncomingBytes(normalizedRoomId, bytes.toByteArray())
          }

          override fun onMessage(webSocket: WebSocket, text: String) {
            if (generation != connectionGeneration) return
            dispatchDanmakuStatus("message", normalizedRoomId, "text=${text.length}")
            handleIncomingBytes(normalizedRoomId, text.toByteArray(StandardCharsets.UTF_8))
          }

          override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            if (generation != connectionGeneration) return
            Log.e("DouyuDanmaku", "Danmaku event (details omitted)")
            dispatchDanmakuStatus("error", normalizedRoomId, t.message ?: "unknown")
          }

          override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
            if (generation != connectionGeneration) return
            Log.w("DouyuDanmaku", "Danmaku event (details omitted)")
            dispatchDanmakuStatus("closed", normalizedRoomId, "code=$code reason=$reason")
            mainHandler.removeCallbacks(heartbeatRunnable)
          }
        })
      } catch (error: Throwable) {
        Log.e("DouyuDanmaku", "Danmaku event (details omitted)")
        dispatchDanmakuStatus("error", roomId?.trim().orEmpty(), error.message ?: "unknown start exception")
      }
    }

    @JavascriptInterface
    fun stop() {
      connectionGeneration += 1
      mainHandler.removeCallbacks(heartbeatRunnable)
      webSocket?.cancel()
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
        if (packetLength < MIN_PACKET_BODY_SIZE || packetLength != readLittleEndianInt(data, cursor + 4)) {
          pendingBuffer = ByteArrayOutputStream()
          dispatchDanmakuStatus("error", roomId, "invalid packet length")
          return
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
          if (parsed["type"] == "loginres") dispatchDanmakuStatus("ready", roomId, "login accepted")
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
      Log.d("DouyuDanmaku", "Danmaku event (details omitted)")

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
