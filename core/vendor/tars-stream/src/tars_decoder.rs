use bytes::{Buf, Bytes, IntoBuf};
use std::collections::BTreeMap;
use std::mem;

use errors::DecodeErr;
use tars_trait::{EnumFromI32, EnumToI32, StructFromTars};
use tars_type::TarsTypeMark;
use tars_type::TarsTypeMark::*;

#[derive(Debug)]
pub struct TarsDecoder {
    buf: Bytes,
    pos: usize,
}
#[derive(Debug)]
pub struct Head {
    tag: u8,
    tars_type: TarsTypeMark,
    len: u8,
}

impl TarsDecoder {
    pub fn new() -> TarsDecoder {
        TarsDecoder {
            buf: Bytes::new(),
            pos: 0,
        }
    }

    pub fn individual_decode<T>(buf: &Bytes) -> Result<T, DecodeErr>
    where
        T: DecodeTars,
    {
        let mut decoder = TarsDecoder::from(buf);
        T::_decode(&mut decoder, 0)
    }

    #[inline]
    fn return_error_if_required_not_found<T>(
        e: DecodeErr,
        is_require: bool,
        default_value: T,
    ) -> Result<T, DecodeErr> {
        match e {
            // field 不存在，若为 require，返回异常，否则为 optional, 返回默认值
            DecodeErr::TarsTagNotFoundErr => if is_require {
                Err(e)
            } else {
                Ok(default_value)
            },
            _ => Err(e),
        }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    fn current_pos(&self) -> usize {
        self.pos
    }

    fn set_pos(&mut self, pos: usize) -> Result<(), DecodeErr> {
        if pos > self.buf.len() {
            Err(DecodeErr::NoEnoughDataErr)
        } else {
            self.pos = pos;
            Ok(())
        }
    }

    fn advance(&mut self, cnt: usize) -> Result<(), DecodeErr> {
        if self.remaining() < cnt {
            Err(DecodeErr::NoEnoughDataErr)
        } else {
            self.pos += cnt;
            Ok(())
        }
    }

    fn take_then_advance(&mut self, size: usize) -> Result<Bytes, DecodeErr> {
        if self.remaining() < size {
            Err(DecodeErr::NoEnoughDataErr)
        } else {
            let pos = self.current_pos();
            let b = self.buf.slice(pos, pos + size);
            self.pos += size;
            Ok(b)
        }
    }

    fn skip_to_tag(&mut self, tag: u8) -> Result<Head, DecodeErr> {
        let mut result: Option<Head> = None;
        // 记录当前位置
        let before_pos = self.current_pos();
        while self.has_remaining() {
            let head = self.take_head()?;
            if head.tag == tag && head.tars_type != EnStructEnd {
                result = Some(head);
                break;
            } else {
                self.skip_field(head.tars_type)?;
            }
        }
        match result {
            Some(h) => Ok(h),
            None => {
                // tag查找失败，恢复至tag查询前位置
                self.set_pos(before_pos)?;
                Err(DecodeErr::TarsTagNotFoundErr)
            }
        }
    }

    fn take_head(&mut self) -> Result<Head, DecodeErr> {
        if self.remaining() < 1 {
            Err(DecodeErr::NoEnoughDataErr)
        } else {
            let mut buf = self.take_then_advance(1)?.into_buf();
            let b = buf.get_u8();
            let tars_type = b & 0x0f;
            let mut tag = (b & 0xf0) >> 4;
            let len = if tag < 15 {
                1
            } else {
                let mut buf = self.take_then_advance(1)?.into_buf();
                tag = buf.get_u8();
                2
            };
            Ok(Head {
                tag,
                len,
                tars_type: TarsTypeMark::from(tars_type),
            })
        }
    }

    fn skip_field(&mut self, tars_type: TarsTypeMark) -> Result<(), DecodeErr> {
        match tars_type {
            EnInt8 => self.advance(1),
            EnInt16 => self.advance(2),
            EnInt32 => self.advance(4),
            EnInt64 => self.advance(8),
            EnFloat => self.advance(4),
            EnDouble => self.advance(8),
            EnString1 => self.skip_string1_field(),
            EnString4 => self.skip_string4_field(),
            EnMaps => self.skip_map_field(),
            EnList => self.skip_list_field(),
            EnStructBegin => self.skip_struct_field(),
            EnStructEnd => Ok(()),
            EnZero => Ok(()),
            EnSimplelist => self.skip_simple_list_field(),
        }
    }

    fn skip_string1_field(&mut self) -> Result<(), DecodeErr> {
        let mut buf = self.take_then_advance(1)?.into_buf();
        let size = buf.get_u8() as usize;
        self.advance(size)
    }

    fn skip_string4_field(&mut self) -> Result<(), DecodeErr> {
        let mut buf = self.take_then_advance(4)?.into_buf();
        let size = buf.get_u32_be() as usize;
        self.advance(size)
    }

    fn skip_map_field(&mut self) -> Result<(), DecodeErr> {
        let ele_size = self.read_int32(0, true, 0)? as usize;
        for _ in 0..ele_size * 2 {
            let head = self.take_head()?;
            self.skip_field(head.tars_type)?;
        }
        Ok(())
    }

    fn skip_list_field(&mut self) -> Result<(), DecodeErr> {
        let ele_size = self.read_int32(0, true, 0)? as usize;
        for _ in 0..ele_size {
            let head = self.take_head()?;
            self.skip_field(head.tars_type)?;
        }
        Ok(())
    }

    fn skip_simple_list_field(&mut self) -> Result<(), DecodeErr> {
        let _head = self.take_head()?; // consume header (list type)
        let size = self.read_int32(0, true, 0)? as usize;
        self.advance(size)
    }

    fn skip_struct_field(&mut self) -> Result<(), DecodeErr> {
        let mut head = self.take_head()?;
        loop {
            match head.tars_type {
                EnStructEnd => break,
                _ => {
                    self.skip_field(head.tars_type)?;
                    head = self.take_head()?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> From<&'a [u8]> for TarsDecoder {
    fn from(buf: &'a [u8]) -> Self {
        let b = Bytes::from(buf);
        TarsDecoder { buf: b, pos: 0 }
    }
}

impl<'a> From<&'a Bytes> for TarsDecoder {
    fn from(buf: &'a Bytes) -> Self {
        let b = buf.clone();
        TarsDecoder { buf: b, pos: 0 }
    }
}

impl From<Vec<u8>> for TarsDecoder {
    fn from(buf: Vec<u8>) -> Self {
        let b = Bytes::from(buf);
        TarsDecoder { buf: b, pos: 0 }
    }
}

pub trait TarsDecodeNormalTrait {
    fn read_int8(&mut self, tag: u8, is_require: bool, default_value: i8) -> Result<i8, DecodeErr>;
    fn read_boolean(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: bool,
    ) -> Result<bool, DecodeErr>;

    fn read_int16(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: i16,
    ) -> Result<i16, DecodeErr>;

    fn read_int32(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: i32,
    ) -> Result<i32, DecodeErr>;

    fn read_int64(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: i64,
    ) -> Result<i64, DecodeErr>;

    fn read_uint8(&mut self, tag: u8, is_require: bool, default_value: u8)
        -> Result<u8, DecodeErr>;

    fn read_uint16(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: u16,
    ) -> Result<u16, DecodeErr>;

    fn read_uint32(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: u32,
    ) -> Result<u32, DecodeErr>;

    fn read_float(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: f32,
    ) -> Result<f32, DecodeErr>;

    fn read_double(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: f64,
    ) -> Result<f64, DecodeErr>;

    fn read_string(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: String,
    ) -> Result<String, DecodeErr>;

    fn read_bytes(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: Bytes,
    ) -> Result<Bytes, DecodeErr>;

    fn read_map<K, V>(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: BTreeMap<K, V>,
    ) -> Result<BTreeMap<K, V>, DecodeErr>
    where
        K: DecodeTars + Ord,
        V: DecodeTars;

    fn read_enum<T>(&mut self, tag: u8, is_require: bool, default_value: T) -> Result<T, DecodeErr>
    where
        T: EnumFromI32 + EnumToI32;

    fn read_struct<T>(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: T,
    ) -> Result<T, DecodeErr>
    where
        T: StructFromTars;
}

pub trait TarsDecodeListTrait<T>
where
    T: DecodeTars,
{
    fn read_list(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: Vec<T>,
    ) -> Result<Vec<T>, DecodeErr>;
}

impl TarsDecodeNormalTrait for TarsDecoder {
    fn read_int8(&mut self, tag: u8, is_require: bool, default_value: i8) -> Result<i8, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                // tag 查找成功
                EnZero => Ok(0),
                EnInt8 => {
                    let mut buf = self.take_then_advance(1)?.into_buf();
                    Ok(buf.get_i8())
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_boolean(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: bool,
    ) -> Result<bool, DecodeErr> {
        self.read_int8(tag, is_require, default_value as i8)
            .map(|i| i != 0)
    }

    fn read_int16(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: i16,
    ) -> Result<i16, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnZero => Ok(0),
                EnInt8 => {
                    let mut buf = self.take_then_advance(1)?.into_buf();
                    Ok(i16::from(buf.get_i8()))
                }
                EnInt16 => {
                    let mut buf = self.take_then_advance(2)?.into_buf();
                    Ok(buf.get_i16_be())
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_int32(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: i32,
    ) -> Result<i32, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnZero => Ok(0),
                EnInt8 => {
                    let mut buf = self.take_then_advance(1)?.into_buf();
                    Ok(i32::from(buf.get_i8()))
                }
                EnInt16 => {
                    let mut buf = self.take_then_advance(2)?.into_buf();
                    Ok(i32::from(buf.get_i16_be()))
                }
                EnInt32 => {
                    let mut buf = self.take_then_advance(4)?.into_buf();
                    Ok(buf.get_i32_be())
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_int64(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: i64,
    ) -> Result<i64, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnZero => Ok(0),
                EnInt8 => {
                    let mut buf = self.take_then_advance(1)?.into_buf();
                    Ok(i64::from(buf.get_i8()))
                }
                EnInt16 => {
                    let mut buf = self.take_then_advance(2)?.into_buf();
                    Ok(i64::from(buf.get_i16_be()))
                }
                EnInt32 => {
                    let mut buf = self.take_then_advance(4)?.into_buf();
                    Ok(i64::from(buf.get_i32_be()))
                }
                EnInt64 => {
                    let mut buf = self.take_then_advance(8)?.into_buf();
                    Ok(buf.get_i64_be())
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_uint8(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: u8,
    ) -> Result<u8, DecodeErr> {
        self.read_int16(tag, is_require, default_value as i16)
            .map(|i| i as u8)
    }

    fn read_uint16(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: u16,
    ) -> Result<u16, DecodeErr> {
        self.read_int32(tag, is_require, default_value as i32)
            .map(|i| i as u16)
    }

    fn read_uint32(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: u32,
    ) -> Result<u32, DecodeErr> {
        self.read_int64(tag, is_require, default_value as i64)
            .map(|i| i as u32)
    }

    fn read_float(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: f32,
    ) -> Result<f32, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnZero => Ok(0.0),
                EnFloat => {
                    let mut buf = self.take_then_advance(4)?.into_buf();
                    Ok(buf.get_f32_be())
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_double(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: f64,
    ) -> Result<f64, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnZero => Ok(0.0),
                EnDouble => {
                    let mut buf = self.take_then_advance(8)?.into_buf();
                    Ok(buf.get_f64_be())
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_string(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: String,
    ) -> Result<String, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnString1 => {
                    let mut size_buf = self.take_then_advance(1)?.into_buf();
                    let size = size_buf.get_u8() as usize;
                    let field_buf = self.take_then_advance(size)?.into_buf();
                    let cow = String::from_utf8_lossy(field_buf.bytes());
                    Ok(String::from(cow))
                }
                EnString4 => {
                    let mut size_buf = self.take_then_advance(4)?.into_buf();
                    let size = size_buf.get_u32_be() as usize;
                    let field_buf = self.take_then_advance(size)?.into_buf();
                    let cow = String::from_utf8_lossy(field_buf.bytes());
                    Ok(String::from(cow))
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_bytes(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: Bytes,
    ) -> Result<Bytes, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnSimplelist => {
                    let head = self.take_head()?;
                    match head.tars_type {
                        EnInt8 | EnInt16 | EnInt32 => {
                            let size = self.read_int32(0, true, 0)? as usize;
                            self.take_then_advance(size)
                        }
                        _ => Err(DecodeErr::WrongSimpleListTarsTypeErr),
                    }
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_map<K, V>(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: BTreeMap<K, V>,
    ) -> Result<BTreeMap<K, V>, DecodeErr>
    where
        K: DecodeTars + Ord,
        V: DecodeTars,
    {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnMaps => {
                    let size = self.read_int32(0, true, 0)? as usize;
                    let mut m = BTreeMap::new();
                    for _ in 0..size {
                        let key = K::_decode(self, 0)?;
                        let value = V::_decode(self, 1)?;
                        m.insert(key, value);
                    }
                    Ok(m)
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }

    fn read_enum<T>(&mut self, tag: u8, is_require: bool, default_value: T) -> Result<T, DecodeErr>
    where
        T: EnumFromI32 + EnumToI32,
    {
        let i = self.read_int32(tag, is_require, default_value._to_i32())?;
        T::_from_i32(i)
    }

    fn read_struct<T>(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: T,
    ) -> Result<T, DecodeErr>
    where
        T: StructFromTars,
    {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnStructBegin => T::_decode_from(self),
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }
}

impl<T> TarsDecodeListTrait<T> for TarsDecoder
where
    T: DecodeTars,
{
    fn read_list(
        &mut self,
        tag: u8,
        is_require: bool,
        default_value: Vec<T>,
    ) -> Result<Vec<T>, DecodeErr> {
        match self.skip_to_tag(tag) {
            Ok(head) => match head.tars_type {
                EnList => {
                    let size = self.read_int32(0, true, 0)? as usize;
                    let mut v = vec![];
                    for _ in 0..size {
                        let ele = T::_decode(self, 0)?;
                        v.push(ele);
                    }
                    Ok(v)
                }
                _ => Err(DecodeErr::MisMatchTarsTypeErr),
            },
            Err(e) => TarsDecoder::return_error_if_required_not_found(e, is_require, default_value),
        }
    }
}

pub trait DecodeTars {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr>
    where
        Self: Sized;
}

impl DecodeTars for i8 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_int8(tag, true, i8::default())
    }
}

impl DecodeTars for bool {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_boolean(tag, true, bool::default())
    }
}

impl DecodeTars for i16 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_int16(tag, true, i16::default())
    }
}

impl DecodeTars for i32 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_int32(tag, true, i32::default())
    }
}

impl DecodeTars for i64 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_int64(tag, true, i64::default())
    }
}

impl DecodeTars for u8 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_uint8(tag, true, u8::default())
    }
}

impl DecodeTars for u16 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_uint16(tag, true, u16::default())
    }
}

impl DecodeTars for u32 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_uint32(tag, true, u32::default())
    }
}

impl DecodeTars for f32 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_float(tag, true, f32::default())
    }
}

impl DecodeTars for f64 {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_double(tag, true, f64::default())
    }
}

impl DecodeTars for String {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_string(tag, true, String::default())
    }
}

impl<K, V> DecodeTars for BTreeMap<K, V>
where
    K: DecodeTars + Ord,
    V: DecodeTars,
{
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_map(tag, true, BTreeMap::<K, V>::new())
    }
}

impl<T> DecodeTars for Vec<T>
where
    T: DecodeTars,
{
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_list(tag, true, vec![])
    }
}

impl DecodeTars for Bytes {
    fn _decode(decoder: &mut TarsDecoder, tag: u8) -> Result<Self, DecodeErr> {
        decoder.read_bytes(tag, true, Bytes::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use errors::DecodeErr;
    use std::collections::BTreeMap;
    use std::mem;

    #[test]
    fn test_decode_simple_list() {
        let head: [u8; 4] = unsafe { mem::transmute(4u32.to_be()) };
        let b: [u8; 11] = [
            0x7d, 0x00, 0x02, head[0], head[1], head[2], head[3], 4, 5, 6, 7,
        ];
        let mut de = TarsDecoder::from(&b[..]);
        let list: Vec<i8> = de.read_list(7, true, vec![]).unwrap();
        let result: Vec<i8> = vec![4, 5, 6, 7];
        assert_eq!(list, result);

        let b2: [u8; 11] = [
            0xed, 0x00, 0x02, head[0], head[1], head[2], head[3], 1, 0, 1, 0,
        ];
        let mut de2 = TarsDecoder::from(&b2[..]);
        let list: Vec<bool> = de2.read_list(14, true, vec![]).unwrap();
        let result: Vec<bool> = vec![true, false, true, false];
        assert_eq!(list, result);

        let olist: Vec<bool> = de2.read_list(16, false, vec![]).unwrap();
        assert_eq!(olist, vec![]);

        let olist2: Vec<i8> = de2.read_list(244, false, vec![1, 2]).unwrap();
        assert_eq!(olist2, vec![1, 2]);

        let err: Result<Vec<bool>, DecodeErr> = de.read_list(129, true, vec![true]);
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_zero() {
        let mut de = TarsDecoder::from(&b"\x0c\x1c\x2c\x3c\x4c\x5c\xfc\xff\x9c\xac\xec"[..]);
        let o0 = de.read_int8(128, false, 0).unwrap();
        let o1 = de.read_int8(128, false, 13).unwrap();

        let v0: u8 = de.read_uint8(0, true, 0).unwrap();
        let v1: u16 = de.read_uint16(1, true, 0).unwrap();
        let v2: u32 = de.read_uint32(2, true, 0).unwrap();
        let v3: i8 = de.read_int8(3, true, 0).unwrap();
        let v4: i16 = de.read_int16(4, true, 0).unwrap();
        let v5: i32 = de.read_int32(5, true, 0).unwrap();
        let v6: i64 = de.read_int64(255, true, 0).unwrap();
        let v7: f32 = de.read_float(9, true, 0.0).unwrap();
        let v8: f64 = de.read_double(10, true, 0.0).unwrap();
        let v9: bool = de.read_boolean(14, true, false).unwrap();

        assert_eq!(o0, 0);
        assert_eq!(o1, 13);

        assert_eq!(v0, 0);
        assert_eq!(v1, 0);
        assert_eq!(v2, 0);
        assert_eq!(v3, 0);
        assert_eq!(v4, 0);
        assert_eq!(v5, 0);
        assert_eq!(v6, 0);
        assert_eq!(v7, 0.0);
        assert_eq!(v8, 0.0);
        assert_eq!(v9, false);

        let err: Result<u32, DecodeErr> = de.read_uint32(129, true, 0);
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_list() {
        let size: [u8; 4] = unsafe { mem::transmute(2u32.to_be()) };
        let b: [u8; 28] = [
            0xa9,
            0x02,
            size[0],
            size[1],
            size[2],
            size[3],
            // {tag: 0, type: 6}
            0x06,
            7,
            b'f',
            b'o',
            b'o',
            b' ',
            b'b',
            b'a',
            b'r',
            // {tag: 0, type: 6}
            0x06,
            11,
            b'h',
            b'e',
            b'l',
            b'l',
            b'o',
            b' ',
            b'w',
            b'o',
            b'r',
            b'l',
            b'd',
        ];
        let mut de = TarsDecoder::from(&b[..]);
        let list: Vec<String> = de.read_list(10, true, vec![]).unwrap();
        assert_eq!(list[0], String::from(&"foo bar"[..]));
        assert_eq!(list[1], String::from(&"hello world"[..]));

        assert_eq!(
            de.read_list(10, true, vec![]) as Result<Vec<String>, DecodeErr>,
            Err(DecodeErr::TarsTagNotFoundErr)
        );

        let b2: [u8; 6] = [0x99, 0x02, 0, 0, 0, 0];
        let mut de2 = TarsDecoder::from(&b2[..]);
        let v2: Vec<BTreeMap<String, i32>> = de2.read_list(9, true, vec![]).unwrap();
        assert_eq!(v2, vec![]);

        let v3: Vec<BTreeMap<String, i32>> = de2.read_list(128, false, vec![]).unwrap();
        assert_eq!(v3, vec![]);

        let err: Result<Vec<BTreeMap<String, i32>>, DecodeErr> = de2.read_list(129, true, vec![]);
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_map() {
        let size: [u8; 4] = unsafe { mem::transmute(1u32.to_be()) };
        let b: [u8; 28] = [
            0x48,
            0x02,
            size[0],
            size[1],
            size[2],
            size[3],
            // {tag: 0, type: 6}
            0x06,
            7,
            b'f',
            b'o',
            b'o',
            b' ',
            b'b',
            b'a',
            b'r',
            // {tag: 1, type: 6}
            0x16,
            11,
            b'h',
            b'e',
            b'l',
            b'l',
            b'o',
            b' ',
            b'w',
            b'o',
            b'r',
            b'l',
            b'd',
        ];
        let mut de = TarsDecoder::from(&b[..]);
        let map: BTreeMap<String, String> = de.read_map(4, true, BTreeMap::new()).unwrap();
        let value2 = map.get(&String::from(&"foo bar"[..])).unwrap();
        assert_eq!(value2, &String::from(&"hello world"[..]));

        let b2: [u8; 6] = [0x48, 0x02, 0, 0, 0, 0];
        let mut de2 = TarsDecoder::from(&b2[..]);
        let map2: BTreeMap<Vec<String>, BTreeMap<i32, String>> =
            de2.read_map(4, true, BTreeMap::new()).unwrap();
        assert_eq!(map2, BTreeMap::new());

        let omap2: BTreeMap<Vec<String>, BTreeMap<i32, String>> =
            de2.read_map(129, false, BTreeMap::new()).unwrap();
        assert_eq!(omap2, BTreeMap::new());

        let err: Result<BTreeMap<Vec<String>, BTreeMap<i32, String>>, DecodeErr> =
            de2.read_map(129, true, BTreeMap::new());
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_int64() {
        let b: [u8; 8] = unsafe { mem::transmute(0x0acb8b9d9d9d9d9di64.to_be()) };
        let mut header_vec: Vec<u8> = vec![0xf3, 0xff];
        header_vec.extend_from_slice(&b);
        let mut de2 = TarsDecoder::from(header_vec.as_slice());
        let i: i64 = de2.read_int64(255, true, 0).unwrap();
        assert_eq!(i, 0x0acb8b9d9d9d9d9d);

        let i2: i64 = de2.read_int64(244, false, i64::max_value()).unwrap();
        assert_eq!(i2, i64::max_value());

        let err: Result<i64, DecodeErr> = de2.read_int64(129, true, 0);
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_int32() {
        let b: [u8; 4] = unsafe { mem::transmute(0x0acb8b9di32.to_be()) };
        let mut header_vec: Vec<u8> = vec![0xf2, 0xff];
        header_vec.extend_from_slice(&b);
        let mut de2 = TarsDecoder::from(header_vec.as_slice());
        let i: i32 = de2.read_int32(255, true, 0).unwrap();
        assert_eq!(i, 0x0acb8b9di32);

        let i2: i32 = de2.read_int32(244, false, i32::max_value()).unwrap();
        assert_eq!(i2, i32::max_value());

        let err: Result<i32, DecodeErr> = de2.read_int32(129, true, 0);
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_int16() {
        let b: [u8; 2] = unsafe { mem::transmute(0x0acbi16.to_be()) };
        let mut header_vec: Vec<u8> = vec![0xe1];
        header_vec.extend_from_slice(&b);
        let mut de2 = TarsDecoder::from(header_vec.as_slice());
        let i: i16 = de2.read_int16(14, true, 0).unwrap();
        assert_eq!(i, 0x0acbi16);

        let i2: i16 = de2.read_int16(244, false, i16::max_value()).unwrap();
        assert_eq!(i2, i16::max_value());

        let err: Result<i16, DecodeErr> = de2.read_int16(129, true, 0);
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_int8() {
        let header_vec: Vec<u8> = vec![0xe0, 0xff];
        let mut de2 = TarsDecoder::from(header_vec.as_slice());
        let i: i8 = de2.read_int8(14, true, 0).unwrap();
        assert_eq!(i, -1);

        let mut de2 = TarsDecoder::from(header_vec.as_slice());
        let i = de2.read_int8(15, false, 0).unwrap();
        let i2 = de2.read_int8(14, false, 0).unwrap();
        assert_eq!(i, 0);
        assert_eq!(i2, -1);
    }

    // #[test]
    // fn test_decode_double() {
    //     let b2: [u8; 8] = unsafe { mem::transmute(0.633313f64.to_bits().to_be()) };
    //     let mut de2 = TarsDecoder::from(&b2[..]);
    //     let f: f64 = de2.read(TarsTypeMark::EnDouble.value()).unwrap();
    //     assert!(f == 0.633313f64);
    // }

    // #[test]
    // fn test_decode_float() {
    //     let b2: [u8; 4] = unsafe { mem::transmute(0.35524f32.to_bits().to_be()) };
    //     let mut de2 = TarsDecoder::from(&b2[..]);
    //     let f: f32 = de2.read(TarsTypeMark::EnFloat.value()).unwrap();
    //     assert!(f == 0.35524f32);
    // }

    #[test]
    fn test_decode_string() {
        // test read string1
        let d: [u8; 9] = [0x06, 0x07, b'f', b'o', b'o', b' ', b'b', b'a', b'r'];
        let mut de = TarsDecoder::from(&d[..]);
        assert_eq!(
            de.read_string(0, true, String::default()).unwrap(),
            String::from(&"foo bar"[..])
        );

        // test read string4
        let size: [u8; 4] = unsafe { mem::transmute(7u32.to_be()) };
        let d2: [u8; 12] = [
            0x27, size[0], size[1], size[2], size[3], b'f', b'o', b'o', b' ', b'b', b'a', b'r',
        ];
        let mut de2 = TarsDecoder::from(&d2[..]);
        assert_eq!(
            de2.read_string(2, true, String::default()).unwrap(),
            String::from(&"foo bar"[..])
        );

        let i2: String = de2.read_string(244, false, String::default()).unwrap();
        assert_eq!(i2, String::default());

        let err: Result<String, DecodeErr> = de2.read_string(129, true, String::default());
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    #[test]
    fn test_decode_bool() {
        let d: [u8; 3] = [0x0c, 0x10, 0x01];
        let mut de = TarsDecoder::from(&d[..]);
        let b: bool = de.read_boolean(0, true, false).unwrap();
        let ob: bool = de.read_boolean(2, false, false).unwrap();

        let b2: bool = de.read_boolean(1, true, false).unwrap();

        let ob2: bool = de.read_boolean(3, false, true).unwrap();

        assert_eq!(b, false);
        assert_eq!(b2, true);
        assert_eq!(ob, false);
        assert_eq!(ob2, true);
    }

    #[test]
    fn test_decode_bytes() {
        let d: [u8; 19] = *b"\x9d\x00\x02\x00\x00\x00\x0chello world!";
        let mut de = TarsDecoder::from(&d[..]);
        let b: Bytes = de.read_bytes(9, true, Bytes::default()).unwrap();
        assert_eq!(b, Bytes::from(&b"hello world!"[..]));

        let i2: Bytes = de.read_bytes(244, false, Bytes::default()).unwrap();
        assert_eq!(i2, Bytes::default());

        let err: Result<Bytes, DecodeErr> = de.read_bytes(129, true, Bytes::default());
        assert_eq!(err, Err(DecodeErr::TarsTagNotFoundErr));
    }

    // #[test]
    // fn test_decode_struct() {
    //     struct Test {
    //         a: u32
    //         b: Vec<i8>
    //     }
    // }
}
