use bytes::Bytes;
use errors::{DecodeErr, EncodeErr};
use std::collections::BTreeMap;

use tars_decoder::{DecodeTars, TarsDecoder};
use tars_encoder::{EncodeTars, TarsEncoder};

use tars_trait::ClassName;
use tars_type::ProtocolVersion;

type SimpleTupMap = BTreeMap<String, Bytes>;
type ComplexTupMap = BTreeMap<String, BTreeMap<String, Bytes>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TupUniAttribute {
    version: ProtocolVersion,
    simple_map: SimpleTupMap,
    complex_map: ComplexTupMap,
}
// for SimpleTup protocol version
impl TupUniAttribute {
    fn return_error_if_required_not_found<T>(
        is_require: bool,
        default_value: T,
    ) -> Result<T, DecodeErr> {
        if is_require {
            Err(DecodeErr::TupKeyNotFoundErr)
        } else {
            Ok(default_value)
        }
    }

    pub fn new(version: ProtocolVersion) -> Self {
        TupUniAttribute {
            version,
            simple_map: BTreeMap::new(),
            complex_map: BTreeMap::new(),
        }
    }

    pub fn from_bytes<'a>(buf: &'a Bytes, version: ProtocolVersion) -> Result<Self, DecodeErr> {
        match version {
            ProtocolVersion::TupSimple => Ok(TupUniAttribute {
                version,
                simple_map: TarsDecoder::individual_decode(buf)?,
                complex_map: BTreeMap::new(),
            }),
            ProtocolVersion::TupComplex => Ok(TupUniAttribute {
                version,
                simple_map: BTreeMap::new(),
                complex_map: TarsDecoder::individual_decode(buf)?,
            }),
            _ => Err(DecodeErr::UnsupportTupVersionErr),
        }
    }

    pub fn to_bytes(&self) -> Result<Bytes, EncodeErr> {
        match self.version {
            ProtocolVersion::TupSimple => TarsEncoder::individual_encode(&self.simple_map),
            ProtocolVersion::TupComplex => TarsEncoder::individual_encode(&self.complex_map),
            _ => Err(EncodeErr::UnsupportTupVersionErr),
        }
    }

    pub fn read<T>(&self, name: &String, is_require: bool, default_value: T) -> Result<T, DecodeErr>
    where
        T: DecodeTars + ClassName,
    {
        match self.version {
            ProtocolVersion::TupSimple => match self.simple_map.get(name) {
                Some(b) => Ok(TarsDecoder::individual_decode(b)?),
                None => Ok(Self::return_error_if_required_not_found(
                    is_require,
                    default_value,
                )?),
            },
            ProtocolVersion::TupComplex => match self.complex_map.get(name) {
                Some(item) => match item.get(&T::_class_name()) {
                    Some(b) => Ok(TarsDecoder::individual_decode(b)?),
                    None => Ok(Self::return_error_if_required_not_found(
                        is_require,
                        default_value,
                    )?),
                },
                None => Ok(Self::return_error_if_required_not_found(
                    is_require,
                    default_value,
                )?),
            },
            _ => Err(DecodeErr::UnsupportTupVersionErr),
        }
    }

    pub fn write<T>(&mut self, name: &String, value: &T) -> Result<(), EncodeErr>
    where
        T: EncodeTars + ClassName,
    {
        match self.version {
            ProtocolVersion::TupSimple => {
                self.simple_map
                    .insert(name.clone(), TarsEncoder::individual_encode(value)?);
                Ok(())
            }
            ProtocolVersion::TupComplex => {
                let mut item: BTreeMap<String, Bytes> = BTreeMap::new();
                item.insert(T::_class_name(), TarsEncoder::individual_encode(value)?);
                self.complex_map.insert(name.clone(), item);
                Ok(())
            }
            _ => Err(EncodeErr::UnsupportTupVersionErr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tars_encoder::*;

    #[test]
    fn test_decode_simple_tup() {
        let key0 = "zero".to_string();
        let value0 = 0;

        let key1 = "hello".to_string();
        let value1 = i8::max_value();

        let key2 = "world".to_string();
        let value2 = i16::max_value();

        let key3 = "aba".to_string();
        let value3 = i32::max_value();

        let key4 = "i64".to_string();
        let value4 = i64::max_value();

        let key5 = "bool".to_string();
        let value5 = true;

        let key6 = "u8".to_string();
        let value6 = u8::max_value();

        let key7 = "u16".to_string();
        let value7 = u16::max_value();

        let key8 = "u32".to_string();
        let value8 = u32::max_value();

        let key9 = "float".to_string();
        let value9 = 0.333f32;

        let key10 = "double".to_string();
        let value10 = 1.77721337f64;

        let key11 = "string".to_string();
        let value11 = String::from("hello wrold! foo bar!");

        let key12 = "bytes".to_string();
        let value12 = Bytes::from("hello wrold! foo bar!");

        let key13 = "vec".to_string();
        let value13: Vec<u32> = vec![1, 2, 3, 4];

        let key14 = "map".to_string();
        let value14: BTreeMap<String, String> = BTreeMap::new();

        let fake_key = "fake_key".to_string();

        let mut map = BTreeMap::new();

        map.insert(
            key0.clone(),
            TarsEncoder::individual_encode(&value0).unwrap(),
        );

        map.insert(
            key1.clone(),
            TarsEncoder::individual_encode(&value1).unwrap(),
        );

        map.insert(
            key2.clone(),
            TarsEncoder::individual_encode(&value2).unwrap(),
        );

        map.insert(
            key3.clone(),
            TarsEncoder::individual_encode(&value3).unwrap(),
        );

        map.insert(
            key4.clone(),
            TarsEncoder::individual_encode(&value4).unwrap(),
        );

        map.insert(
            key5.clone(),
            TarsEncoder::individual_encode(&value5).unwrap(),
        );

        map.insert(
            key6.clone(),
            TarsEncoder::individual_encode(&value6).unwrap(),
        );

        map.insert(
            key7.clone(),
            TarsEncoder::individual_encode(&value7).unwrap(),
        );

        map.insert(
            key8.clone(),
            TarsEncoder::individual_encode(&value8).unwrap(),
        );

        map.insert(
            key9.clone(),
            TarsEncoder::individual_encode(&value9).unwrap(),
        );

        map.insert(
            key10.clone(),
            TarsEncoder::individual_encode(&value10).unwrap(),
        );

        map.insert(
            key11.clone(),
            TarsEncoder::individual_encode(&value11).unwrap(),
        );

        map.insert(
            key12.clone(),
            TarsEncoder::individual_encode(&value12).unwrap(),
        );

        map.insert(
            key13.clone(),
            TarsEncoder::individual_encode(&value13).unwrap(),
        );

        map.insert(
            key14.clone(),
            TarsEncoder::individual_encode(&value14).unwrap(),
        );

        let uni = TupUniAttribute::from_bytes(
            &TarsEncoder::individual_encode(&map).unwrap(),
            ProtocolVersion::TupSimple,
        ).unwrap();

        let de_0 = uni.read(&key0, true, 0).unwrap();
        assert_eq!(de_0, value0);

        let de_i8: i8 = uni.read(&key1, true, 0).unwrap();
        assert_eq!(de_i8, value1);

        let de_i16 = uni.read(&key2, true, 0).unwrap();
        assert_eq!(de_i16, value2);

        let de_i32 = uni.read(&key3, true, 0).unwrap();
        assert_eq!(de_i32, value3);

        let de_i64 = uni.read(&key4, true, 0).unwrap();
        assert_eq!(de_i64, value4);

        let de_bool = uni.read(&key5, true, false).unwrap();
        assert_eq!(de_bool, value5);

        let de_u8 = uni.read(&key6, true, 0).unwrap();
        assert_eq!(de_u8, value6);

        let de_u16 = uni.read(&key7, true, 0).unwrap();
        assert_eq!(de_u16, value7);

        let de_u32 = uni.read(&key8, true, 0).unwrap();
        assert_eq!(de_u32, value8);

        let de_f32 = uni.read(&key9, true, 0.0).unwrap();
        assert_eq!(de_f32, value9);

        let de_f64 = uni.read(&key10, true, 0.0).unwrap();
        assert_eq!(de_f64, value10);

        let de_string = uni.read(&key11, true, String::from("")).unwrap();
        assert_eq!(de_string, value11);

        let de_bytes = uni.read(&key12, true, Bytes::default()).unwrap();
        assert_eq!(de_bytes, value12);

        let de_vec: Vec<u32> = uni.read(&key13, true, vec![]).unwrap();
        assert_eq!(de_vec, value13);

        let de_map: BTreeMap<String, String> = uni.read(&key14, true, BTreeMap::new()).unwrap();
        assert_eq!(de_map, value14);

        let de_fake_value_err = uni.read(&fake_key, true, 0);
        assert_eq!(de_fake_value_err, Err(DecodeErr::TupKeyNotFoundErr));

        let de_fake_value = uni.read(&fake_key, false, 0).unwrap();
        assert_eq!(de_fake_value, 0);
    }

    #[test]
    fn test_decode_complex_tup() {
        let key0 = "zero".to_string();
        let value0: i64 = 0;
        let mut item0: BTreeMap<String, Bytes> = BTreeMap::new();
        item0.insert(
            i64::_class_name(),
            TarsEncoder::individual_encode(&value0).unwrap(),
        );

        let key1 = "hello".to_string();
        let value1 = i8::max_value();
        let mut item1: BTreeMap<String, Bytes> = BTreeMap::new();
        item1.insert(
            i8::_class_name(),
            TarsEncoder::individual_encode(&value1).unwrap(),
        );

        let key2 = "world".to_string();
        let value2 = i16::max_value();
        let mut item2: BTreeMap<String, Bytes> = BTreeMap::new();
        item2.insert(
            i16::_class_name(),
            TarsEncoder::individual_encode(&value2).unwrap(),
        );

        let key3 = "aba".to_string();
        let value3 = i32::max_value();
        let mut item3: BTreeMap<String, Bytes> = BTreeMap::new();
        item3.insert(
            i32::_class_name(),
            TarsEncoder::individual_encode(&value3).unwrap(),
        );

        let key4 = "i64".to_string();
        let value4 = i64::max_value();
        let mut item4: BTreeMap<String, Bytes> = BTreeMap::new();
        item4.insert(
            i64::_class_name(),
            TarsEncoder::individual_encode(&value4).unwrap(),
        );

        let key5 = "bool".to_string();
        let value5 = true;
        let mut item5: BTreeMap<String, Bytes> = BTreeMap::new();
        item5.insert(
            bool::_class_name(),
            TarsEncoder::individual_encode(&value5).unwrap(),
        );

        let key6 = "u8".to_string();
        let value6 = u8::max_value();
        let mut item6: BTreeMap<String, Bytes> = BTreeMap::new();
        item6.insert(
            u8::_class_name(),
            TarsEncoder::individual_encode(&value6).unwrap(),
        );

        let key7 = "u16".to_string();
        let value7 = u16::max_value();
        let mut item7: BTreeMap<String, Bytes> = BTreeMap::new();
        item7.insert(
            u16::_class_name(),
            TarsEncoder::individual_encode(&value7).unwrap(),
        );

        let key8 = "u32".to_string();
        let value8 = u32::max_value();
        let mut item8: BTreeMap<String, Bytes> = BTreeMap::new();
        item8.insert(
            u32::_class_name(),
            TarsEncoder::individual_encode(&value8).unwrap(),
        );

        let key9 = "float".to_string();
        let value9 = 0.333f32;
        let mut item9: BTreeMap<String, Bytes> = BTreeMap::new();
        item9.insert(
            f32::_class_name(),
            TarsEncoder::individual_encode(&value9).unwrap(),
        );

        let key10 = "double".to_string();
        let value10 = 1.77721337f64;
        let mut item10: BTreeMap<String, Bytes> = BTreeMap::new();
        item10.insert(
            f64::_class_name(),
            TarsEncoder::individual_encode(&value10).unwrap(),
        );

        let key11 = "string".to_string();
        let value11 = String::from("hello wrold! foo bar!");
        let mut item11: BTreeMap<String, Bytes> = BTreeMap::new();
        item11.insert(
            String::_class_name(),
            TarsEncoder::individual_encode(&value11).unwrap(),
        );

        let key12 = "bytes".to_string();
        let value12 = Bytes::from("hello wrold! foo bar!");
        let mut item12: BTreeMap<String, Bytes> = BTreeMap::new();
        item12.insert(
            Bytes::_class_name(),
            TarsEncoder::individual_encode(&value12).unwrap(),
        );

        let key13 = "vec".to_string();
        let value13: Vec<u32> = vec![1, 2, 3, 4];
        let mut item13: BTreeMap<String, Bytes> = BTreeMap::new();
        item13.insert(
            Vec::<u32>::_class_name(),
            TarsEncoder::individual_encode(&value13).unwrap(),
        );

        let key14 = "map".to_string();
        let value14: BTreeMap<String, String> = BTreeMap::new();
        let mut item14: BTreeMap<String, Bytes> = BTreeMap::new();
        item14.insert(
            BTreeMap::<String, String>::_class_name(),
            TarsEncoder::individual_encode(&value14).unwrap(),
        );

        let fake_key = "fake_key".to_string();

        let mut map: BTreeMap<String, BTreeMap<String, Bytes>> = BTreeMap::new();

        map.insert(key0.clone(), item0);
        map.insert(key1.clone(), item1);
        map.insert(key2.clone(), item2);
        map.insert(key3.clone(), item3);
        map.insert(key4.clone(), item4);
        map.insert(key5.clone(), item5);
        map.insert(key6.clone(), item6);
        map.insert(key7.clone(), item7);
        map.insert(key8.clone(), item8);
        map.insert(key9.clone(), item9);
        map.insert(key10.clone(), item10);
        map.insert(key11.clone(), item11);
        map.insert(key12.clone(), item12);
        map.insert(key13.clone(), item13);
        map.insert(key14.clone(), item14);

        let uni = TupUniAttribute::from_bytes(
            &TarsEncoder::individual_encode(&map).unwrap(),
            ProtocolVersion::TupComplex,
        ).unwrap();

        let de_0: i64 = uni.read(&key0, true, 0).unwrap();
        assert_eq!(de_0, value0);

        let de_i8: i8 = uni.read(&key1, true, 0).unwrap();
        assert_eq!(de_i8, value1);

        let de_i16 = uni.read(&key2, true, 0).unwrap();
        assert_eq!(de_i16, value2);

        let de_i32 = uni.read(&key3, true, 0).unwrap();
        assert_eq!(de_i32, value3);

        let de_i64 = uni.read(&key4, true, 0).unwrap();
        assert_eq!(de_i64, value4);

        let de_bool = uni.read(&key5, true, false).unwrap();
        assert_eq!(de_bool, value5);

        let de_u8 = uni.read(&key6, true, 0).unwrap();
        assert_eq!(de_u8, value6);

        let de_u16 = uni.read(&key7, true, 0).unwrap();
        assert_eq!(de_u16, value7);

        let de_u32 = uni.read(&key8, true, 0).unwrap();
        assert_eq!(de_u32, value8);

        let de_f32 = uni.read(&key9, true, 0.0).unwrap();
        assert_eq!(de_f32, value9);

        let de_f64 = uni.read(&key10, true, 0.0).unwrap();
        assert_eq!(de_f64, value10);

        let de_string = uni.read(&key11, true, String::from("")).unwrap();
        assert_eq!(de_string, value11);

        let de_bytes = uni.read(&key12, true, Bytes::default()).unwrap();
        assert_eq!(de_bytes, value12);

        let de_vec: Vec<u32> = uni.read(&key13, true, vec![]).unwrap();
        assert_eq!(de_vec, value13);

        let de_map: BTreeMap<String, String> = uni.read(&key14, true, BTreeMap::new()).unwrap();
        assert_eq!(de_map, value14);

        let de_fake_value_err = uni.read(&fake_key, true, 0);
        assert_eq!(de_fake_value_err, Err(DecodeErr::TupKeyNotFoundErr));

        let de_fake_value = uni.read(&fake_key, false, 0).unwrap();
        assert_eq!(de_fake_value, 0);
    }

    #[test]
    fn test_encode_simple_tup() {
        let key0 = "zero".to_string();
        let value0 = 0;

        let key1 = "hello".to_string();
        let value1 = i8::max_value();

        let key2 = "world".to_string();
        let value2 = i16::max_value();

        let key3 = "aba".to_string();
        let value3 = i32::max_value();

        let key4 = "i64".to_string();
        let value4 = i64::max_value();

        let key5 = "bool".to_string();
        let value5 = true;

        let key6 = "u8".to_string();
        let value6 = u8::max_value();

        let key7 = "u16".to_string();
        let value7 = u16::max_value();

        let key8 = "u32".to_string();
        let value8 = u32::max_value();

        let key9 = "float".to_string();
        let value9 = 0.333f32;

        let key10 = "double".to_string();
        let value10 = 1.77721337f64;

        let key11 = "string".to_string();
        let value11 = String::from("hello wrold! foo bar!");

        let key12 = "bytes".to_string();
        let value12 = Bytes::from("hello wrold! foo bar!");

        let key13 = "vec".to_string();
        let value13: Vec<u32> = vec![1, 2, 3, 4];

        let key14 = "map".to_string();
        let value14: BTreeMap<String, String> = BTreeMap::new();

        let fake_key = "fake_key".to_string();

        let mut uni = TupUniAttribute::new(ProtocolVersion::TupSimple);

        uni.write(&key0, &value0).unwrap();
        uni.write(&key1, &value1).unwrap();
        uni.write(&key2, &value2).unwrap();
        uni.write(&key3, &value3).unwrap();
        uni.write(&key4, &value4).unwrap();
        uni.write(&key5, &value5).unwrap();
        uni.write(&key6, &value6).unwrap();
        uni.write(&key7, &value7).unwrap();
        uni.write(&key8, &value8).unwrap();
        uni.write(&key9, &value9).unwrap();
        uni.write(&key10, &value10).unwrap();
        uni.write(&key11, &value11).unwrap();
        uni.write(&key12, &value12).unwrap();
        uni.write(&key13, &value13).unwrap();
        uni.write(&key14, &value14).unwrap();

        let de_0: i64 = uni.read(&key0, true, 0).unwrap();
        assert_eq!(de_0, value0);

        let de_i8: i8 = uni.read(&key1, true, 0).unwrap();
        assert_eq!(de_i8, value1);

        let de_i16 = uni.read(&key2, true, 0).unwrap();
        assert_eq!(de_i16, value2);

        let de_i32 = uni.read(&key3, true, 0).unwrap();
        assert_eq!(de_i32, value3);

        let de_i64 = uni.read(&key4, true, 0).unwrap();
        assert_eq!(de_i64, value4);

        let de_bool = uni.read(&key5, true, false).unwrap();
        assert_eq!(de_bool, value5);

        let de_u8 = uni.read(&key6, true, 0).unwrap();
        assert_eq!(de_u8, value6);

        let de_u16 = uni.read(&key7, true, 0).unwrap();
        assert_eq!(de_u16, value7);

        let de_u32 = uni.read(&key8, true, 0).unwrap();
        assert_eq!(de_u32, value8);

        let de_f32 = uni.read(&key9, true, 0.0).unwrap();
        assert_eq!(de_f32, value9);

        let de_f64 = uni.read(&key10, true, 0.0).unwrap();
        assert_eq!(de_f64, value10);

        let de_string = uni.read(&key11, true, String::from("")).unwrap();
        assert_eq!(de_string, value11);

        let de_bytes = uni.read(&key12, true, Bytes::default()).unwrap();
        assert_eq!(de_bytes, value12);

        let de_vec: Vec<u32> = uni.read(&key13, true, vec![]).unwrap();
        assert_eq!(de_vec, value13);

        let de_map: BTreeMap<String, String> = uni.read(&key14, true, BTreeMap::new()).unwrap();
        assert_eq!(de_map, value14);

        let de_fake_value_err = uni.read(&fake_key, true, 0);
        assert_eq!(de_fake_value_err, Err(DecodeErr::TupKeyNotFoundErr));

        let de_fake_value = uni.read(&fake_key, false, 0).unwrap();
        assert_eq!(de_fake_value, 0);
    }

    #[test]
    fn test_encode_complex_tup() {
        let key0 = "zero".to_string();
        let value0 = 0;

        let key1 = "hello".to_string();
        let value1 = i8::max_value();

        let key2 = "world".to_string();
        let value2 = i16::max_value();

        let key3 = "aba".to_string();
        let value3 = i32::max_value();

        let key4 = "i64".to_string();
        let value4 = i64::max_value();

        let key5 = "bool".to_string();
        let value5 = true;

        let key6 = "u8".to_string();
        let value6 = u8::max_value();

        let key7 = "u16".to_string();
        let value7 = u16::max_value();

        let key8 = "u32".to_string();
        let value8 = u32::max_value();

        let key9 = "float".to_string();
        let value9 = 0.333f32;

        let key10 = "double".to_string();
        let value10 = 1.77721337f64;

        let key11 = "string".to_string();
        let value11 = String::from("hello wrold! foo bar!");

        let key12 = "bytes".to_string();
        let value12 = Bytes::from("hello wrold! foo bar!");

        let key13 = "vec".to_string();
        let value13: Vec<u32> = vec![1, 2, 3, 4];

        let key14 = "map".to_string();
        let value14: BTreeMap<String, String> = BTreeMap::new();

        let fake_key = "fake_key".to_string();

        let mut uni = TupUniAttribute::new(ProtocolVersion::TupComplex);

        uni.write(&key0, &value0).unwrap();
        uni.write(&key1, &value1).unwrap();
        uni.write(&key2, &value2).unwrap();
        uni.write(&key3, &value3).unwrap();
        uni.write(&key4, &value4).unwrap();
        uni.write(&key5, &value5).unwrap();
        uni.write(&key6, &value6).unwrap();
        uni.write(&key7, &value7).unwrap();
        uni.write(&key8, &value8).unwrap();
        uni.write(&key9, &value9).unwrap();
        uni.write(&key10, &value10).unwrap();
        uni.write(&key11, &value11).unwrap();
        uni.write(&key12, &value12).unwrap();
        uni.write(&key13, &value13).unwrap();
        uni.write(&key14, &value14).unwrap();

        let de_0: i64 = uni.read(&key0, true, 0).unwrap();
        assert_eq!(de_0, value0);

        let de_i8: i8 = uni.read(&key1, true, 0).unwrap();
        assert_eq!(de_i8, value1);

        let de_i16 = uni.read(&key2, true, 0).unwrap();
        assert_eq!(de_i16, value2);

        let de_i32 = uni.read(&key3, true, 0).unwrap();
        assert_eq!(de_i32, value3);

        let de_i64 = uni.read(&key4, true, 0).unwrap();
        assert_eq!(de_i64, value4);

        let de_bool = uni.read(&key5, true, false).unwrap();
        assert_eq!(de_bool, value5);

        let de_u8 = uni.read(&key6, true, 0).unwrap();
        assert_eq!(de_u8, value6);

        let de_u16 = uni.read(&key7, true, 0).unwrap();
        assert_eq!(de_u16, value7);

        let de_u32 = uni.read(&key8, true, 0).unwrap();
        assert_eq!(de_u32, value8);

        let de_f32 = uni.read(&key9, true, 0.0).unwrap();
        assert_eq!(de_f32, value9);

        let de_f64 = uni.read(&key10, true, 0.0).unwrap();
        assert_eq!(de_f64, value10);

        let de_string = uni.read(&key11, true, String::from("")).unwrap();
        assert_eq!(de_string, value11);

        let de_bytes = uni.read(&key12, true, Bytes::default()).unwrap();
        assert_eq!(de_bytes, value12);

        let de_vec: Vec<u32> = uni.read(&key13, true, vec![]).unwrap();
        assert_eq!(de_vec, value13);

        let de_map: BTreeMap<String, String> = uni.read(&key14, true, BTreeMap::new()).unwrap();
        assert_eq!(de_map, value14);

        let de_fake_value_err = uni.read(&fake_key, true, 0);
        assert_eq!(de_fake_value_err, Err(DecodeErr::TupKeyNotFoundErr));

        let de_fake_value = uni.read(&fake_key, false, 0).unwrap();
        assert_eq!(de_fake_value, 0);
    }
}
