package www.sp.com

import android.content.pm.ActivityInfo
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var playerWebView: WebView? = null
  private lateinit var backPressedCallback: OnBackPressedCallback

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
            """.trimIndent()
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

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    playerWebView = webView
    webView.addJavascriptInterface(OrientationBridge(), "DTVOrientation")
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
}
