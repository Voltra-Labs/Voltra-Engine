//! The serializer that writes a [`SerialTree`] instead of text: each call
//! becomes the node of the same name.

use serde::ser::{self, Serialize};

use super::SerialTree;

type Error = ron::Error;

/// Records one value. Stateless: every compound gets its own collector.
pub(super) struct Recorder;

fn record<T: Serialize + ?Sized>(value: &T) -> Result<SerialTree, Error> {
    value.serialize(Recorder)
}

impl ser::Serializer for Recorder {
    type Ok = SerialTree;
    type Error = Error;
    type SerializeSeq = Elements;
    type SerializeTuple = Elements;
    type SerializeTupleStruct = Elements;
    type SerializeTupleVariant = Elements;
    type SerializeMap = Entries;
    type SerializeStruct = Fields;
    type SerializeStructVariant = Fields;

    fn serialize_bool(self, v: bool) -> Result<SerialTree, Error> {
        Ok(SerialTree::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<SerialTree, Error> {
        Ok(SerialTree::I8(v))
    }
    fn serialize_i16(self, v: i16) -> Result<SerialTree, Error> {
        Ok(SerialTree::I16(v))
    }
    fn serialize_i32(self, v: i32) -> Result<SerialTree, Error> {
        Ok(SerialTree::I32(v))
    }
    fn serialize_i64(self, v: i64) -> Result<SerialTree, Error> {
        Ok(SerialTree::I64(v))
    }
    fn serialize_i128(self, v: i128) -> Result<SerialTree, Error> {
        Ok(SerialTree::I128(v))
    }
    fn serialize_u8(self, v: u8) -> Result<SerialTree, Error> {
        Ok(SerialTree::U8(v))
    }
    fn serialize_u16(self, v: u16) -> Result<SerialTree, Error> {
        Ok(SerialTree::U16(v))
    }
    fn serialize_u32(self, v: u32) -> Result<SerialTree, Error> {
        Ok(SerialTree::U32(v))
    }
    fn serialize_u64(self, v: u64) -> Result<SerialTree, Error> {
        Ok(SerialTree::U64(v))
    }
    fn serialize_u128(self, v: u128) -> Result<SerialTree, Error> {
        Ok(SerialTree::U128(v))
    }
    fn serialize_f32(self, v: f32) -> Result<SerialTree, Error> {
        Ok(SerialTree::F32(super::Exact(v)))
    }
    fn serialize_f64(self, v: f64) -> Result<SerialTree, Error> {
        Ok(SerialTree::F64(super::Exact(v)))
    }
    fn serialize_char(self, v: char) -> Result<SerialTree, Error> {
        Ok(SerialTree::Char(v))
    }
    fn serialize_str(self, v: &str) -> Result<SerialTree, Error> {
        Ok(SerialTree::Str(v.to_owned()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<SerialTree, Error> {
        Ok(SerialTree::Bytes(v.to_vec()))
    }
    fn serialize_none(self) -> Result<SerialTree, Error> {
        Ok(SerialTree::None)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<SerialTree, Error> {
        Ok(SerialTree::Some(Box::new(record(value)?)))
    }
    fn serialize_unit(self) -> Result<SerialTree, Error> {
        Ok(SerialTree::Unit)
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<SerialTree, Error> {
        Ok(SerialTree::UnitStruct(name))
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<SerialTree, Error> {
        Ok(SerialTree::UnitVariant {
            name,
            index,
            variant,
        })
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<SerialTree, Error> {
        Ok(SerialTree::NewtypeStruct {
            name,
            value: Box::new(record(value)?),
        })
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<SerialTree, Error> {
        Ok(SerialTree::NewtypeVariant {
            name,
            index,
            variant,
            value: Box::new(record(value)?),
        })
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Elements, Error> {
        Ok(Elements::new(Shape::Seq, len.unwrap_or(0)))
    }
    fn serialize_tuple(self, len: usize) -> Result<Elements, Error> {
        Ok(Elements::new(Shape::Tuple, len))
    }
    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Elements, Error> {
        Ok(Elements::new(Shape::TupleStruct(name), len))
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Elements, Error> {
        Ok(Elements::new(
            Shape::TupleVariant {
                name,
                index,
                variant,
            },
            len,
        ))
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Entries, Error> {
        Ok(Entries {
            entries: Vec::with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Fields, Error> {
        Ok(Fields {
            variant: None,
            name,
            fields: Vec::with_capacity(len),
        })
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Fields, Error> {
        Ok(Fields {
            variant: Some((index, variant)),
            name,
            fields: Vec::with_capacity(len),
        })
    }
}

/// Which of the four element-list nodes an [`Elements`] becomes.
enum Shape {
    Seq,
    Tuple,
    TupleStruct(&'static str),
    TupleVariant {
        name: &'static str,
        index: u32,
        variant: &'static str,
    },
}

/// A sequence, tuple, tuple struct or tuple variant being recorded.
pub(super) struct Elements {
    shape: Shape,
    elements: Vec<SerialTree>,
}

impl Elements {
    fn new(shape: Shape, len: usize) -> Self {
        Self {
            shape,
            elements: Vec::with_capacity(len),
        }
    }

    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.elements.push(record(value)?);
        Ok(())
    }

    fn finish(self) -> SerialTree {
        let fields = self.elements;
        match self.shape {
            Shape::Seq => SerialTree::Seq(fields),
            Shape::Tuple => SerialTree::Tuple(fields),
            Shape::TupleStruct(name) => SerialTree::TupleStruct { name, fields },
            Shape::TupleVariant {
                name,
                index,
                variant,
            } => SerialTree::TupleVariant {
                name,
                index,
                variant,
                fields,
            },
        }
    }
}

impl ser::SerializeSeq for Elements {
    type Ok = SerialTree;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }
    fn end(self) -> Result<SerialTree, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTuple for Elements {
    type Ok = SerialTree;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }
    fn end(self) -> Result<SerialTree, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleStruct for Elements {
    type Ok = SerialTree;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }
    fn end(self) -> Result<SerialTree, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleVariant for Elements {
    type Ok = SerialTree;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }
    fn end(self) -> Result<SerialTree, Error> {
        Ok(self.finish())
    }
}

/// A map being recorded, and the key waiting for its value.
pub(super) struct Entries {
    entries: Vec<(SerialTree, SerialTree)>,
    key: Option<SerialTree>,
}

impl ser::SerializeMap for Entries {
    type Ok = SerialTree;
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.key = Some(record(key)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .key
            .take()
            .ok_or_else(|| <Error as ser::Error>::custom("a map value came before its key"))?;
        self.entries.push((key, record(value)?));
        Ok(())
    }
    fn end(self) -> Result<SerialTree, Error> {
        Ok(SerialTree::Map(self.entries))
    }
}

/// A struct or struct variant being recorded.
pub(super) struct Fields {
    /// The variant's index and name, for a struct variant.
    variant: Option<(u32, &'static str)>,
    name: &'static str,
    fields: Vec<(&'static str, SerialTree)>,
}

impl Fields {
    fn push<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
        self.fields.push((key, record(value)?));
        Ok(())
    }

    fn finish(self) -> SerialTree {
        match self.variant {
            None => SerialTree::Struct {
                name: self.name,
                fields: self.fields,
            },
            Some((index, variant)) => SerialTree::StructVariant {
                name: self.name,
                index,
                variant,
                fields: self.fields,
            },
        }
    }
}

impl ser::SerializeStruct for Fields {
    type Ok = SerialTree;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.push(key, value)
    }
    fn end(self) -> Result<SerialTree, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeStructVariant for Fields {
    type Ok = SerialTree;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.push(key, value)
    }
    fn end(self) -> Result<SerialTree, Error> {
        Ok(self.finish())
    }
}
