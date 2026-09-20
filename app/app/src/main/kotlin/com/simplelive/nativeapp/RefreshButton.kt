package com.simplelive.nativeapp

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.compose.runtime.collectAsState

@Composable
fun RefreshButton(refreshing: Boolean,description: String,onClick: ()->Unit) {
    val lifecycle by LocalLifecycleOwner.current.lifecycle.currentStateFlow.collectAsState()
    val rotation=if(refreshing && lifecycle.isAtLeast(Lifecycle.State.STARTED)) {
        val transition=rememberInfiniteTransition(label="refresh")
        val angle by transition.animateFloat(0f,360f,
            infiniteRepeatable(tween(900,easing=LinearEasing),RepeatMode.Restart),label="refreshRotation")
        angle
    } else 0f
    IconButton(onClick=onClick,enabled=!refreshing) {
        Icon(Icons.Outlined.Refresh,if(refreshing) "$description，刷新中" else description,Modifier.rotate(rotation))
    }
}
