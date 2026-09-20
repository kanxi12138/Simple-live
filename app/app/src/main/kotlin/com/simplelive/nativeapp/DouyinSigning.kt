package com.simplelive.nativeapp

import java.nio.ByteBuffer

/** Kotlin port of the existing a_bogus adapter, preserving byte order and SM3 rounds. */
object DouyinSigning {
    private fun sm3(data: ByteArray): ByteArray {
        val paddedSize = ((data.size + 9 + 63) / 64) * 64
        val padded = data.copyOf(paddedSize)
        padded[data.size] = 0x80.toByte()
        ByteBuffer.wrap(padded, paddedSize - 8, 8).putLong(data.size.toLong() * 8)
        val state = intArrayOf(0x7380166f,0x4914b2b9,0x172442d7,0xda8a0600.toInt(),0xa96f30bc.toInt(),0x163138aa,0xe38dee4d.toInt(),0xb0fb0e4e.toInt())
        for (offset in padded.indices step 64) {
            val words = IntArray(68)
            val block = ByteBuffer.wrap(padded, offset, 64)
            for (index in 0..15) words[index] = block.int
            for (index in 16..67) {
                val mixed = words[index - 16] xor words[index - 9] xor Integer.rotateLeft(words[index - 3], 15)
                words[index] = mixed xor Integer.rotateLeft(mixed,15) xor Integer.rotateLeft(mixed,23) xor Integer.rotateLeft(words[index-13],7) xor words[index-6]
            }
            val work = state.copyOf()
            for (index in 0..63) {
                val first = Integer.rotateLeft(Integer.rotateLeft(work[0],12) + work[4] + Integer.rotateLeft(if(index<16) 0x79cc4519 else 0x7a879d8a,index),7)
                val second = first xor Integer.rotateLeft(work[0],12)
                val left = if(index<16) work[0] xor work[1] xor work[2] else (work[0] and work[1]) or (work[0] and work[2]) or (work[1] and work[2])
                val right = if(index<16) work[4] xor work[5] xor work[6] else (work[4] and work[5]) or (work[4].inv() and work[6])
                val tempLeft = left + work[3] + second + (words[index] xor words[index+4])
                val tempRight = right + work[7] + first + words[index]
                work[3]=work[2]; work[2]=Integer.rotateLeft(work[1],9); work[1]=work[0]; work[0]=tempLeft
                work[7]=work[6]; work[6]=Integer.rotateLeft(work[5],19); work[5]=work[4]
                work[4]=tempRight xor Integer.rotateLeft(tempRight,9) xor Integer.rotateLeft(tempRight,17)
            }
            for (index in state.indices) state[index] = state[index] xor work[index]
        }
        return ByteBuffer.allocate(32).apply { state.forEach { putInt(it) } }.array()
    }
    private fun rc4(data: ByteArray, key: ByteArray): ByteArray {
        val state = IntArray(256) { it }
        var cursor = 0
        for (index in state.indices) { cursor = (cursor + state[index] + (key[index % key.size].toInt() and 255)) and 255; val saved=state[index]; state[index]=state[cursor]; state[cursor]=saved }
        var first = 0; cursor = 0
        return ByteArray(data.size) { index ->
            first=(first+1) and 255; cursor=(cursor+state[first]) and 255
            val saved=state[first]; state[first]=state[cursor]; state[cursor]=saved
            (data[index].toInt() xor state[(state[first]+state[cursor]) and 255]).toByte()
        }
    }
    private fun encode(data: ByteArray, table: String): String = buildString {
        for (index in 0 until ((data.size * 4 + 2) / 3)) {
            val start = index / 4 * 3
            val packed = ((data.getOrElse(start){0}.toInt() and 255) shl 16) or ((data.getOrElse(start+1){0}.toInt() and 255) shl 8) or (data.getOrElse(start+2){0}.toInt() and 255)
            append(table[(packed ushr (18 - (index % 4) * 6)) and 63])
        }
    }
    fun generate(query: String, userAgent: String): String {
        val start = System.currentTimeMillis(); val end = start + 100
        val urlHash = sm3(sm3((query+"cus").toByteArray()))
        val suffixHash = sm3(sm3("cus".toByteArray()))
        val userHash = sm3(encode(rc4(userAgent.toByteArray(),byteArrayOf(0,1,14)),"ckdp1h4ZKsUB80/Mfvw36XIgR25+WQAlEi7NLboqYTOPuzmFjJnryx9HVGDaStCe").toByteArray())
        val environment = "1920|1080|1920|1040|0|30|0|0|1872|92|1920|1040|1857|92|1|24|Win32".toByteArray()
        val fields=IntArray(80)
        fun split(value: Long, offset: Int) { for(index in 0..3) fields[offset+index]=((value ushr (24-index*8)) and 255).toInt() }
        fields[18]=44; split(start,20); fields[24]=((start ushr 32) and 255).toInt(); fields[25]=((start ushr 40) and 255).toInt()
        fields[31]=1; fields[37]=14; fields[38]=urlHash[21].toInt() and 255; fields[39]=urlHash[22].toInt() and 255
        fields[40]=suffixHash[21].toInt() and 255; fields[41]=suffixHash[22].toInt() and 255
        fields[42]=userHash[23].toInt() and 255; fields[43]=userHash[24].toInt() and 255
        split(end,44); fields[48]=3; fields[49]=((end ushr 32) and 255).toInt(); fields[50]=((end ushr 40) and 255).toInt()
        split(110624,52); fields[57]=6383 and 255; fields[58]=(6383 ushr 8) and 255; fields[65]=environment.size and 255; fields[66]=environment.size ushr 8
        val order=intArrayOf(18,20,52,26,30,34,58,38,40,53,42,21,27,54,55,31,35,57,39,41,43,22,28,32,60,36,23,29,33,37,44,45,59,46,47,48,49,50,24,25,65,66,70,71)
        val checksum=order.filter { it!=34 }.fold(0) { value,index -> value xor fields[index] }
        val body=order.map { fields[it].toByte() }.toByteArray()+environment+byteArrayOf(checksum.toByte())
        fun random(number: Int, first: Int, second: Int): ByteArray = byteArrayOf(((number and 170) or (first and 85)).toByte(),((number and 85) or (first and 170)).toByte(),(((number ushr 8) and 170) or (second and 85)).toByte(),(((number ushr 8) and 85) or (second and 170)).toByte())
        val prefix=random(1234,3,45)+random(9876,1,0)+random(5555,1,5)
        return encode(prefix+rc4(body,byteArrayOf(121)),"Dkdpgh2ZmsQB80/MfvV36XI1R45-WUAlEixNLwoqYTOPuzKFjJnry79HbGcaStCe")+"="
    }
}
