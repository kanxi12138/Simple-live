# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# Keep the generated Android host bridge stable for Tauri runtime/JNI.
-keep class www.sp.com.MainActivity { *; }
-keep class www.sp.com.TauriActivity { *; }
-keep class www.sp.com.WryActivity { *; }
-keep class www.sp.com.RustWebView { *; }
-keep class www.sp.com.RustWebViewClient { *; }
-keep class www.sp.com.RustWebChromeClient { *; }
-keep class www.sp.com.** { *; }
-keepclassmembers class www.sp.com.TauriActivity {
  app.tauri.plugin.PluginManager pluginManager;
  app.tauri.plugin.PluginManager getPluginManager();
  void setPluginManager(app.tauri.plugin.PluginManager);
}
-keepclassmembers class www.sp.com.WryActivity {
  void onWebViewCreate(android.webkit.WebView);
}

# Keep Tauri plugin bridge classes and members that may be accessed
# reflectively during startup.
-keep class app.tauri.plugin.** { *; }
-keep class app.tauri.annotation.** { *; }
-keepclassmembers class ** {
  @app.tauri.annotation.Command *;
  @app.tauri.annotation.InvokeArg *;
  @app.tauri.annotation.Permission *;
  @app.tauri.annotation.ActivityCallback *;
  @app.tauri.annotation.PermissionCallback *;
  @android.webkit.JavascriptInterface *;
}

# Preserve Kotlin metadata and reflective accessors used by the generated
# host classes and plugins.
-keep class kotlin.Metadata { *; }
-keepattributes *Annotation*,InnerClasses,EnclosingMethod,Signature
