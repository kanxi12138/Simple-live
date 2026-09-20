package com.simplelive.nativeapp

import java.io.ByteArrayOutputStream
import java.io.DataOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Minimal bounded TARS reader/writer for Huya WUP and websocket messages. */
sealed interface TarsValue {
    data class Number(val value: Long) : TarsValue
    data class Text(val value: String) : TarsValue
    data class Bytes(val value: ByteArray) : TarsValue
    data class Struct(val value: Map<Int, TarsValue>) : TarsValue
    data class ListValue(val value: List<TarsValue>) : TarsValue
    data class MapValue(val value: List<Pair<TarsValue, TarsValue>>) : TarsValue
}
class TarsWriter {
    private val bytes = ByteArrayOutputStream()
    private val output = DataOutputStream(bytes)
    private fun head(tag: Int, kind: Int) { if (tag < 15) output.writeByte(tag shl 4 or kind) else { output.writeByte(240 or kind); output.writeByte(tag) } }
    fun number(tag: Int, value: Long): TarsWriter = apply {
        when {
            value == 0L -> head(tag, 12)
            value in -128..127 -> { head(tag, 0); output.writeByte(value.toInt()) }
            value in -32768..32767 -> { head(tag, 1); output.writeShort(value.toInt()) }
            value in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong() -> { head(tag, 2); output.writeInt(value.toInt()) }
            else -> { head(tag, 3); output.writeLong(value) }
        }
    }
    fun text(tag: Int, value: String): TarsWriter = apply {
        val data = value.toByteArray()
        if (data.size < 256) { head(tag, 6); output.writeByte(data.size) } else { head(tag, 7); output.writeInt(data.size) }
        output.write(data)
    }
    fun blob(tag: Int, value: ByteArray): TarsWriter = apply { head(tag, 13); head(0, 0); number(0, value.size.toLong()); output.write(value) }
    fun struct(tag: Int, fields: TarsWriter.() -> Unit): TarsWriter = apply { head(tag, 10); fields(); head(0, 11) }
    fun stringBytesMap(tag: Int, entries: Map<String, ByteArray>): TarsWriter = apply {
        head(tag, 8); number(0, entries.size.toLong()); entries.forEach { (key, value) -> text(0, key); blob(1, value) }
    }
    fun build(): ByteArray = bytes.toByteArray()
}
class TarsReader(data: ByteArray) {
    private val input = ByteBuffer.wrap(data).order(ByteOrder.BIG_ENDIAN)
    fun fields(depth: Int = 0): Map<Int, TarsValue> {
        require(depth < 24) { "TARS 嵌套过深" }
        val fields = linkedMapOf<Int, TarsValue>()
        while (input.hasRemaining()) {
            val first = input.get().toInt() and 255
            val tag = if (first ushr 4 == 15) input.get().toInt() and 255 else first ushr 4
            val kind = first and 15
            if (kind == 11) break
            fields[tag] = value(kind, depth)
        }
        return fields
    }
    private fun next(depth: Int): TarsValue {
        val first = input.get().toInt() and 255
        if (first ushr 4 == 15) input.get()
        return value(first and 15, depth)
    }
    private fun count(depth: Int): Int {
        val count = (next(depth) as TarsValue.Number).value
        require(count in 0..1_048_576) { "TARS 长度无效" }
        return count.toInt()
    }
    private fun bytes(length: Int): ByteArray { require(length in 0..input.remaining()); return ByteArray(length).also { input.get(it) } }
    private fun value(kind: Int, depth: Int): TarsValue {
        require(depth < 24)
        return when (kind) {
            0 -> TarsValue.Number(input.get().toLong())
            1 -> TarsValue.Number(input.short.toLong())
            2 -> TarsValue.Number(input.int.toLong())
            3 -> TarsValue.Number(input.long)
            4 -> TarsValue.Number(input.float.toLong())
            5 -> TarsValue.Number(input.double.toLong())
            6 -> TarsValue.Text(bytes(input.get().toInt() and 255).toString(Charsets.UTF_8))
            7 -> TarsValue.Text(bytes(input.int).toString(Charsets.UTF_8))
            8 -> TarsValue.MapValue(List(count(depth)) { next(depth + 1) to next(depth + 1) })
            9 -> TarsValue.ListValue(List(count(depth)) { next(depth + 1) })
            10 -> TarsValue.Struct(fields(depth + 1))
            12 -> TarsValue.Number(0)
            13 -> { require((input.get().toInt() and 15) == 0); TarsValue.Bytes(bytes(count(depth))) }
            else -> throw PlatformException("TARS 数据类型无效")
        }
    }
}
fun Map<Int, TarsValue>.number(tag: Int): Long = (get(tag) as? TarsValue.Number)?.value ?: 0
fun Map<Int, TarsValue>.text(tag: Int): String = (get(tag) as? TarsValue.Text)?.value.orEmpty()
fun Map<Int, TarsValue>.blob(tag: Int): ByteArray = (get(tag) as? TarsValue.Bytes)?.value ?: byteArrayOf()
fun Map<Int, TarsValue>.struct(tag: Int): Map<Int, TarsValue> = (get(tag) as? TarsValue.Struct)?.value ?: emptyMap()

/** Protobuf wire reader preserves repeated fields and checks every length before slicing. */
class Proto(data: ByteArray) {
    val fields = linkedMapOf<Int, MutableList<ByteArray>>()
    private val input = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN)
    init {
        while (input.hasRemaining()) {
            val key = varint()
            val tag = (key ushr 3).toInt()
            require(tag > 0)
            val value = when ((key and 7).toInt()) {
                0 -> ByteBuffer.allocate(8).putLong(varint()).array()
                1 -> take(8)
                2 -> { val length = varint(); require(length in 0..input.remaining().toLong()); take(length.toInt()) }
                5 -> take(4)
                else -> throw PlatformException("Protobuf 编码无效")
            }
            fields.getOrPut(tag) { mutableListOf() }.add(value)
        }
    }
    private fun take(length: Int): ByteArray { require(length <= input.remaining()); return ByteArray(length).also { input.get(it) } }
    private fun varint(): Long {
        var value = 0L
        for (index in 0..9) {
            val part = input.get().toInt() and 255
            value = value or ((part and 127).toLong() shl (7 * index))
            if (part and 128 == 0) return value
        }
        throw PlatformException("Protobuf 整数无效")
    }
    fun bytes(tag: Int): ByteArray = fields[tag]?.firstOrNull() ?: byteArrayOf()
    fun text(tag: Int): String = bytes(tag).toString(Charsets.UTF_8)
    fun number(tag: Int): Long = bytes(tag).let { if (it.size == 8) ByteBuffer.wrap(it).long else 0L }
    fun child(tag: Int): Proto = Proto(bytes(tag))
    companion object {
        private fun writeInt(output: ByteArrayOutputStream, number: Long) {
            var remainder = number
            while (remainder and -128L != 0L) { output.write((remainder.toInt() and 127) or 128); remainder = remainder ushr 7 }
            output.write(remainder.toInt())
        }
        fun encode(numbers: Map<Int, Long> = emptyMap(), blobs: Map<Int, ByteArray> = emptyMap()): ByteArray = ByteArrayOutputStream().apply {
            numbers.forEach { (tag, value) -> writeInt(this, (tag.toLong() shl 3)); writeInt(this, value) }
            blobs.forEach { (tag, value) -> writeInt(this, (tag.toLong() shl 3) or 2); writeInt(this, value.size.toLong()); write(value) }
        }.toByteArray()
    }
}
