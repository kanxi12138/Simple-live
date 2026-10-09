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
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume

class MainActivity : ComponentActivity() {
    private val model: LiveViewModel by viewModels()
    private var loginDialog: Dialog?=null
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(); super.onCreate(savedInstanceState)
        setContent { LiveApp(model,{login(it)},::fullscreen,::verifyBili,::verifyDouyinSearch) }
    }
    fun fullscreen(enabled: Boolean,landscape: Boolean) {
        requestedOrientation=if(enabled && landscape) ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE else ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
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
    private fun verifyDouyinSearch() {
        val state=model.search.value
        val query=state.committed ?: return
        if(!state.douyinVerification || query.platform!=Platform.DOUYIN) return
        login(Platform.DOUYIN,query)
    }
    private fun login(platform: Platform,searchQuery: SearchQuery?=null) {
        if(loginDialog!=null || platform !in listOf(Platform.DOUYIN,Platform.BILIBILI,Platform.DOUYU)) return
        val domain=when(platform) { Platform.DOUYIN -> "douyin.com"; Platform.DOUYU -> "douyu.com"; else -> "bilibili.com" }
        val target=if(searchQuery!=null) "https://www.douyin.com/search/${encoded(searchQuery.keyword)}?type=${if(searchQuery.accounts) "user" else "live"}"
            else when(platform) { Platform.DOUYIN -> "https://www.douyin.com/"; Platform.DOUYU -> "https://www.douyu.com/member/login"; else -> "https://passport.bilibili.com/login" }
        val cookies=CookieManager.getInstance()
        val webView=WebView(this).apply {
            settings.javaScriptEnabled=true; settings.domStorageEnabled=true
            settings.userAgentString=if(platform==Platform.DOUYIN) DOUYIN_UA else DESKTOP_UA
            if(searchQuery!=null || platform==Platform.DOUYU) {
                settings.useWideViewPort=true;settings.loadWithOverviewMode=true
                settings.builtInZoomControls=true;settings.displayZoomControls=false
            }
            if(platform==Platform.DOUYU) setInitialScale(35)
            settings.allowFileAccess=false; settings.allowContentAccess=false
            settings.mixedContentMode=android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW
            webViewClient=object: WebViewClient() {
                override fun onPageCommitVisible(view: WebView,url: String) {
                    if(platform==Platform.DOUYU) view.evaluateJavascript("document.querySelector('meta[name=viewport]')?.setAttribute('content','width=1200,initial-scale=0.35,user-scalable=yes');",null)
                }
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val uri=request.url; val host=uri.host.orEmpty()
                    return uri.scheme!="https" || !(host==domain || host.endsWith(".$domain"))
                }
                override fun onPageFinished(view: WebView,url: String) {
                    cookies.flush()
                    if(platform==Platform.DOUYU) {
                        // The official desktop login fixes a phone-width viewport around a wide form.
                        view.evaluateJavascript("document.querySelector('meta[name=viewport]')?.setAttribute('content','width=1200,initial-scale=0.35,user-scalable=yes');",null)
                    }
                }
            }
        }
        val dialog=Dialog(this,android.R.style.Theme_DeviceDefault_Light_NoActionBar)
        val layout=LinearLayout(this).apply { orientation=LinearLayout.VERTICAL; fitsSystemWindows=true }
        var confirmed=false
        var loginGeneration: Long?=null
        val done=Button(this).apply { text=if(searchQuery!=null) "完成登录或验证，重试搜索" else "完成登录，返回简直播";isEnabled=platform!=Platform.DOUYIN }
        done.setOnClickListener {
            if(platform!=Platform.DOUYIN) { confirmed=true;dialog.dismiss();return@setOnClickListener }
            val generation=loginGeneration ?: return@setOnClickListener
            val candidate=(cookies.getCookie("https://www.douyin.com/").orEmpty().split(';') + cookies.getCookie(Platform.DOUYIN.home).orEmpty().split(';'))
                .map { it.trim() }.filter { it.contains('=') }.associate { it.substringBefore('=') to it.substringAfter('=') }
            if(listOf("sessionid","sessionid_ss","sid_guard").none { !candidate[it].isNullOrBlank() }) {
                android.widget.Toast.makeText(this,"未获取到账号会话，请先完成登录",android.widget.Toast.LENGTH_LONG).show()
                return@setOnClickListener
            }
            val cookie=candidate.entries.joinToString("; ") { "${it.key}=${it.value}" }
            done.isEnabled=false
            val application=model.app
            application.applicationScope.launch {
                try {
                    withContext(Dispatchers.IO) { application.credentials.saveDouyinLogin(cookie,generation) }
                    if(!isDestroyed) {
                        model.notice.value="抖音登录信息已保存"
                        if(loginDialog===dialog) dialog.dismiss()
                        if(searchQuery!=null) {
                            val state=model.search.value
                            if(SearchQuery(state.platform,state.keyword.trim(),state.accounts)==searchQuery) model.searchRooms()
                        } else if(model.browse.value.platform==Platform.DOUYIN) model.loadRooms()
                    }
                } catch(error: kotlinx.coroutines.CancellationException) { throw error }
                catch(error: Exception) {
                    if(!isDestroyed && loginDialog===dialog) {
                        done.isEnabled=true
                        android.widget.Toast.makeText(this@MainActivity,"登录信息保存失败，原登录信息已保留",android.widget.Toast.LENGTH_LONG).show()
                    }
                }
            }
        }
        layout.addView(done); layout.addView(webView,LinearLayout.LayoutParams(-1,0,1f))
        dialog.setContentView(layout)
        dialog.setOnDismissListener {
            // Cancellation and Activity destruction never replace the encrypted session.
            val candidate=if(confirmed) (cookies.getCookie("https://www.$domain/").orEmpty().split(';') + cookies.getCookie(platform.home).orEmpty().split(';'))
                .map { it.trim() }.filter { it.contains('=') }.associate { it.substringBefore('=') to it.substringAfter('=') } else emptyMap()
            val hasAccountSession=when(platform) {
                Platform.BILIBILI -> !candidate["SESSDATA"].isNullOrBlank()
                Platform.DOUYU -> !candidate["acf_auth"].isNullOrBlank()
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
        loginDialog=dialog; dialog.show()
        if(platform!=Platform.DOUYIN) webView.loadUrl(target) else lifecycleScope.launch {
            try {
                val snapshot=withContext(Dispatchers.IO) { model.app.credentials.douyinSnapshot() }
                suspendCancellableCoroutine<Unit> { continuation ->
                    cookies.removeAllCookies { if(continuation.isActive) continuation.resume(Unit) }
                }
                for(cookie in snapshot.cookies.filter { it.expiresAt>System.currentTimeMillis() }) {
                    if(loginDialog!==dialog || !dialog.isShowing) return@launch
                    val restored=suspendCancellableCoroutine<Boolean> { continuation ->
                        cookies.setCookie("https://${cookie.domain}${cookie.path}",cookie.toString()) {
                            if(continuation.isActive) continuation.resume(it)
                        }
                    }
                    check(restored) { "抖音登录信息恢复失败" }
                }
                if(loginDialog===dialog && dialog.isShowing) {
                    cookies.flush();loginGeneration=snapshot.generation;done.isEnabled=true
                    webView.loadUrl(target)
                }
            } catch(error: kotlinx.coroutines.CancellationException) { throw error }
            catch(error: Exception) {
                if(loginDialog===dialog && dialog.isShowing) {
                    model.notice.value="抖音登录信息恢复失败，原登录信息已保留"
                    dialog.dismiss()
                }
            }
        }
    }
    override fun onDestroy() { loginDialog?.dismiss(); super.onDestroy() }
    override fun onStart() { super.onStart();model.foreground(true) }
    override fun onStop() { model.foreground(false);super.onStop() }
}
