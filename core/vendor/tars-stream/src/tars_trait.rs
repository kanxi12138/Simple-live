use bytes::Bytes;
use errors::{DecodeErr, EncodeErr};
use std::collections::BTreeMap;
use tars_decoder::TarsDecoder;
use tars_encoder::TarsEncoder;

// Tars Struct 需要实现此trait
pub trait StructFromTars {
    fn _decode_from(decoder: &mut TarsDecoder) -> Result<Self, DecodeErr>
    where
        Self: Sized;
}

// Tars Struct 需要实现此trait
pub trait StructToTars {
    fn _encode_to(&self, encoder: &mut TarsEncoder) -> Result<(), EncodeErr>;
}

// Tars Enum 需要实现此 trait
pub trait EnumFromI32 {
    fn _from_i32(ele: i32) -> Result<Self, DecodeErr>
    where
        Self: Sized;
}

// Tars Enum 需要实现此 trait
pub trait EnumToI32 {
    fn _to_i32(&self) -> i32;
}

// Tars 所有类型需要实现此trait
pub trait ClassName {
    fn _class_name() -> String;
}

impl ClassName for bool {
    fn _class_name() -> String {
        String::from("bool")
    }
}

impl ClassName for i8 {
    fn _class_name() -> String {
        String::from("char")
    }
}

impl ClassName for i16 {
    fn _class_name() -> String {
        String::from("short")
    }
}

impl ClassName for i32 {
    fn _class_name() -> String {
        String::from("int32")
    }
}

impl ClassName for i64 {
    fn _class_name() -> String {
        String::from("int64")
    }
}

impl ClassName for u8 {
    fn _class_name() -> String {
        String::from("short")
    }
}

impl ClassName for u16 {
    fn _class_name() -> String {
        String::from("int32")
    }
}

impl ClassName for u32 {
    fn _class_name() -> String {
        String::from("int64")
    }
}

impl ClassName for f32 {
    fn _class_name() -> String {
        String::from("float")
    }
}

impl ClassName for f64 {
    fn _class_name() -> String {
        String::from("double")
    }
}

impl ClassName for String {
    fn _class_name() -> String {
        String::from("string")
    }
}

impl<K, V> ClassName for BTreeMap<K, V>
where
    K: ClassName + Ord,
    V: ClassName,
{
    fn _class_name() -> String {
        String::from("map<")
            + &K::_class_name()
            + &String::from(",")
            + &V::_class_name()
            + &String::from(">")
    }
}

impl<T> ClassName for Vec<T>
where
    T: ClassName,
{
    fn _class_name() -> String {
        // List not list
        String::from("List<") + &T::_class_name() + &String::from(">")
    }
}

impl ClassName for Bytes {
    fn _class_name() -> String {
        String::from("list<byte>")
    }
}
