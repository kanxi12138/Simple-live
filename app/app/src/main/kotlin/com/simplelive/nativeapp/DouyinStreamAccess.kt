package com.simplelive.nativeapp

import androidx.media3.common.util.UriUtil
import androidx.media3.exoplayer.hls.playlist.DefaultHlsPlaylistParserFactory
import androidx.media3.exoplayer.hls.playlist.HlsMediaPlaylist
import androidx.media3.exoplayer.hls.playlist.HlsMultivariantPlaylist
import androidx.media3.exoplayer.hls.playlist.HlsPlaylist
import androidx.media3.exoplayer.hls.playlist.HlsPlaylistParserFactory
import androidx.media3.exoplayer.upstream.ParsingLoadable
import okhttp3.Dns
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.Interceptor
import okhttp3.Response
import java.net.InetAddress
import java.util.concurrent.ConcurrentHashMap

/** Exact resource permissions derived from this playback's validated entry and playlists. */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
class DouyinStreamAccess(entry: String) : Interceptor, HlsPlaylistParserFactory {
    private val resources=ConcurrentHashMap<String,Long>()
    private val playlists=ConcurrentHashMap.newKeySet<String>()
    private val parserFactory=DefaultHlsPlaylistParserFactory()
    @Volatile private var active=true

    private val entryKey=key(grant(validateStream(Platform.DOUYIN,entry),"entry"))

    // Check DNS results before OkHttp connects, including redirects to new hosts.
    val dns=object: Dns {
        override fun lookup(hostname: String): List<InetAddress> = Dns.SYSTEM.lookup(hostname).also { addresses ->
            if(addresses.any { !isPublic(it) }) reject(hostname,"dns","non_public_address")
        }
    }

    @Synchronized fun close() { active=false;resources.clear();playlists.clear() }

    private fun reject(host: String,stage: String,reason: String): Nothing {
        android.util.Log.w("StreamPolicy","DOUYIN: rejected host=$host stage=$stage reason=$reason")
        throw StreamAddressException()
    }

    private fun address(value: String,stage: String): HttpUrl {
        val url=value.toHttpUrlOrNull() ?: reject("invalid",stage,"invalid_url")
        if(url.username.isNotEmpty() || url.password.isNotEmpty() || url.port !in listOf(80,443)) {
            reject(url.host,stage,"credentials_or_port")
        }
        // Literal addresses bypass OkHttp's DNS implementation.
        if(url.host.contains(':') || url.host.all { it.isDigit() || it=='.' }) {
            if(!isPublic(InetAddress.getByName(url.host))) reject(url.host,stage,"non_public_address")
        }
        return url
    }

    private fun playlistKey(url: HttpUrl): String = url.newBuilder().apply {
        listOf("_HLS_msn","_HLS_part","_HLS_skip").forEach { removeAllQueryParameters(it) }
    }.fragment(null).build().toString()

    private fun key(url: HttpUrl): String = url.newBuilder().fragment(null).build().toString()

    @Synchronized private fun grant(value: String,stage: String,playlist: Boolean=false): HttpUrl {
        val url=address(value,stage)
        if(!active) reject(url.host,stage,"playback_closed")
        val now=android.os.SystemClock.elapsedRealtime()
        // Live playlists renew permissions; old segments do not accumulate indefinitely.
        resources.entries.removeAll { now-it.value>600_000 }
        if(resources.size>=8192) resources.minByOrNull { it.value }?.let { resources.remove(it.key) }
        resources[key(url)]=now
        if(playlist) playlists.add(playlistKey(url))
        return url
    }

    private fun requireAllowed(value: String,stage: String): HttpUrl {
        val url=address(value,stage)
        if(!active) reject(url.host,stage,"playback_closed")
        val allowedAt=resources[key(url)]
        if(key(url)!=entryKey && (allowedAt==null || android.os.SystemClock.elapsedRealtime()-allowedAt>600_000) && playlistKey(url) !in playlists) {
            reject(url.host,stage,"not_referenced")
        }
        return url
    }

    override fun intercept(chain: Interceptor.Chain): Response {
        var request=chain.request().newBuilder().removeHeader("Cookie").removeHeader("Authorization").build()
        repeat(6) { hop ->
            val source=requireAllowed(request.url.toString(),if(hop==0) "resource" else "redirect")
            val response=chain.proceed(request)
            if(response.code !in listOf(301,302,303,307,308)) return response
            val location=response.header("Location") ?: return response
            val target=source.resolve(location)
            response.close()
            if(hop==5) reject(source.host,"redirect","too_many_redirects")
            if(target==null) reject(source.host,"redirect","invalid_location")
            grant(target.toString(),"redirect",playlistKey(source) in playlists)
            request=request.newBuilder().url(target).build()
        }
        throw StreamAddressException()
    }

    override fun createPlaylistParser(): ParsingLoadable.Parser<HlsPlaylist> = wrap(parserFactory.createPlaylistParser())

    override fun createPlaylistParser(multivariantPlaylist: HlsMultivariantPlaylist,previousMediaPlaylist: HlsMediaPlaylist?): ParsingLoadable.Parser<HlsPlaylist> =
        wrap(parserFactory.createPlaylistParser(multivariantPlaylist,previousMediaPlaylist))

    private fun wrap(parser: ParsingLoadable.Parser<HlsPlaylist>): ParsingLoadable.Parser<HlsPlaylist> = ParsingLoadable.Parser { uri,input ->
        requireAllowed(uri.toString(),"hls_playlist")
        val playlist=parser.parse(uri,input)
        grant(uri.toString(),"hls_playlist",true)
        fun reference(value: String,isPlaylist: Boolean=false) {
            grant(UriUtil.resolve(uri.toString(),value),"hls_reference",isPlaylist)
        }
        fun segment(value: HlsMediaPlaylist.SegmentBase) {
            reference(value.url)
            value.fullSegmentEncryptionKeyUri?.let { reference(it) }
            value.initializationSegment?.let { init ->
                reference(init.url)
                init.fullSegmentEncryptionKeyUri?.let { reference(it) }
            }
        }
        when(playlist) {
            is HlsMultivariantPlaylist -> playlist.mediaPlaylistUrls.forEach { reference(it.toString(),true) }
            is HlsMediaPlaylist -> {
                playlist.segments.forEach { entry -> segment(entry);entry.parts.forEach { segment(it) } }
                playlist.trailingParts.forEach { segment(it) }
                playlist.renditionReports.keys.forEach { reference(it.toString(),true) }
            }
        }
        playlist
    }

    private fun isPublic(address: InetAddress): Boolean {
        if(address.isAnyLocalAddress || address.isLoopbackAddress || address.isLinkLocalAddress || address.isSiteLocalAddress || address.isMulticastAddress) return false
        val bytes=address.address.map { it.toInt() and 255 }
        if(bytes.size==4) {
            val first=bytes[0];val second=bytes[1];val third=bytes[2]
            return first !in listOf(0,10,127) && first<224 &&
                !(first==100 && second in 64..127) && !(first==169 && second==254) &&
                !(first==172 && second in 16..31) && !(first==192 && second==168) &&
                !(first==192 && second==0 && third in listOf(0,2)) &&
                !(first==198 && (second in 18..19 || (second==51 && third==100))) &&
                !(first==203 && second==0 && third==113)
        }
        // Only global IPv6 unicast; exclude special-purpose and transition prefixes.
        return bytes.size==16 && bytes[0] in 0x20..0x3f &&
            !(bytes[0]==0x20 && bytes[1]==0x01 && (bytes[2]<2 || (bytes[2]==0x0d && bytes[3]==0xb8))) &&
            !(bytes[0]==0x20 && bytes[1]==0x02)
    }
}
