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
//! [`SerialTree::at`] and [`SerialTree::set`] read and write one.

mod path;
mod record;
mod replay;

pub use path::{describe, differences, Step, TreePath};

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
#[allow(missing_docs)] // One per serde data model type; the names are serde's.
pub enum SerialTree {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    F32(Exact<f32>),
    F64(Exact<f64>),
    Char(char),
    Str(String),
    Bytes(Vec<u8>),
    None,
    Some(Box<SerialTree>),
    Unit,
    UnitStruct(&'static str),
    UnitVariant {
        name: &'static str,
        index: u32,
        variant: &'static str,
    },
    NewtypeStruct {
        name: &'static str,
        value: Box<SerialTree>,
    },
    NewtypeVariant {
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: Box<SerialTree>,
    },
    Seq(Vec<SerialTree>),
    Tuple(Vec<SerialTree>),
    TupleStruct {
        name: &'static str,
        fields: Vec<SerialTree>,
    },
    TupleVariant {
        name: &'static str,
        index: u32,
        variant: &'static str,
        fields: Vec<SerialTree>,
    },
    Map(Vec<(SerialTree, SerialTree)>),
    Struct {
        name: &'static str,
        fields: Vec<(&'static str, SerialTree)>,
    },
    StructVariant {
        name: &'static str,
        index: u32,
        variant: &'static str,
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
