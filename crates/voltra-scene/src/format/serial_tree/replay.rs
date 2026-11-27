//! A [`SerialTree`] played back into any serializer: the calls it recorded,
//! in order, with the names they had.

use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};

use super::SerialTree;

impl Serialize for SerialTree {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Bool(v) => s.serialize_bool(*v),
            Self::I8(v) => s.serialize_i8(*v),
            Self::I16(v) => s.serialize_i16(*v),
            Self::I32(v) => s.serialize_i32(*v),
            Self::I64(v) => s.serialize_i64(*v),
            Self::I128(v) => s.serialize_i128(*v),
            Self::U8(v) => s.serialize_u8(*v),
            Self::U16(v) => s.serialize_u16(*v),
            Self::U32(v) => s.serialize_u32(*v),
            Self::U64(v) => s.serialize_u64(*v),
            Self::U128(v) => s.serialize_u128(*v),
            Self::F32(v) => s.serialize_f32(v.0),
            Self::F64(v) => s.serialize_f64(v.0),
            Self::Char(v) => s.serialize_char(*v),
            Self::Str(v) => s.serialize_str(v),
            Self::Bytes(v) => s.serialize_bytes(v),
            Self::None => s.serialize_none(),
            Self::Some(v) => s.serialize_some(v.as_ref()),
            Self::Unit => s.serialize_unit(),
            Self::UnitStruct(name) => s.serialize_unit_struct(name),
            Self::UnitVariant {
                name,
                index,
                variant,
            } => s.serialize_unit_variant(name, *index, variant),
            Self::NewtypeStruct { name, value } => s.serialize_newtype_struct(name, value.as_ref()),
            Self::NewtypeVariant {
                name,
                index,
                variant,
                value,
            } => s.serialize_newtype_variant(name, *index, variant, value.as_ref()),
            Self::Seq(elements) => {
                let mut seq = s.serialize_seq(Some(elements.len()))?;
                for element in elements {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            Self::Tuple(elements) => {
                let mut tuple = s.serialize_tuple(elements.len())?;
                for element in elements {
                    tuple.serialize_element(element)?;
                }
                tuple.end()
            }
            Self::TupleStruct { name, fields } => {
                let mut tuple = s.serialize_tuple_struct(name, fields.len())?;
                for field in fields {
                    tuple.serialize_field(field)?;
                }
                tuple.end()
            }
            Self::TupleVariant {
                name,
                index,
                variant,
                fields,
            } => {
                let mut tuple = s.serialize_tuple_variant(name, *index, variant, fields.len())?;
                for field in fields {
                    tuple.serialize_field(field)?;
                }
                tuple.end()
            }
            Self::Map(entries) => {
                let mut map = s.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
            Self::Struct { name, fields } => {
                let mut record = s.serialize_struct(name, fields.len())?;
                for (key, value) in fields {
                    record.serialize_field(key, value)?;
                }
                record.end()
            }
            Self::StructVariant {
                name,
                index,
                variant,
                fields,
            } => {
                let mut record = s.serialize_struct_variant(name, *index, variant, fields.len())?;
                for (key, value) in fields {
                    record.serialize_field(key, value)?;
                }
                record.end()
            }
        }
    }
}
