package com.simplelive.nativeapp

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import kotlinx.coroutines.sync.withLock
import okhttp3.Request
import java.io.File
import kotlin.coroutines.coroutineContext

data class Release(val version: String,val notes: String,val url: String,val size: Long)
data class UpdateState(val busy: Boolean=false,val downloading: Boolean=false,val progress: Int=0,val release: Release?=null,val file: File?=null)
class Updates(context: Context,private val network: Network) {
    private val context=context.applicationContext
    companion object { private val downloadMutex=kotlinx.coroutines.sync.Mutex() }
    suspend fun restoredDownload(): File? = downloadMutex.withLock { withContext(Dispatchers.IO) {
        val file=File(context.cacheDir,"updates/simplelive-update.apk")
        if(!file.isFile) return@withContext null
        try { validate(file);file }
        catch(error: IllegalArgumentException) { file.delete();null }
        catch(error: PlatformException) { file.delete();null }
    }
    }
    suspend fun latest(): Release? {
        val json=network.json("https://api.github.com/repos/kanxi12138/Simple-live/releases/latest",mapOf("Accept" to "application/vnd.github+json"))
        val version=json.str("tag_name").removePrefix("v")
        val current=BuildConfig.VERSION_NAME.split('.').map { it.toIntOrNull() ?: 0 }
        val available=version.split('.').map { it.toIntOrNull() ?: 0 }
        val comparison=(0 until maxOf(current.size,available.size)).map { available.getOrElse(it){0}.compareTo(current.getOrElse(it){0}) }.firstOrNull { it!=0 } ?: 0
        if(comparison<=0) return null
        val asset=json.arr("assets").objects().firstOrNull { it.str("name").endsWith(".apk") } ?: throw PlatformException("新版未提供 APK，请查看项目主页")
        return Release(version,json.str("body"),asset.getString("browser_download_url"),asset.getLong("size"))
    }
    suspend fun download(release: Release,onProgress: (Int)->Unit): File = downloadMutex.withLock { withContext(Dispatchers.IO) {
        require(release.size in 1..250_000_000) { "更新包大小异常" }
        val directory=File(context.cacheDir,"updates").apply { mkdirs() }
        val target=File(directory,"simplelive-update.apk")
        val temporary=File(directory,"simplelive-update.part")
        val client=network.client.newBuilder().callTimeout(0,java.util.concurrent.TimeUnit.MILLISECONDS).followRedirects(true).followSslRedirects(true)
            .addNetworkInterceptor { chain ->
                val url=chain.request().url
                val allowed=listOf("github.com","objects.githubusercontent.com","release-assets.githubusercontent.com")
                if(!url.isHttps || url.port!=443 || url.host !in allowed || url.username.isNotBlank() || url.password.isNotBlank()) throw java.io.IOException("更新下载域名无效")
                chain.proceed(chain.request().newBuilder().removeHeader("Cookie").removeHeader("Authorization").build())
            }.build()
        try {
            Network.await(client.newCall(Request.Builder().url(release.url).build())).use { response ->
                if(!response.isSuccessful) throw PlatformException("更新下载失败（${response.code}）")
                val body=response.body ?: throw PlatformException("更新包为空")
                body.byteStream().use { source -> temporary.outputStream().use { output ->
                    val buffer=ByteArray(65536); var total=0L
                    while(true) { coroutineContext.ensureActive(); val count=source.read(buffer); if(count<0) break; total+=count; require(total<=release.size); output.write(buffer,0,count); onProgress((total*100/release.size).toInt()) }
                    require(total==release.size) { "更新包下载不完整" }
                } }
            }
            coroutineContext.ensureActive()
            validate(temporary)
            if(target.exists() && !target.delete()) throw java.io.IOException("旧更新包清理失败")
            if(!temporary.renameTo(target)) throw java.io.IOException("更新包保存失败")
            target
        } finally { if(temporary.exists()) temporary.delete() }
    }
    }
    @Suppress("DEPRECATION")
    private fun validate(file: File) {
        val manager=context.packageManager
        val flags=if(Build.VERSION.SDK_INT>=28) PackageManager.GET_SIGNING_CERTIFICATES else PackageManager.GET_SIGNATURES
        val archive=manager.getPackageArchiveInfo(file.absolutePath,flags) ?: throw PlatformException("无法读取更新包")
        val installed=manager.getPackageInfo(context.packageName,flags)
        val archiveVersion=if(Build.VERSION.SDK_INT>=28) archive.longVersionCode else archive.versionCode.toLong()
        val installedVersion=if(Build.VERSION.SDK_INT>=28) installed.longVersionCode else installed.versionCode.toLong()
        require(archive.packageName==context.packageName && archiveVersion>installedVersion) { "更新包的包名或版本不匹配" }
        fun certificates(info: android.content.pm.PackageInfo): Set<String> =
            (if(Build.VERSION.SDK_INT>=28) info.signingInfo?.apkContentsSigners else info.signatures).orEmpty()
                .map { android.util.Base64.encodeToString(it.toByteArray(),android.util.Base64.NO_WRAP) }.toSet()
        require(certificates(archive).isNotEmpty() && certificates(archive)==certificates(installed)) { "更新包签名不匹配" }
    }
    fun install(file: File) {
        validate(file)
        if(Build.VERSION.SDK_INT>=26 && !context.packageManager.canRequestPackageInstalls()) {
            context.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,android.net.Uri.parse("package:${context.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            throw PlatformException("请允许安装此来源的应用，然后再次点击安装")
        }
        val uri=FileProvider.getUriForFile(context,"${context.packageName}.fileprovider",file)
        context.startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(uri,"application/vnd.android.package-archive").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}
