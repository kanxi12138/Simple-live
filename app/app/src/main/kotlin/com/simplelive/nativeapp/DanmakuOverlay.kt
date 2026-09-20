package com.simplelive.nativeapp

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.view.Choreographer
import android.view.View
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch

/** The screen queue owns tracks independently of the bounded message history. */
@Composable
fun DanmakuOverlay(model: LiveViewModel,revision: Int,settings: DanmakuSettings,blocked: List<String>,modifier: Modifier) {
    val context=LocalContext.current
    val lifecycle=LocalLifecycleOwner.current.lifecycle
    val fontSize=with(LocalDensity.current) { settings.fontSize.sp.toPx() }
    val overlay=remember(context,revision) { DanmakuCanvas(context) }
    AndroidView(factory={overlay},modifier=modifier,update={it.configure(settings,fontSize,blocked)})
    LaunchedEffect(overlay,lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            overlay.start()
            try {
                coroutineScope {
                    launch { model.overlayMessages.collect { overlay.accept(it) } }
                    awaitCancellation()
                }
            } finally { overlay.stop() }
        }
    }
    DisposableEffect(overlay) { onDispose { overlay.stop() } }
}

private class DanmakuCanvas(context: Context) : View(context),Choreographer.FrameCallback {
    private data class Entry(val id: Long,val text: String,val width: Float,val row: Int,val started: Long,val duration: Long)
    private val paint=Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color=android.graphics.Color.WHITE
        setShadowLayer(2f,1f,1f,android.graphics.Color.BLACK)
    }
    private val entries=ArrayList<Entry>(512)
    private var settings=DanmakuSettings()
    private var blocked: List<String> = emptyList()
    private var availableAt=LongArray(1)
    private var nextRow=0
    private var lastAccepted=Long.MIN_VALUE
    private var rowHeight=1f
    private var descent=0f
    private var running=false
    private var lifecycleActive=false
    private var scheduled=false
    private var frameTime=0L
    private val choreographer=Choreographer.getInstance()

    fun configure(value: DanmakuSettings,fontSize: Float,words: List<String>) {
        val layoutChanged=paint.textSize!=fontSize || settings.copy(opacity=value.opacity)!=value
        settings=value
        paint.textSize=fontSize
        paint.alpha=(value.opacity*255).toInt().coerceIn(0,255)
        if(blocked!=words) {
            blocked=words
            entries.removeAll { entry -> blocked.any { it.isNotBlank() && entry.text.contains(it,true) } }
        }
        if(layoutChanged) resetLayout()
        invalidate()
    }

    override fun onSizeChanged(width: Int,height: Int,oldWidth: Int,oldHeight: Int) { resetLayout() }

    private fun resetLayout() {
        clear()
        rowHeight=(paint.textSize*1.5f).coerceAtLeast(1f)
        descent=paint.fontMetrics.descent
        availableAt=LongArray((height*settings.area/rowHeight).toInt().coerceIn(1,128))
    }

    fun start() { clear();lifecycleActive=true;running=isAttachedToWindow }
    fun stop() { lifecycleActive=false;running=false;clear() }

    private fun clear() {
        choreographer.removeFrameCallback(this);scheduled=false
        entries.clear();availableAt.fill(0);nextRow=0;lastAccepted=Long.MIN_VALUE
        invalidate()
    }

    fun accept(messages: List<Danmaku>) {
        if(!running || width<=0 || height<=0) return
        val interval=when(settings.density) { "dense" -> 600L;"sparse" -> 2200L;else -> 1200L }
        val now=System.nanoTime()
        for(message in messages) {
            if(blocked.any { it.isNotBlank() && message.text.contains(it,true) }) continue
            // Density limits admission, not the movement of already visible messages.
            if(lastAccepted!=Long.MIN_VALUE && message.arrivedAt-lastAccepted<interval/availableAt.size) continue
            lastAccepted=message.arrivedAt
            val text=message.text.take(160).replace('\n',' ').replace('\r',' ')
            val textWidth=paint.measureText(text)
            val duration=(settings.duration*1_000_000).toLong()
            var row=availableAt.indexOfFirst { it<=now }
            if(row<0) { row=nextRow;nextRow=(nextRow+1)%availableAt.size }
            val headClear=(textWidth/(width+textWidth)*duration).toLong()+150_000_000L
            availableAt[row]=now+if(settings.mode=="scroll") maxOf(interval*1_000_000,headClear) else duration
            if(entries.size==512) entries.removeAt(0)
            entries.add(Entry(message.id,text,textWidth,row,now,duration))
        }
        requestFrame()
    }

    private fun requestFrame() {
        if(running && entries.isNotEmpty() && !scheduled) {
            scheduled=true;choreographer.postFrameCallback(this)
        }
    }

    override fun doFrame(frameTimeNanos: Long) {
        scheduled=false
        if(!running) return
        frameTime=frameTimeNanos
        var index=entries.lastIndex
        while(index>=0) {
            val entry=entries[index]
            if(frameTime-entry.started>=entry.duration) entries.removeAt(index)
            index--
        }
        invalidate();requestFrame()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        var index=0
        while(index<entries.size) {
            val entry=entries[index++]
            val elapsed=((frameTime-entry.started).coerceAtLeast(0).toDouble()/entry.duration).toFloat()
            val horizontal=if(settings.mode=="scroll") width-(width+entry.width)*elapsed else (width-entry.width)/2
            val vertical=if(settings.mode=="bottom") height-entry.row*rowHeight-descent else (entry.row+1)*rowHeight
            canvas.drawText(entry.text,horizontal,vertical,paint)
        }
    }

    override fun onAttachedToWindow() { super.onAttachedToWindow();running=lifecycleActive }
    override fun onDetachedFromWindow() { running=false;clear();super.onDetachedFromWindow() }
}
