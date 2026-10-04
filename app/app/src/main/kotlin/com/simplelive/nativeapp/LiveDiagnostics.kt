package com.simplelive.nativeapp

import android.content.Context
import android.util.Log
import java.io.File
import java.io.IOException
import java.util.concurrent.Executors

/** Bounded, content-free diagnostics for devices that suppress application logcat. */
object LiveDiagnostics {
    private var directory: File?=null
    private val writer=Executors.newSingleThreadExecutor()
    fun initialize(context: Context) { directory=context.getExternalFilesDir(null) }
    fun record(tag: String,event: String) {
        Log.i(tag,event)
        val root=directory ?: return
        writer.execute {
            try {
                val file=File(root,"live-diagnostics.log")
                if(file.length()>1_048_576) file.writeText("")
                file.appendText("${System.currentTimeMillis()} $tag $event\n")
            } catch(error: IOException) { Log.w("LiveDiagnostics","write_failed",error) }
        }
    }
}
