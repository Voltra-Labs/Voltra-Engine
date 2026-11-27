//! A component's value as its `Serialize` writes it, kept as a tree a tool
//! can compare and write into field by field.
//!
//! **Why not `ron::Value`.** Reading RON into a `ron::Value` drops every
//! struct and variant name: `Box(half_extents: (1.0, 1.0))` comes back as a
//! map, and written out again it no longer reads as the enum it was. This
//! tree records the serializer's own calls instead — every name serde hands
//! a serializer is `&'static str` — and [`SerialTree::to_raw`] replays them
//! into RON, so what it writes is, byte for byte, what the type writes.
//! Unity's `SerializedProperty` is the same idea: a value reached by its
//! serialized shape and a property path, with no reflection.
//!
//! **Equality is the saved value's.** Floats compare by their bits, so
//! `0.1` and `0.1000001` differ and `-0.0` differs from `0.0` (a file
//! writes them differently), except that every NaN equals every other NaN —
//! a file writes each of them as `NaN`.
//!
//! [`differences`] answers where two values part, as [`TreePath`]s;
//! [`SerialTree::find`] and [`SerialTree::put`] read and write one —
//! including a field `skip_serializing_if` leaves out, which is absent
//! rather than nowhere, and written back by being left out again.

mod path;
mod record;
mod replay;

pub use path::{describe, differences, Found, Step, TreePath};

use ron::value::RawValue;
use serde::Serialize;

use super::error::SceneError;

/// A float compared as a saved file compares it: by its bits, every NaN
/// equal to every other.
#[derive(Debug, Clone, Copy)]
pub struct Exact<F>(pub F);

impl PartialEq for Exact<f32> {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits() || (self.0.is_nan() && other.0.is_nan())
    }
}

impl PartialEq for Exact<f64> {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits() || (self.0.is_nan() && other.0.is_nan())
    }
}

/// One value, as the serializer was told it: one variant per serde data
/// model type, with every name serde gave.
#[derive(Debug, Clone, PartialEq)]
pub enum SerialTree {
    /// `serialize_bool`.
    Bool(bool),
    /// `serialize_i8`.
    I8(i8),
    /// `serialize_i16`.
    I16(i16),
    /// `serialize_i32`.
    I32(i32),
    /// `serialize_i64`.
    I64(i64),
    /// `serialize_i128`.
    I128(i128),
    /// `serialize_u8`.
    U8(u8),
    /// `serialize_u16`.
    U16(u16),
    /// `serialize_u32`.
    U32(u32),
    /// `serialize_u64`.
    U64(u64),
    /// `serialize_u128`.
    U128(u128),
    /// `serialize_f32`, compared exactly.
    F32(Exact<f32>),
    /// `serialize_f64`, compared exactly.
    F64(Exact<f64>),
    /// `serialize_char`.
    Char(char),
    /// `serialize_str`.
    Str(String),
    /// `serialize_bytes`.
    Bytes(Vec<u8>),
    /// `serialize_none`: an `Option` with nothing in it.
    None,
    /// `serialize_some`: an `Option` and what it holds.
    Some(Box<SerialTree>),
    /// `serialize_unit`: `()`.
    Unit,
    /// `serialize_unit_struct`: a struct with no fields, by name.
    UnitStruct(&'static str),
    /// `serialize_unit_variant`: an enum variant with no data.
    UnitVariant {
        /// The enum's name.
        name: &'static str,
        /// The variant's position in the enum.
        index: u32,
        /// The variant's name.
        variant: &'static str,
    },
    /// `serialize_newtype_struct`: a struct wrapping one value.
    NewtypeStruct {
        /// The struct's name.
        name: &'static str,
        /// What it wraps.
        value: Box<SerialTree>,
    },
    /// `serialize_newtype_variant`: an enum variant wrapping one value.
    NewtypeVariant {
        /// The enum's name.
        name: &'static str,
        /// The variant's position in the enum.
        index: u32,
        /// The variant's name.
        variant: &'static str,
        /// What it wraps.
        value: Box<SerialTree>,
    },
    /// `serialize_seq`: a list of any length.
    Seq(Vec<SerialTree>),
    /// `serialize_tuple`: a fixed number of values.
    Tuple(Vec<SerialTree>),
    /// `serialize_tuple_struct`: a struct with unnamed fields, by name.
    TupleStruct {
        /// The struct's name.
        name: &'static str,
        /// Its fields, in order.
        fields: Vec<SerialTree>,
    },
    /// `serialize_tuple_variant`: an enum variant with unnamed fields.
    TupleVariant {
        /// The enum's name.
        name: &'static str,
        /// The variant's position in the enum.
        index: u32,
        /// The variant's name.
        variant: &'static str,
        /// Its fields, in order.
        fields: Vec<SerialTree>,
    },
    /// `serialize_map`: keys and values, in the order written.
    Map(Vec<(SerialTree, SerialTree)>),
    /// `serialize_struct`: named fields — those written; a field
    /// `skip_serializing_if` skipped is not here.
    Struct {
        /// The struct's name.
        name: &'static str,
        /// Its fields as written, by name.
        fields: Vec<(&'static str, SerialTree)>,
    },
    /// `serialize_struct_variant`: an enum variant with named fields.
    StructVariant {
        /// The enum's name.
        name: &'static str,
        /// The variant's position in the enum.
        index: u32,
        /// The variant's name.
        variant: &'static str,
        /// Its fields as written, by name.
        fields: Vec<(&'static str, SerialTree)>,
    },
}

impl SerialTree {
    /// `value` as its `Serialize` writes it.
    pub fn record<T: Serialize + ?Sized>(value: &T) -> Result<Self, SceneError> {
        value
            .serialize(record::Recorder)
            .map_err(SceneError::Serialize)
    }

    /// The tree written out as RON: exactly the text the recorded value's
    /// own `Serialize` writes, so it loads back as that type.
    pub fn to_raw(&self) -> Result<Box<RawValue>, SceneError> {
        RawValue::from_rust(self).map_err(SceneError::Serialize)
    }
}

#[cfg(test)]
mod tests;
