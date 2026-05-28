#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TarsTypeMark {
    EnInt8 = 0,
    EnInt16 = 1,
    EnInt32 = 2,
    EnInt64 = 3,
    EnFloat = 4,
    EnDouble = 5,
    EnString1 = 6,
    EnString4 = 7,
    EnMaps = 8,
    EnList = 9,
    EnStructBegin = 10,
    EnStructEnd = 11,
    EnZero = 12,
    EnSimplelist = 13,
}

impl TarsTypeMark {
    pub fn value(self) -> u8 {
        self as u8
    }
}

impl From<u8> for TarsTypeMark {
    fn from(v: u8) -> Self {
        match v {
            0 => TarsTypeMark::EnInt8,
            1 => TarsTypeMark::EnInt16,
            2 => TarsTypeMark::EnInt32,
            3 => TarsTypeMark::EnInt64,
            4 => TarsTypeMark::EnFloat,
            5 => TarsTypeMark::EnDouble,
            6 => TarsTypeMark::EnString1,
            7 => TarsTypeMark::EnString4,
            8 => TarsTypeMark::EnMaps,
            9 => TarsTypeMark::EnList,
            10 => TarsTypeMark::EnStructBegin,
            11 => TarsTypeMark::EnStructEnd,
            12 => TarsTypeMark::EnZero,
            13 => TarsTypeMark::EnSimplelist,
            _ => TarsTypeMark::EnZero, // unknown type, read nothing from buffer
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProtocolVersion {
    Tars = 1,
    TupSimple = 2,
    TupComplex = 3,
}

impl ProtocolVersion {
    pub fn value(self) -> i8 {
        self as i8
    }
}

impl From<i8> for ProtocolVersion {
    fn from(v: i8) -> Self {
        if v == 1 {
            ProtocolVersion::Tars
        } else if v == 2 {
            ProtocolVersion::TupSimple
        } else {
            ProtocolVersion::TupComplex
        }
    }
}
