@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.simplelive.nativeapp

import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray

@Composable
fun SettingsScreen(model: LiveViewModel,onLogin: (Platform)->Unit) {
    val context=LocalContext.current
    val preferences by model.preferences.collectAsStateWithLifecycle()
    var pendingImport by remember { mutableStateOf<String?>(null) }
    var paste by remember { mutableStateOf(false) }
    var pasteText by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var legalFile by remember { mutableStateOf<String?>(null) }
    var danmaku by remember { mutableStateOf(false) }
    val update by model.update.collectAsStateWithLifecycle()
    val importLauncher=rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if(uri!=null) model.operation {
            busy=true
            try { pendingImport=withContext(Dispatchers.IO) {
                context.contentResolver.openInputStream(uri)?.use { input ->
                    val output=java.io.ByteArrayOutputStream();val buffer=ByteArray(8192)
                    while(true){val count=input.read(buffer);if(count<0)break;require(output.size()+count<=4*1024*1024){"配置文件过大"};output.write(buffer,0,count)}
                    output.toString("UTF-8")
                } ?: throw IllegalArgumentException("无法读取配置文件")
            } } finally {busy=false}
        }
    }
    val exportLauncher=rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        if(uri!=null) model.operation {busy=true;try{val data=model.storage.export();withContext(Dispatchers.IO){context.contentResolver.openOutputStream(uri,"wt")?.bufferedWriter()?.use{it.write(data)} ?: throw IllegalArgumentException("无法写入配置文件")};model.notice.value="配置已导出，不包含登录凭证"}finally{busy=false}}
    }
    SectionTitle("设置","简直播 ${BuildConfig.VERSION_NAME} · Android 原生版")
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom=24.dp)) {
        SettingHeading("外观与播放")
        Row(Modifier.padding(horizontal=20.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)) {
            listOf("system" to "跟随系统","light" to "浅色","dark" to "深色").forEach { (value,label)->FilterChip((preferences["theme_preference"] ?: "system")==value,{model.setPreference("theme_preference",value)},label={Text(label)}) }
        }
        ListItem(headlineContent={Text("弹幕与屏蔽词")},supportingContent={Text("字号、速度、透明度、密度和显示区域")},leadingContent={Icon(Icons.Outlined.Forum,null)},modifier=Modifier.clickable{danmaku=true})
        SettingHeading("平台登录")
        listOf(Platform.DOUYIN,Platform.BILIBILI).forEach { platform ->
            ListItem(headlineContent={Text(platform.label)},supportingContent={Text("使用平台官方网页登录")},leadingContent={Icon(Icons.Outlined.AccountCircle,null)},trailingContent={TextButton(onClick={model.operation{model.app.credentials.clear(platform);model.notice.value="已清除${platform.label}登录信息"}}){Text("退出登录")}},modifier=Modifier.clickable{onLogin(platform)})
        }
        SettingHeading("配置迁移")
        ListItem(headlineContent={Text("导出配置")},supportingContent={Text("关注、分组、订阅分类、主题和播放偏好")},leadingContent={Icon(Icons.Outlined.FileUpload,null)},modifier=Modifier.clickable(enabled=!busy){exportLauncher.launch("simplelive-config.json")})
        ListItem(headlineContent={Text("导入配置文件")},supportingContent={Text("兼容旧版 dtv-config，导入会整体替换配置")},leadingContent={Icon(Icons.Outlined.FileDownload,null)},modifier=Modifier.clickable(enabled=!busy){importLauncher.launch(arrayOf("application/json","text/plain","application/octet-stream"))})
        ListItem(headlineContent={Text("粘贴配置内容")},leadingContent={Icon(Icons.Outlined.ContentPaste,null)},modifier=Modifier.clickable(enabled=!busy){paste=true})
        SettingHeading("应用更新")
        ListItem(headlineContent={Text(if(update.downloading) "下载中 ${update.progress}%" else if(update.busy) "正在检查更新…" else "检查更新")},supportingContent={Text("仅接受包名、版本和签名匹配的安装包")},leadingContent={Icon(Icons.Outlined.SystemUpdate,null)},modifier=Modifier.clickable(enabled=!update.busy,onClick=model::checkUpdate))
        if(update.file!=null) ListItem(headlineContent={Text("安装已下载的更新")},modifier=Modifier.clickable(enabled=!update.busy,onClick=model::installUpdate))
        if(busy || update.busy) LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal=20.dp))
        SettingHeading("项目与版权")
        ListItem(headlineContent={Text("项目主页")},supportingContent={Text("Simple-live")},leadingContent={Icon(Icons.Outlined.OpenInNew,null)},modifier=Modifier.clickable {context.startActivity(Intent(Intent.ACTION_VIEW,Uri.parse("https://github.com/kanxi12138/Simple-live")))})
        listOf("PRIVACY.md" to "隐私说明","COMPLIANCE.md" to "功能边界","LICENSE" to "GNU GPL v3","THIRD_PARTY_NOTICES.md" to "第三方版权声明","ANDROID-NATIVE-DEPENDENCIES.txt" to "原生版开源依赖","Apache-2.0.txt" to "Apache 2.0 许可证","Rhino-MPL-2.0.txt" to "Rhino MPL 2.0 许可证","nv-websocket-client-LICENSE.txt" to "抖音 WebSocket 许可证","DTV-MIT.txt" to "DTV MIT 许可证","Brotli-MIT.txt" to "Brotli MIT 许可证","jsoup-MIT.txt" to "jsoup MIT 许可证","CryptoJS-MIT.txt" to "CryptoJS MIT 许可证").forEach { (file,label)->
            ListItem(headlineContent={Text(label)},trailingContent={Icon(Icons.Outlined.ChevronRight,null)},modifier=Modifier.clickable{legalFile=file})
        }
    }
    if(paste) AlertDialog(onDismissRequest={paste=false},title={Text("粘贴配置内容")},text={OutlinedTextField(pasteText,{pasteText=it},modifier=Modifier.fillMaxWidth().heightIn(min=180.dp,max=360.dp),placeholder={Text("粘贴完整 JSON 配置")})},confirmButton={TextButton(onClick={pendingImport=pasteText;paste=false},enabled=pasteText.isNotBlank()){Text("下一步")}},dismissButton={TextButton(onClick={paste=false}){Text("取消")}})
    pendingImport?.let { text -> AlertDialog(onDismissRequest={if(!busy)pendingImport=null},title={Text("替换当前配置？")},text={Text("将替换关注、分组、分类订阅和播放偏好。登录信息保留。配置会先校验，再写入；建议先导出当前配置。")},
        confirmButton={TextButton(enabled=!busy,onClick={model.operation{busy=true;try{model.storage.importConfig(text);pendingImport=null;model.notice.value="配置导入完成"}finally{busy=false}}}){Text(if(busy)"导入中…" else "确认导入")}},dismissButton={TextButton(enabled=!busy,onClick={pendingImport=null}){Text("取消")}}) }
    if(danmaku) ModalBottomSheet(onDismissRequest={danmaku=false}) {Column(Modifier.verticalScroll(rememberScrollState()).padding(20.dp)){DanmakuSettingsPanel(DanmakuSettings.from(preferences["dtv_danmu_preferences_v1"]),JSONArray(preferences["danmu_block_keywords"] ?: "[]").strings(),{model.setPreference("dtv_danmu_preferences_v1",it.json())},{model.setPreference("danmu_block_keywords",JSONArray(it).toString())});Spacer(Modifier.height(24.dp))}}
    legalFile?.let { name ->
        var content by remember(name){mutableStateOf("正在读取…")}
        LaunchedEffect(name){content=withContext(Dispatchers.IO){context.assetText("legal/$name")}}
        ModalBottomSheet(onDismissRequest={legalFile=null}){Column(Modifier.fillMaxHeight(0.9f).verticalScroll(rememberScrollState()).padding(20.dp)){Text(name,style=MaterialTheme.typography.titleLarge);Spacer(Modifier.height(16.dp));Text(content,style=MaterialTheme.typography.bodySmall);Spacer(Modifier.height(32.dp))}}
    }
    update.release?.let { info -> AlertDialog(onDismissRequest=model::dismissUpdate,title={Text("发现新版本 ${info.version}")},text={Text(info.notes.take(1500),Modifier.heightIn(max=300.dp).verticalScroll(rememberScrollState()))},confirmButton={TextButton(onClick=model::downloadUpdate,enabled=!update.busy){Text("下载更新")}},dismissButton={TextButton(onClick=model::dismissUpdate){Text("稍后")}}) }
}
@Composable
private fun SettingHeading(label: String){Text(label,Modifier.padding(start=20.dp,top=24.dp,bottom=8.dp),style=MaterialTheme.typography.labelLarge,fontWeight=FontWeight.SemiBold,color=MaterialTheme.colorScheme.primary)}
