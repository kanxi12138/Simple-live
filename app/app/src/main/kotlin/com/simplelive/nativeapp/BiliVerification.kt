package com.simplelive.nativeapp

import android.app.Dialog
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.LinearLayout
import org.json.JSONObject
import org.json.JSONTokener
import java.util.UUID

/** Official captcha SDK only. No Java bridge, account cookies, or arbitrary request proxy. */
fun showBiliVerification(context: Context,challenge: BiliListChallenge,onResult: (String?)->Unit): Dialog {
    val dialog=Dialog(context,android.R.style.Theme_DeviceDefault_Light_NoActionBar)
    val handler=Handler(Looper.getMainLooper())
    val nonce=UUID.randomUUID().toString()
    val origin="https://live.bilibili.com/native-verification"
    var finished=false
    var resultToken: String?=null
    val webView=WebView(context).apply {
        settings.javaScriptEnabled=true
        settings.domStorageEnabled=true
        settings.userAgentString=BILI_LIST_UA
        settings.allowFileAccess=false
        settings.allowContentAccess=false
        settings.mixedContentMode=android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW
        webViewClient=object: WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView,request: WebResourceRequest): Boolean =
                request.isForMainFrame || request.url.scheme!="https"
        }
    }
    val layout=LinearLayout(context).apply { orientation=LinearLayout.VERTICAL;fitsSystemWindows=true }
    layout.addView(Button(context).apply { text="取消验证，返回简直播";setOnClickListener { dialog.dismiss() } })
    layout.addView(webView,LinearLayout.LayoutParams(-1,0,1f))
    dialog.setContentView(layout)
    dialog.setOnDismissListener {
        finished=true
        handler.removeCallbacksAndMessages(null)
        webView.stopLoading();layout.removeView(webView);webView.destroy()
        android.webkit.WebStorage.getInstance().deleteAllData()
        // The caller must keep its login guard until global cookie cleanup completes.
        CookieManager.getInstance().removeAllCookies {
            CookieManager.getInstance().flush()
            onResult(resultToken)
        }
    }
    val poll=object: Runnable {
        override fun run() {
            if(finished) return
            if(webView.url!=origin) { handler.postDelayed(this,500);return }
            webView.evaluateJavascript("JSON.stringify(window.__nativeVerificationResult || null)") { raw ->
                if(!finished) {
                    val result=try {
                        val decoded=JSONTokener(raw).nextValue()
                        if(decoded is String && decoded!="null") JSONObject(decoded) else null
                    } catch(error: org.json.JSONException) { null }
                    if(result?.optString("nonce")==nonce) {
                        val token=result.optString("token")
                        if(result.optBoolean("success") && token.isNotBlank() && token.length<=8192) resultToken=token
                        dialog.dismiss()
                    } else handler.postDelayed(this,500)
                }
            }
        }
    }
    val parameters=JSONObject().put("v_voucher",challenge.voucher).put("fromSpmid","444.253").toString().replace("<","\\u003c")
    val html="""<!doctype html><html lang="zh-CN"><head><meta charset="utf-8">
        <meta name="viewport" content="width=device-width,initial-scale=1">
        <title>B站安全验证</title></head><body><p id="status">正在加载B站官方安全验证…</p>
        <script src="https://s1.hdslb.com/bfs/seed/jinkela/risk-captcha-sdk/CaptchaLoader.js"></script>
        <script>
        (async function(){
          try {
            const verify = await window.CaptchaLoader.load();
            const token = await verify($parameters);
            window.__nativeVerificationResult={nonce:${JSONObject.quote(nonce)},success:typeof token==='string' && token.length>0,token:typeof token==='string'?token:''};
          } catch(error) {
            window.__nativeVerificationResult={nonce:${JSONObject.quote(nonce)},success:false};
          }
        })();
        </script></body></html>""".trimIndent()
    dialog.show()
    val cookies=CookieManager.getInstance()
    cookies.removeAllCookies {
        if(!finished) cookies.setCookie("https://live.bilibili.com/","buvid3=i; Domain=.bilibili.com; Path=/; Secure") {
            if(!finished) {
                webView.loadDataWithBaseURL(origin,html,"text/html","UTF-8",null)
                handler.postDelayed(poll,500)
                handler.postDelayed({ if(!finished) dialog.dismiss() },180_000)
            }
        }
    }
    return dialog
}
