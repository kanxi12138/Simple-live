use bytes::{BufMut, Bytes, BytesMut};
use errors::EncodeErr;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::mem;
use tars_trait::{EnumToI32, StructToTars};
use tars_type::TarsTypeMark::*;
use tars_type::*;

const MAX_HEADER_LEN: usize = 2;
const MAX_SIZE_LEN: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TarsEncoder {
    buf: BytesMut,
}

impl TarsEncoder {
    pub fn new() -> Self {
        TarsEncoder {
            buf: BytesMut::new(),
        }
    }

    pub fn individual_encode<T>(ele: &T) -> Result<Bytes, EncodeErr>
    where
        T: EncodeTars,
    {
        let mut encoder = TarsEncoder::new();
        ele._encode(&mut encoder, 0)?;
        Ok(encoder.to_bytes())
    }

    // move out buf
    pub fn to_bytes(self) -> Bytes {
        self.buf.freeze()
    }

    pub fn to_bytes_mut(self) -> BytesMut {
        self.buf
    }

    pub fn check_maybe_resize(&mut self, len: usize) {
        if self.buf.remaining_mut() < len {
            let new_len = self.buf.remaining_mut() + len + 1024;
            self.buf.reserve(new_len)
        }
    }

    fn put_head(&mut self, tag: u8, tars_type: TarsTypeMark) -> Result<(), EncodeErr> {
        self.check_maybe_resize(MAX_HEADER_LEN);
        if tag > u8::max_value() {
            Err(EncodeErr::TooBigTagErr)
        } else {
            if tag < 15 {
                let head = (tag << 4) | tars_type.value();
                self.buf.put_u8(head);
            } else {
                let head: u16 = u16::from((0xF0u8) | tars_type.value()) << 8 | u16::from(tag);
                self.buf.put_u16_be(head)
            }
            Ok(())
        }
    }
}

// all write_xxxx method will move value into TarsDecoder

pub trait TarsEncoderNormalTrait {
    fn write_int8(&mut self, tag: u8, ele: i8) -> Result<(), EncodeErr>;
    fn write_boolean(&mut self, tag: u8, ele: bool) -> Result<(), EncodeErr>;

    fn write_int16(&mut self, tag: u8, ele: i16) -> Result<(), EncodeErr>;
    fn write_int32(&mut self, tag: u8, ele: i32) -> Result<(), EncodeErr>;
    fn write_int64(&mut self, tag: u8, ele: i64) -> Result<(), EncodeErr>;

    fn write_uint8(&mut self, tag: u8, ele: u8) -> Result<(), EncodeErr>;
    fn write_uint16(&mut self, tag: u8, ele: u16) -> Result<(), EncodeErr>;
    fn write_uint32(&mut self, tag: u8, ele: u32) -> Result<(), EncodeErr>;

    fn write_float(&mut self, tag: u8, ele: f32) -> Result<(), EncodeErr>;
    fn write_double(&mut self, tag: u8, ele: f64) -> Result<(), EncodeErr>;

    fn write_string(&mut self, tag: u8, ele: &String) -> Result<(), EncodeErr>;

    fn write_bytes(&mut self, tag: u8, ele: &Bytes) -> Result<(), EncodeErr>;

    fn write_map<K, V>(&mut self, tag: u8, ele: &BTreeMap<K, V>) -> Result<(), EncodeErr>
    where
        K: EncodeTars + Ord,
        V: EncodeTars;

    fn write_enum<T>(&mut self, tag: u8, ele: &T) -> Result<(), EncodeErr>
    where
        T: EnumToI32;

    fn write_struct<T>(&mut self, tag: u8, ele: &T) -> Result<(), EncodeErr>
    where
        T: StructToTars;
}

pub trait TarsEncodeListTrait<T>
where
    T: EncodeTars,
{
    fn write_list(&mut self, tag: u8, ele: &Vec<T>) -> Result<(), EncodeErr>;
}

impl TarsEncoderNormalTrait for TarsEncoder {
    fn write_int8(&mut self, tag: u8, ele: i8) -> Result<(), EncodeErr> {
        if ele == 0 {
            self.put_head(tag, EnZero)
        } else {
            self.put_head(tag, EnInt8)?;
            self.check_maybe_resize(mem::size_of::<i8>());
            self.buf.put_i8(ele);
            Ok(())
        }
    }

    fn write_boolean(&mut self, tag: u8, ele: bool) -> Result<(), EncodeErr> {
        self.write_int8(tag, ele as i8)
    }

    fn write_int16(&mut self, tag: u8, ele: i16) -> Result<(), EncodeErr> {
        if ele >= i16::from(i8::min_value()) && ele <= i16::from(i8::max_value()) {
            self.write_int8(tag, ele as i8)
        } else {
            self.put_head(tag, EnInt16)?;
            self.check_maybe_resize(mem::size_of::<i16>());
            self.buf.put_i16_be(ele);
            Ok(())
        }
    }

    fn write_int32(&mut self, tag: u8, ele: i32) -> Result<(), EncodeErr> {
        if ele >= i32::from(i16::min_value()) && ele <= i32::from(i16::max_value()) {
            self.write_int16(tag, ele as i16)
        } else {
            self.put_head(tag, EnInt32)?;
            self.check_maybe_resize(mem::size_of::<i32>());
            self.buf.put_i32_be(ele);
            Ok(())
        }
    }

    fn write_int64(&mut self, tag: u8, ele: i64) -> Result<(), EncodeErr> {
        if ele >= i64::from(i32::min_value()) && ele <= i64::from(i32::max_value()) {
            self.write_int32(tag, ele as i32)
        } else {
            self.put_head(tag, EnInt64)?;
            self.check_maybe_resize(mem::size_of::<i64>());
            self.buf.put_i64_be(ele);
            Ok(())
        }
    }

    fn write_uint8(&mut self, tag: u8, ele: u8) -> Result<(), EncodeErr> {
        self.write_int16(tag, ele as i16)
    }

    fn write_uint16(&mut self, tag: u8, ele: u16) -> Result<(), EncodeErr> {
        self.write_int32(tag, ele as i32)
    }

    fn write_uint32(&mut self, tag: u8, ele: u32) -> Result<(), EncodeErr> {
        self.write_int64(tag, ele as i64)
    }

    fn write_float(&mut self, tag: u8, ele: f32) -> Result<(), EncodeErr> {
        if ele == 0.0 {
            self.put_head(tag, EnZero)?;
        } else {
            self.put_head(tag, EnFloat)?;
            self.check_maybe_resize(mem::size_of::<f32>());
            self.buf.put_f32_be(ele)
        }
        Ok(())
    }
    fn write_double(&mut self, tag: u8, ele: f64) -> Result<(), EncodeErr> {
        if ele == 0.0 {
            self.put_head(tag, EnZero)?;
        } else {
            self.put_head(tag, EnDouble)?;
            self.check_maybe_resize(mem::size_of::<f64>());
            self.buf.put_f64_be(ele)
        }
        Ok(())
    }
    fn write_string(&mut self, tag: u8, ele: &String) -> Result<(), EncodeErr> {
        let len = ele.len();
        self.check_maybe_resize(MAX_SIZE_LEN + len);

        if len <= usize::from(u8::max_value()) {
            // encode as string1
            self.put_head(tag, EnString1)?;
            match u8::try_from(len) {
                Ok(l) => {
                    self.buf.put_u8(l);
                    self.buf.put(ele);
                    Ok(())
                }
                Err(_) => Err(EncodeErr::ConvertU8Err),
            }
        } else if len <= u32::max_value() as usize {
            // encode as string4
            self.put_head(tag, EnString4)?;
            self.buf.put_u32_be(len as u32);
            self.buf.put(ele);
            Ok(())
        } else {
            Err(EncodeErr::DataTooBigErr)
        }
    }

    fn write_bytes(&mut self, tag: u8, ele: &Bytes) -> Result<(), EncodeErr> {
        let len = ele.len();
        if len > i32::max_value() as usize {
            Err(EncodeErr::DataTooBigErr)
        } else {
            self.put_head(tag, EnSimplelist)?;
            self.put_head(0, EnInt8)?;
            self.write_int32(0, len as i32)?;
            self.buf.extend_from_slice(ele);
            Ok(())
        }
    }

    fn write_map<K, V>(&mut self, tag: u8, ele: &BTreeMap<K, V>) -> Result<(), EncodeErr>
    where
        K: EncodeTars + Ord,
        V: EncodeTars,
    {
        let len = ele.len();
        if len > i32::max_value() as usize {
            Err(EncodeErr::DataTooBigErr)
        } else {
            self.put_head(tag, EnMaps)?;
            self.write_int32(0, len as i32)?;
            for (key, value) in ele.iter() {
                key._encode(self, 0)?;
                value._encode(self, 1)?;
            }
            Ok(())
        }
    }

    fn write_enum<T>(&mut self, tag: u8, ele: &T) -> Result<(), EncodeErr>
    where
        T: EnumToI32,
    {
        self.write_int32(tag, ele._to_i32())
    }

    fn write_struct<T>(&mut self, tag: u8, ele: &T) -> Result<(), EncodeErr>
    where
        T: StructToTars,
    {
        self.put_head(tag, EnStructBegin)?;
        ele._encode_to(self)?;
        self.put_head(0, EnStructEnd)
    }
}

impl<T> TarsEncodeListTrait<T> for TarsEncoder
where
    T: EncodeTars,
{
    fn write_list(&mut self, tag: u8, ele: &Vec<T>) -> Result<(), EncodeErr> {
        let len = ele.len();
        if len > i32::max_value() as usize {
            Err(EncodeErr::DataTooBigErr)
        } else {
            self.put_head(tag, EnList)?;
            self.write_int32(0, len as i32)?;
            for ele in ele.into_iter() {
                ele._encode(self, 0)?;
            }
            Ok(())
        }
    }
}

// EncodeTars Trait, 各类型将自身写入 TarsEncoder 中
pub trait EncodeTars {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr>;
}

impl EncodeTars for i8 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_int8(tag, *self)
    }
}

impl EncodeTars for i16 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_int16(tag, *self)
    }
}

impl EncodeTars for i32 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_int32(tag, *self)
    }
}

impl EncodeTars for i64 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_int64(tag, *self)
    }
}

impl EncodeTars for u8 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_uint8(tag, *self)
    }
}

impl EncodeTars for u16 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_uint16(tag, *self)
    }
}

impl EncodeTars for u32 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_uint32(tag, *self)
    }
}

impl EncodeTars for f32 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_float(tag, *self)
    }
}

impl EncodeTars for f64 {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_double(tag, *self)
    }
}

impl EncodeTars for bool {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_boolean(tag, *self)
    }
}

impl EncodeTars for String {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_string(tag, self)
    }
}

impl<K, V> EncodeTars for BTreeMap<K, V>
where
    K: EncodeTars + Ord,
    V: EncodeTars,
{
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_map(tag, self)
    }
}

impl<T> EncodeTars for Vec<T>
where
    T: EncodeTars,
{
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_list(tag, self)
    }
}

impl EncodeTars for Bytes {
    fn _encode(&self, encoder: &mut TarsEncoder, tag: u8) -> Result<(), EncodeErr> {
        encoder.write_bytes(tag, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_i8() {
        let mut encoder = TarsEncoder::new();
        let i0: i8 = -127;
        let i3: i8 = 0;
        let i1: i8 = 127;
        let i2: i8 = -1;

        encoder.write_int8(0, i0).unwrap();
        encoder.write_int8(14, i1).unwrap();
        encoder.write_int8(255, i2).unwrap();
        encoder.write_int8(3, i3).unwrap();

        assert_eq!(
            &encoder.to_bytes(),
            &b"\x00\x81\xe0\x7f\xf0\xff\xff\x3c"[..]
        );
    }

    #[test]
    fn test_encode_i16() {
        let mut encoder = TarsEncoder::new();
        let i0: i16 = -32768;
        let i1: i16 = -127;
        let i2: i16 = 32767;

        encoder.write_int16(0, i0).unwrap();
        encoder.write_int16(15, i1).unwrap();
        encoder.write_int16(19, i2).unwrap();

        assert_eq!(
            &encoder.to_bytes(),
            &b"\x01\x80\x00\xf0\x0f\x81\xf1\x13\x7f\xff"[..]
        );
    }

    #[test]
    fn test_encode_i32() {
        let mut encoder = TarsEncoder::new();

        let i0: i32 = 90909;
        let i1: i32 = 255;
        let i2: i32 = -127;
        let i3: i32 = -95234;

        encoder.write_int32(0, i0).unwrap();
        encoder.write_int32(15, i1).unwrap();
        encoder.write_int32(14, i2).unwrap();
        encoder.write_int32(14, i3).unwrap();

        assert_eq!(
            &encoder.to_bytes(),
            &b"\x02\x00\x01\x63\x1d\xf1\x0f\x00\xff\xe0\x81\xe2\xff\xfe\x8b\xfe"[..]
        );
    }

    #[test]
    fn test_encode_i64() {
        let mut encoder = TarsEncoder::new();

        let i0: i64 = -1;
        let i1: i64 = -129;
        let i2: i64 = -32769;
        let i3: i64 = -2147483649;

        encoder.write_int64(0, i0).unwrap();
        encoder.write_int64(0, i1).unwrap();
        encoder.write_int64(0, i2).unwrap();
        encoder.write_int64(0, i3).unwrap();

        assert_eq!(
            &encoder.to_bytes(),
            &b"\x00\xff\x01\xff\x7f\x02\xff\xff\x7f\xff\x03\xff\xff\xff\xff\x7f\xff\xff\xff"[..]
        );
    }

    #[test]
    fn test_encode_u8() {
        let mut encoder = TarsEncoder::new();
        let u0: u8 = 127;
        let u1: u8 = 255;
        let u2: u8 = 0;

        encoder.write_uint8(0, u0).unwrap();
        encoder.write_uint8(14, u1).unwrap();
        encoder.write_uint8(255, u2).unwrap();

        assert_eq!(&encoder.to_bytes(), &b"\x00\x7f\xe1\x00\xff\xfc\xff"[..]);
    }

    #[test]
    fn test_encode_u16() {
        let mut encoder = TarsEncoder::new();

        let i0: u16 = 32768;
        let i1: u16 = 255;
        let i2: u16 = 65535;

        encoder.write_uint16(0, i0).unwrap();
        encoder.write_uint16(15, i1).unwrap();

        encoder.write_uint16(19, i2).unwrap();
        assert_eq!(
            &encoder.to_bytes(),
            &b"\x02\x00\x00\x80\x00\xf1\x0f\x00\xff\xf2\x13\x00\x00\xff\xff"[..]
        );
    }

    #[test]
    fn test_encode_u32() {
        let mut encoder = TarsEncoder::new();
        let u0: u32 = 88888;
        let u1: u32 = 254;
        let u2: u32 = 256;

        encoder.write_uint32(0, u0).unwrap();
        encoder.write_uint32(14, u1).unwrap();
        encoder.write_uint32(14, u2).unwrap();

        assert_eq!(
            &encoder.to_bytes(),
            &b"\x02\x00\x01\x5b\x38\xe1\x00\xfe\xe1\x01\x00"[..]
        );
    }

    #[test]
    fn test_encode_f32() {
        let mut encoder = TarsEncoder::new();
        let f1: f32 = 0.1472;
        encoder.write_float(0, f1).unwrap();
        assert_eq!(&encoder.to_bytes(), &b"\x04\x3e\x16\xbb\x99"[..]);
    }

    #[test]
    fn test_encode_f64() {
        let mut encoder = TarsEncoder::new();
        let f1: f64 = 0.14723333;
        encoder.write_double(0, f1).unwrap();
        assert_eq!(
            &encoder.to_bytes(),
            &b"\x05\x3f\xc2\xd8\x8a\xb0\x9d\x97\x2a"[..]
        );
    }

    #[test]
    fn test_encode_bool() {
        let mut encoder = TarsEncoder::new();
        encoder.write_boolean(0, false).unwrap();
        encoder.write_boolean(1, true).unwrap();
        assert_eq!(&encoder.to_bytes(), &b"\x0c\x10\x01"[..]);
    }

    #[test]
    fn test_encode_string() {
        let mut encoder = TarsEncoder::new();
        let s: String = "hello wrold!".to_string();
        let expect_buf = "\x06\x0c".to_string() + &s;
        encoder.write_string(0, &s).unwrap();
        assert_eq!(&encoder.to_bytes(), &expect_buf);

        let mut encoder = TarsEncoder::new();
        let mut s1: String = String::new();
        for _ in 0..0xf7f7f {
            s1.push('z');
        }
        let expect_buf = "\x07\x00\x0f\x7f\x7f".to_string() + &s1;
        encoder.write_string(0, &s1).unwrap();
        assert_eq!(&encoder.to_bytes(), &expect_buf);
    }

    #[test]
    fn test_encode_vec() {
        let mut v2: Vec<i8> = Vec::with_capacity(0xf7f7f);
        for _ in 0..0xf7f7f {
            v2.push(-127);
        }
        let mut encoder = TarsEncoder::new();
        encoder.write_list(0, &v2).unwrap();
        let mut header_v: Vec<u8> = Vec::from(&b"\x0d\x00\x02\x00\x0f\x7f\x7f"[..]);
        header_v.extend_from_slice(unsafe { mem::transmute(v2.as_slice()) });
        assert_eq!(&encoder.to_bytes(), &header_v);

        let mut v3: Vec<bool> = Vec::with_capacity(0xf6f7f);
        let mut b = false;
        for _ in 0..0xf6f7f {
            v3.push(b);
            b = !b;
        }

        let mut encoder = TarsEncoder::new();
        encoder.write_list(0, &v3).unwrap();
        let mut header_v: Vec<u8> = Vec::from(&b"\x0d\x00\x02\x00\x0f\x6f\x7f"[..]);
        header_v.extend_from_slice(unsafe { mem::transmute(v3.as_slice()) });
        assert_eq!(&encoder.to_bytes(), &header_v);

        let mut v4: Vec<String> = Vec::with_capacity(0xf6f7e);
        let str4 = "hello".repeat(128);
        let str1 = "hello".to_string();
        let times = 0xf6f7e / 2;
        for _ in 0..times {
            v4.push(str4.clone());
        }
        for _ in 0..times {
            v4.push(str1.clone());
        }

        let mut encoder = TarsEncoder::new();
        encoder.write_list(10, &v4).unwrap();
        let buf = encoder.to_bytes();
        assert_eq!(&buf[0..2], &b"\xa9\x02"[..]);
        let len_in_u8: [u8; 4] = [buf[2], buf[3], buf[4], buf[5]];
        let len: i32 = i32::from_be(unsafe { mem::transmute(len_in_u8) });
        assert_eq!(len, v4.len() as i32);
    }

    #[test]
    fn test_encode_map() {
        let mut map: BTreeMap<String, i32> = BTreeMap::new();
        map.insert("hello".to_string(), 32);
        map.insert("world".to_string(), 42);

        let mut encoder = TarsEncoder::new();
        encoder.write_map(0, &map).unwrap();
        assert_eq!(
            &encoder.to_bytes(),
            &b"\x08\x00\x02\x06\x05hello\x10\x20\x06\x05world\x10\x2a"[..]
        );
    }

    #[test]
    fn test_encode_bytes() {
        let b = Bytes::from(&b"hello world!"[..]);
        let mut encoder = TarsEncoder::new();
        encoder.write_bytes(9, &b).unwrap();
        assert_eq!(&encoder.to_bytes(), &b"\x9d\x00\x00\x0chello world!"[..]);
    }
}
