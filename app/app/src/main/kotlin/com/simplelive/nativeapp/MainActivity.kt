package com.simplelive.nativeapp

import android.app.Dialog
import android.content.pm.ActivityInfo
import android.os.Bundle
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.LinearLayout
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {
    private val model: LiveViewModel by viewModels()
    private var loginDialog: Dialog?=null
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(); super.onCreate(savedInstanceState)
        setContent { LiveApp(model,::login,::fullscreen,::verifyBili) }
    }
    fun fullscreen(enabled: Boolean) {
        requestedOrientation=if(enabled) ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE else ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        WindowCompat.getInsetsController(window,window.decorView).apply {
            systemBarsBehavior=WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            if(enabled) hide(WindowInsetsCompat.Type.systemBars()) else show(WindowInsetsCompat.Type.systemBars())
        }
    }
    private fun verifyBili(challenge: BiliListChallenge) {
        if(loginDialog!=null || model.browse.value.verification !== challenge) return
        loginDialog=showBiliVerification(this,challenge) { token ->
            loginDialog=null
            model.completeVerification(challenge,token)
        }
    }
    private fun login(platform: Platform) {
        if(loginDialog!=null || platform !in listOf(Platform.DOUYIN,Platform.BILIBILI)) return
        val domain=if(platform==Platform.DOUYIN) "douyin.com" else "bilibili.com"
        val target=if(platform==Platform.DOUYIN) "https://www.douyin.com/" else "https://passport.bilibili.com/login"
        val cookies=CookieManager.getInstance()
        val webView=WebView(this).apply {
            settings.javaScriptEnabled=true; settings.domStorageEnabled=true
            settings.userAgentString=if(platform==Platform.DOUYIN) DOUYIN_UA else DESKTOP_UA
            settings.allowFileAccess=false; settings.allowContentAccess=false
            settings.mixedContentMode=android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW
            webViewClient=object: WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val uri=request.url; val host=uri.host.orEmpty()
                    return uri.scheme!="https" || !(host==domain || host.endsWith(".$domain"))
                }
                override fun onPageFinished(view: WebView,url: String) { cookies.flush() }
            }
        }
        val dialog=Dialog(this,android.R.style.Theme_DeviceDefault_Light_NoActionBar)
        val layout=LinearLayout(this).apply { orientation=LinearLayout.VERTICAL; fitsSystemWindows=true }
        var confirmed=false
        val done=Button(this).apply { text="完成登录，返回简直播"; setOnClickListener { confirmed=true; dialog.dismiss() } }
        layout.addView(done); layout.addView(webView,LinearLayout.LayoutParams(-1,0,1f))
        dialog.setContentView(layout)
        dialog.setOnDismissListener {
            // Cancellation and Activity destruction never replace the encrypted session.
            val candidate=if(confirmed) (cookies.getCookie("https://www.$domain/").orEmpty().split(';') + cookies.getCookie(platform.home).orEmpty().split(';'))
                .map { it.trim() }.filter { it.contains('=') }.associate { it.substringBefore('=') to it.substringAfter('=') } else emptyMap()
            val hasAccountSession=when(platform) {
                Platform.BILIBILI -> !candidate["SESSDATA"].isNullOrBlank()
                Platform.DOUYIN -> listOf("sessionid","sessionid_ss","sid_guard").any { !candidate[it].isNullOrBlank() }
                else -> false
            }
            val cookie=candidate.entries.joinToString("; ") { "${it.key}=${it.value}" }
            webView.stopLoading();layout.removeView(webView)
            webView.clearCache(true)
            android.webkit.WebStorage.getInstance().deleteAllData()
            webView.destroy()
            // Keep the dialog guard until the asynchronous global cookie cleanup finishes.
            cookies.removeAllCookies {
                cookies.flush()
                if(loginDialog===dialog) loginDialog=null
            }
            if(confirmed) lifecycleScope.launch {
                try {
                    if(hasAccountSession) {
                        withContext(Dispatchers.IO) { model.app.credentials.save(platform,cookie) }
                        model.notice.value="会话已保存，登录状态待确认"
                        if(model.browse.value.platform==platform) model.loadRooms()
                    } else model.notice.value="未获取到账号会话，原登录信息已保留"
                }
                catch(error: kotlinx.coroutines.CancellationException) { throw error }
                catch(error: Exception) { model.notice.value="登录信息保存失败，原登录信息已保留" }
            }
        }
        loginDialog=dialog; dialog.show(); webView.loadUrl(target)
    }
    override fun onDestroy() { loginDialog?.dismiss(); super.onDestroy() }
    override fun onStart() { super.onStart();model.foreground(true) }
    override fun onStop() { model.foreground(false);super.onStop() }
}
