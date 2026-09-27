package com.simplelive.nativeapp

import android.app.Application
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

class LiveApplication : Application() {
    val applicationScope=CoroutineScope(SupervisorJob()+Dispatchers.Main.immediate)
    val playback by lazy { PlaybackConnection(this) }
    val network by lazy { Network() }
    val storage by lazy { Storage(this) }
    val credentials by lazy { Credentials(this) }
    val signer by lazy { SignatureEngine(this) }
    val platforms: Map<Platform,LivePlatform> by lazy { mapOf(
        Platform.DOUYU to DouyuPlatform(network,signer),
        Platform.HUYA to HuyaPlatform(this,network),
        Platform.DOUYIN to DouyinPlatform(this,network,credentials),
        Platform.BILIBILI to BilibiliPlatform(this,network,credentials),
    ) }
    val danmaku by lazy { DanmakuClient(network,signer,platforms.getValue(Platform.BILIBILI) as BilibiliPlatform,platforms.getValue(Platform.DOUYIN) as DouyinPlatform) }
}
