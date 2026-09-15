package www.sp.com

import android.app.Activity
import android.app.Dialog
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.LinearLayout
import android.view.ViewGroup

private const val DOUYIN_LOGIN_URL = "https://www.douyin.com/"
private const val DOUYIN_LOGIN_USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.5845.97 Safari/537.36 Core/1.116.567.400 QQBrowser/19.7.6764.400"

/** Opens Douyin's own login page and shares its cookies only with the app search flow. */
class DouyinLoginBridge(private val activity: Activity, private val onClosed: () -> Unit) {
  private var dialog: Dialog? = null

  /** Returns the official website's runtime cookies; no credentials are embedded or logged. */
  @JavascriptInterface
  fun getCookie(): String = CookieManager.getInstance().getCookie(DOUYIN_LOGIN_URL).orEmpty()

  /** Opens the login dialog on the UI thread; duplicate requests reuse the existing dialog. */
  @JavascriptInterface
  fun openLogin() {
    activity.runOnUiThread {
      if (!activity.isFinishing && !activity.isDestroyed && dialog == null) showLogin()
    }
  }

  /** Dismisses the login view during activity teardown, releasing its WebView. */
  fun close() {
    dialog?.dismiss()
  }

  private fun createWebView(): WebView = WebView(activity).apply {
    // This website view intentionally has no app JavascriptInterface attached.
    settings.javaScriptEnabled = true
    settings.domStorageEnabled = true
    settings.userAgentString = DOUYIN_LOGIN_USER_AGENT
    settings.useWideViewPort = true
    settings.loadWithOverviewMode = true
    settings.allowFileAccess = false
    settings.allowContentAccess = false
    webViewClient = object : WebViewClient() {
      override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
        return request.url.scheme != "https"
      }
    }
    loadUrl(DOUYIN_LOGIN_URL)
  }

  private fun showLogin() {
    val webView = createWebView()
    val loginDialog = Dialog(activity, android.R.style.Theme_DeviceDefault_NoActionBar)
    val layout = LinearLayout(activity).apply {
      orientation = LinearLayout.VERTICAL
      fitsSystemWindows = true
    }
    val doneButton = Button(activity).apply {
      text = "完成登录，返回搜索"
      setOnClickListener { loginDialog.dismiss() }
    }
    layout.addView(doneButton)
    layout.addView(webView, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
    loginDialog.setContentView(layout)
    loginDialog.setOnDismissListener {
      CookieManager.getInstance().flush()
      layout.removeView(webView)
      webView.destroy()
      dialog = null
      if (!activity.isFinishing && !activity.isDestroyed) onClosed()
    }
    dialog = loginDialog
    loginDialog.show()
  }
}
