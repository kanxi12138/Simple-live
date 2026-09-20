package com.simplelive.nativeapp

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import androidx.room.*
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyStore
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

private val Context.nativePreferences by preferencesDataStore("preferences")

/** Credentials never enter the portable configuration or database. */
class Credentials(context: Context) {
    private val storage = context.getSharedPreferences("encrypted_credentials", Context.MODE_PRIVATE)
    private val keyAlias = "simplelive.credentials.v1"
    @Synchronized private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        return (store.getKey(keyAlias, null) as? SecretKey) ?: KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder(keyAlias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build())
        }.generateKey()
    }
    @Synchronized fun get(platform: Platform): String {
        val value = storage.getString(platform.name, null) ?: return ""
        val bytes = Base64.decode(value, Base64.NO_WRAP)
        require(bytes.size >= 28) { "登录凭证损坏，请重新登录" }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, bytes.copyOfRange(0, 12)))
        cipher.updateAAD(platform.name.toByteArray())
        return cipher.doFinal(bytes.copyOfRange(12, bytes.size)).toString(Charsets.UTF_8)
    }
    @Synchronized fun save(platform: Platform, cookie: String) {
        require(!cookie.contains('\r') && !cookie.contains('\n') && cookie.length <= 64 * 1024) { "登录凭证格式无效" }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key()); cipher.updateAAD(platform.name.toByteArray())
        val encrypted = Base64.encodeToString(cipher.iv + cipher.doFinal(cookie.toByteArray()), Base64.NO_WRAP)
        val previous=storage.getString(platform.name,null)
        if(!storage.edit().putString(platform.name,encrypted).commit()) {
            // SharedPreferences updates memory even when persisting to disk fails.
            val rollback=storage.edit()
            if(previous==null) rollback.remove(platform.name) else rollback.putString(platform.name,previous)
            rollback.commit()
            throw IllegalStateException("登录信息保存失败")
        }
    }
    @Synchronized fun clear(platform: Platform) { check(storage.edit().remove(platform.name).commit()) { "登录信息清除失败" } }
}

@Entity(tableName = "saved_items")
data class SavedItem(@PrimaryKey val key: String, val kind: String, val payload: String, val position: Int = 0)
@Dao
interface SavedDao {
    @Query("SELECT * FROM saved_items ORDER BY position, `key`") fun observe(): Flow<List<SavedItem>>
    @Query("SELECT * FROM saved_items ORDER BY position, `key`") suspend fun all(): List<SavedItem>
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun put(item: SavedItem)
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun putAll(items: List<SavedItem>)
    @Query("DELETE FROM saved_items WHERE `key` = :key") suspend fun remove(key: String)
    @Query("DELETE FROM saved_items") suspend fun clear()
}
@Database(entities = [SavedItem::class], version = 1, exportSchema = false)
abstract class LiveDatabase : RoomDatabase() { abstract fun items(): SavedDao }

fun Room.toJson(): JSONObject = JSONObject().put("platform", platform.name).put("id", id).put("nickname", name)
    .put("roomTitle", title).put("avatarUrl", avatar).put("cover", cover).put("liveStatus", status.name)
    .put("currentRoomId", realId).put("userId", userId).put("extra", JSONObject(extra))
fun roomFromJson(value: JSONObject): Room = Room(
    Platform.valueOf(value.getString("platform")), value.getString("id"), value.str("nickname"), value.str("roomTitle"),
    value.str("cover"), value.str("avatarUrl"), LiveStatus.valueOf(value.str("liveStatus", if(value.has("isLive") && !value.isNull("isLive")) {if(value.optBoolean("isLive")) "LIVE" else "OFFLINE"} else "UNKNOWN")),
    realId = value.str("currentRoomId", value.str("id")), userId = value.str("userId"),
    extra = value.obj("extra").let { extra -> extra.keys().asSequence().associateWith { extra.str(it) } },
)
data class Follow(val room: Room, val folder: String = "", val pinned: Boolean = false, val displayName: String = "", val position: Int = 0)
data class Folder(val id: String, val name: String, val expanded: Boolean = true, val position: Int = 0)
data class Subscription(val platform: Platform, val category: Category, val raw: String, val key: String)
data class Library(val follows: List<Follow> = emptyList(), val folders: List<Folder> = emptyList(), val subscriptions: List<Subscription> = emptyList())

class Storage(private val context: Context) {
    private val database = androidx.room.Room.databaseBuilder(context, LiveDatabase::class.java, "simplelive.db").build()
    private val dao = database.items()
    private val mutex = Mutex()
    val preferences: Flow<Map<String, String>> = context.nativePreferences.data.map { values -> values.asMap().entries.associate { it.key.name to it.value.toString() } }
    val library: Flow<Library> = dao.observe().map { records ->
        Library(records.filter { it.kind == "follow" }.map {
            val json = JSONObject(it.payload); Follow(roomFromJson(json), json.str("folder"), json.optBoolean("isPinned"), json.str("displayName"), it.position)
        }, records.filter { it.kind == "folder" }.map { val json = JSONObject(it.payload); Folder(it.key.removePrefix("folder:"), json.str("name"), json.optBoolean("expanded", true), it.position) },
            records.filter { it.kind == "category" }.map { subscription(JSONObject(it.payload)) })
    }
    suspend fun recoverImport() = mutex.withLock {
        restorePendingImport()
    }
    private suspend fun restorePendingImport() {
        val pending=dao.all().firstOrNull { it.kind=="import_settings" } ?: return
        val snapshot=JSONObject(pending.payload)
        replacePreferences(snapshot.getJSONObject("preferences"))
        val records=snapshot.getJSONArray("records").objects().map { SavedItem(it.getString("key"),it.getString("kind"),it.getString("payload"),it.getInt("position")) }
        database.withTransaction { dao.clear();dao.putAll(records) }
    }
    suspend fun preference(key: String, value: String) = mutex.withLock { context.nativePreferences.edit { it[stringPreferencesKey(key)] = value } }
    suspend fun follow(room: Room) = mutex.withLock {
        val existing = dao.all().firstOrNull { it.key == room.key }
        if (existing == null) dao.put(SavedItem(room.key, "follow", room.toJson().put("followedAt", System.currentTimeMillis()).toString(), (dao.all().maxOfOrNull { it.position } ?: -1)+1))
    }
    suspend fun unfollow(room: Room) = mutex.withLock { dao.remove(room.key) }
    suspend fun editFollow(follow: Follow) = mutex.withLock {
        val record=dao.all().firstOrNull { it.key==follow.room.key && it.kind=="follow" } ?: return@withLock
        val payload=JSONObject(record.payload).put("folder",follow.folder).put("isPinned",follow.pinned).put("displayName",follow.displayName)
        dao.put(record.copy(payload=payload.toString()))
    }
    suspend fun refreshRoom(key: String, room: Room) = mutex.withLock {
        val record=dao.all().firstOrNull { it.key==key && it.kind=="follow" } ?: return@withLock
        val previous=JSONObject(record.payload)
        val updated=room.toJson()
        updated.keys().forEach { previous.put(it,updated.get(it)) }
        previous.put("lastUpdated",System.currentTimeMillis())
        dao.put(record.copy(payload=previous.toString()))
    }
    suspend fun folder(folder: Folder) = mutex.withLock {
        val records=dao.all()
        val position=records.firstOrNull { it.key=="folder:${folder.id}" }?.position ?: ((records.maxOfOrNull { it.position } ?: -1)+1)
        dao.put(SavedItem("folder:${folder.id}", "folder", JSONObject().put("id",folder.id).put("name", folder.name).put("expanded", folder.expanded).toString(), position))
    }
    suspend fun deleteFolder(folder: Folder) = mutex.withLock {
        database.withTransaction {
            dao.remove("folder:${folder.id}")
            dao.all().filter { it.kind == "follow" }.forEach { item -> val json = JSONObject(item.payload); if(json.str("folder")==folder.id) dao.put(item.copy(payload=json.put("folder", "").toString())) }
        }
    }
    suspend fun swap(first: String, second: String) = mutex.withLock {
        database.withTransaction {
            val records=dao.all(); val left=records.firstOrNull { it.key==first } ?: return@withTransaction
            val right=records.firstOrNull { it.key==second } ?: return@withTransaction
            dao.put(left.copy(position=right.position)); dao.put(right.copy(position=left.position))
        }
    }
    suspend fun swapFollows(first: Follow, second: Follow) = mutex.withLock {
        database.withTransaction {
            val records=dao.all()
            val left=records.firstOrNull { it.key==first.room.key && it.kind=="follow" } ?: return@withTransaction
            val right=records.firstOrNull { it.key==second.room.key && it.kind=="follow" } ?: return@withTransaction
            val leftData=JSONObject(left.payload);val rightData=JSONObject(right.payload)
            if(first.folder!=second.folder || first.pinned!=second.pinned ||
                leftData.str("folder")!=first.folder || rightData.str("folder")!=second.folder ||
                leftData.optBoolean("isPinned")!=first.pinned || rightData.optBoolean("isPinned")!=second.pinned ||
                left.position!=first.position || right.position!=second.position) return@withTransaction
            dao.put(left.copy(position=right.position));dao.put(right.copy(position=left.position))
        }
    }
    suspend fun subscribe(platform: Platform, category: Category) = mutex.withLock {
        val raw = JSONObject().put("platform",platform.name.lowercase()).put("categoryLevel",if(category.level==3) "cate3" else "cate2")
            .put("cate2Name",category.name).put("cate2Id",if(category.level==3) category.parent else category.id).put("cate1Name",category.parent)
            .put("douyuShortName",category.queryId)
            .put("cate2Href",when(platform) { Platform.BILIBILI -> "/category/${category.parent}_${category.id}"; Platform.HUYA -> "https://www.huya.com/g/${category.id}"; Platform.DOUYIN -> "/category/${category.id}"; else -> category.id })
        if(category.level==3) raw.put("cate3Id",category.id).put("cate3Name",category.name)
        val key="${platform.name.lowercase()}:cate${category.level}:${category.parent}:${category.id}"
        raw.put("key",key)
        if(dao.all().any { it.key=="category:$key" }) dao.remove("category:$key") else dao.put(SavedItem("category:$key","category",raw.toString(),dao.all().size))
    }
    suspend fun removeSubscription(key: String) = mutex.withLock { dao.remove("category:$key") }
    private fun subscription(raw: JSONObject): Subscription {
        val platform=Platform.valueOf(raw.getString("platform").uppercase())
        val level=if(raw.str("categoryLevel")=="cate3") 3 else 2
        val href=raw.str("cate2Href").substringAfterLast('/')
        val id=when(platform) {
            Platform.DOUYU -> if(level==3) raw.str("cate3Id") else raw.str("cate2Id").ifBlank { raw.str("douyuShortName",raw.str("douyuId")) }
            Platform.BILIBILI -> href.substringAfterLast('_')
            else -> href
        }
        val parent=if(platform==Platform.BILIBILI) href.substringBefore('_') else if(level==3) raw.str("cate2Id") else raw.str("cate1Name")
        return Subscription(platform,Category(id,if(level==3) raw.str("cate3Name") else raw.str("cate2Name"),parent,level,if(platform==Platform.DOUYU) raw.str("douyuShortName",raw.str("douyuId",id)) else id),raw.toString(),raw.str("key","${platform.name}:$id"))
    }
    private val libraryKeys=setOf("followedStreamers","followFolders","followPinnedKeys","followListOrder","dtv_custom_categories_v1","dtv_custom_categories_v2")
    private fun portableKey(key: String): Boolean = key in libraryKeys || key in setOf("danmu_block_keywords","dtv_danmu_preferences_v1","dtv_player_danmu_collapsed","dtv_player_volume_v1","theme_preference") || Regex("(DOUYU|HUYA|DOUYIN|BILIBILI)_preferred_(quality|line)").matches(key)
    suspend fun export(): String = mutex.withLock {
        restorePendingImport()
        val records=dao.all(); val lib=library.first(); val entries=JSONObject()
        preferences.first().filterKeys(::portableKey).forEach { (key,value) -> entries.put(key,value) }
        entries.put("followedStreamers",JSONArray(lib.follows.map { follow -> JSONObject(records.first { it.key==follow.room.key }.payload).apply { remove("folder") } }).toString())
        val folderValues=lib.folders.map { folder -> JSONObject().put("id",folder.id).put("name",folder.name).put("expanded",folder.expanded).put("streamerIds",JSONArray(lib.follows.filter { it.folder==folder.id }.map { it.room.key })) }
        entries.put("followFolders",JSONArray(folderValues).toString())
        entries.put("followPinnedKeys",JSONArray(lib.follows.filter { it.pinned }.map { it.room.key }).toString())
        entries.put("followListOrder",JSONArray(records.filter { it.kind=="follow" || it.kind=="folder" }.map { record ->
            val data=if(record.kind=="folder") folderValues.first { "folder:${it.str("id")}"==record.key } else JSONObject(record.payload).apply { remove("folder") }
            JSONObject().put("type",if(record.kind=="follow") "streamer" else "folder").put("data",data)
        }).toString())
        entries.put("dtv_custom_categories_v2",JSONArray(lib.subscriptions.map { JSONObject(it.raw) }).toString())
        JSONObject().put("kind","dtv-config").put("version",1).put("exportedAt",java.text.SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ssXXX",java.util.Locale.ROOT).format(java.util.Date()))
            .put("source",JSONObject().put("client","android-native").put("appVersion",BuildConfig.VERSION_NAME)).put("entries",entries).toString(2)
    }
    /** Validate before mutation; the rollback journal survives interruptions across both stores. */
    suspend fun importConfig(text: String) = mutex.withLock {
        restorePendingImport()
        require(text.length <= 4 * 1024 * 1024) { "配置文件过大" }
        val payload=JSONObject(text)
        require(payload.str("kind")=="dtv-config" && payload.optInt("version")==1) { "不支持的配置版本" }
        val source=payload.getJSONObject("entries"); val entries=JSONObject()
        source.keys().forEach { key -> if(portableKey(key)) { require(source.get(key) is String) { "配置值必须是文本" }; entries.put(key,source.getString(key)) } }
        require(source.length()==0 || entries.length()>0) { "备份中没有可迁移配置，未修改现有数据" }
        fun array(key: String): JSONArray = JSONArray(entries.str(key,"[]"))
        fun checkedObjects(key: String): List<JSONObject> { val value=array(key); require(value.objects().size==value.length()) { "$key 格式无效" }; return value.objects() }
        val sourceFollows=checkedObjects("followedStreamers")
        sourceFollows.forEach { value ->
            require(value.get("id") is String && value.getString("id").isNotBlank() && value.get("nickname") is String && value.get("avatarUrl") is String) { "关注内容无效" }
            require(value.getString("platform") in Platform.entries.map { it.name } + listOf("KUAISHOU","NETEASECC","CUSTOM_M3U8")) { "关注平台无效" }
            listOf("displayName","roomTitle","currentRoomId").forEach { field -> if(value.has(field)) require(value.get(field) is String) { "关注字段类型无效" } }
            if(value.has("isPinned")) require(value.get("isPinned") is Boolean)
            if(value.has("isLive") && !value.isNull("isLive")) require(value.get("isLive") is Boolean)
            if(value.has("liveStatus")) require(value.getString("liveStatus") in LiveStatus.entries.map { it.name })
        }
        val follows=sourceFollows.filter { value -> value.str("platform") in Platform.entries.map { it.name } }.distinctBy { "${it.str("platform")}:${it.str("id")}" }
        require(follows.map { "${it.str("platform")}:${it.str("id")}" }.distinct().size==follows.size) { "关注列表包含重复项" }
        val folders=checkedObjects("followFolders")
        folders.forEach { value -> require(value.getString("id").isNotBlank() && value.getString("name").isNotBlank()); val members=value.getJSONArray("streamerIds"); require((0 until members.length()).all { members.get(it) is String }) }
        require(folders.map { it.str("id") }.distinct().size==folders.size)
        val pinned=array("followPinnedKeys"); require((0 until pinned.length()).all { pinned.get(it) is String })
        val order=checkedObjects("followListOrder").map { item ->
            val value=item.getJSONObject("data")
            when(item.getString("type")) { "folder" -> "folder:${value.getString("id")}"; "streamer" -> "${value.getString("platform")}:${value.getString("id")}"; else -> throw IllegalArgumentException("排序内容无效") }
        }
        val categories=checkedObjects(if(entries.has("dtv_custom_categories_v2")) "dtv_custom_categories_v2" else "dtv_custom_categories_v1")
        categories.forEach { raw ->
            listOf("key","platform","categoryLevel","cate1Name","cate1Href","cate2Name","cate2Href","cate2Id","cate3Id","cate3Name","douyuShortName","douyuId").forEach { field -> if(raw.has(field)) require(raw.get(field) is String) { "分类字段类型无效" } }
            require(raw.getString("cate2Name").isNotBlank());val value=subscription(raw);require(value.category.id.isNotBlank())
            require(raw.str("categoryLevel","cate2") in listOf("cate2","cate3"))
            if(value.platform!=Platform.DOUYU) require(value.category.level==2 && raw.str("cate2Href").isNotBlank())
            if(value.category.level==3) require(raw.str("cate3Name").isNotBlank())
        }
        validatePreferences(entries)
        val records=mutableListOf<SavedItem>()
        follows.forEachIndexed { index,value ->
            val key=roomFromJson(value).key
            val folder=folders.firstOrNull { it.arr("streamerIds").strings().any { member -> member==key || member==value.str("id") } }?.str("id").orEmpty()
            value.put("folder",folder).put("isPinned",key in pinned.strings() || value.optBoolean("isPinned"))
            records += SavedItem(key,"follow",value.toString(),order.indexOf(key).takeIf { it>=0 } ?: (order.size+index))
        }
        folders.forEachIndexed { index,value -> val key="folder:${value.str("id")}"; records += SavedItem(key,"folder",value.toString(),order.indexOf(key).takeIf { it>=0 } ?: (order.size+follows.size+index)) }
        categories.forEachIndexed { index,value -> val category=subscription(value); records += SavedItem("category:${category.key}","category",value.toString(),index) }
        val settings=JSONObject(); entries.keys().forEach { key -> if(key !in libraryKeys) settings.put(key,entries.getString(key)) }
        val previous=JSONObject().put("preferences",JSONObject(preferences.first())).put("records",JSONArray(dao.all().map {
            JSONObject().put("key",it.key).put("kind",it.kind).put("payload",it.payload).put("position",it.position)
        }))
        records += SavedItem("pending-import","import_settings",previous.toString())
        withContext(NonCancellable) {
            database.withTransaction { dao.clear(); dao.putAll(records) }
            try { replacePreferences(settings); dao.remove("pending-import") }
            catch(error: Exception) {
                try { restorePendingImport() } catch(recoveryError: Exception) { throw IllegalStateException("导入恢复未完成，请释放存储空间并重新启动应用",recoveryError) }
                throw IllegalStateException("导入失败，原配置已恢复",error)
            }
        }
    }
    private suspend fun replacePreferences(values: JSONObject) { context.nativePreferences.edit { stored -> stored.clear(); values.keys().forEach { stored[stringPreferencesKey(it)] = values.getString(it) } } }
    private fun validatePreferences(entries: JSONObject) {
        if(entries.has("theme_preference")) require(entries.getString("theme_preference") in listOf("light","dark","system"))
        if(entries.has("dtv_player_volume_v1")) require(entries.getString("dtv_player_volume_v1").toDouble() in 0.0..1.0)
        if(entries.has("dtv_player_danmu_collapsed")) require(entries.getString("dtv_player_danmu_collapsed") in listOf("true","false"))
        if(entries.has("danmu_block_keywords")) { val words=JSONArray(entries.getString("danmu_block_keywords")); require((0 until words.length()).all { words.get(it) is String }) }
        if(entries.has("dtv_danmu_preferences_v1")) {
            val data=JSONObject(entries.getString("dtv_danmu_preferences_v1")); require(data.get("enabled") is Boolean)
            val settings=data.getJSONObject("settings")
            if(settings.has("opacity")) require(settings.getDouble("opacity") in 0.2..1.0)
            if(settings.has("area")) require(settings.getDouble("area") > 0 && settings.getDouble("area") <= 1)
            if(settings.has("duration")) require(settings.getDouble("duration") > 0)
            if(settings.has("fontSize")) require(Regex("[0-9]+(?:\\.[0-9]+)?px").matches(settings.getString("fontSize")) && settings.getString("fontSize").removeSuffix("px").toFloat() > 0)
            if(settings.has("mode")) require(settings.getString("mode") in listOf("scroll","top","bottom"))
            if(settings.has("density")) require(settings.getString("density") in listOf("dense","medium","sparse"))
        }
        entries.keys().forEach { key -> if(key.endsWith("_preferred_quality") || key.endsWith("_preferred_line")) require(entries.getString(key).isNotBlank()) }
    }
}
