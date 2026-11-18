//! A tile's mark: a hash of its typed document, taken without writing it out.
//!
//! Every value is fed in with a tag, and every string and sequence with its
//! length, so two documents feed the same stream exactly when they would
//! serialize to the same JSON.

use serde::ser::{self, Serialize};
use std::hash::Hasher;

/// The mark of a value: equal values mark alike, and different values mark
/// apart but for a 64-bit collision.
pub(crate) fn mark_of(value: &impl Serialize) -> u64 {
    let mut marker = Marker(crate::fast_map::FastHasher::default());
    // A `Marker` cannot fail; the type only names an error for `serde`.
    let _ = value.serialize(&mut marker);
    marker.0.finish()
}

struct Marker(crate::fast_map::FastHasher);

#[derive(Debug)]
struct Never;

impl std::fmt::Display for Never {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a mark cannot fail")
    }
}

impl std::error::Error for Never {}

impl ser::Error for Never {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Never
    }
}

impl Marker {
    fn tag(&mut self, tag: u8) {
        self.0.write_u8(tag);
    }
}

impl ser::Serializer for &mut Marker {
    type Ok = ();
    type Error = Never;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<(), Never> {
        self.tag(1);
        self.0.write_u8(u8::from(v));
        Ok(())
    }
    fn serialize_i8(self, v: i8) -> Result<(), Never> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i16(self, v: i16) -> Result<(), Never> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i32(self, v: i32) -> Result<(), Never> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i64(self, v: i64) -> Result<(), Never> {
        // JSON writes a non-negative integer the same whatever its type.
        if v >= 0 {
            return self.serialize_u64(v as u64);
        }
        self.tag(2);
        self.0.write_i64(v);
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<(), Never> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u16(self, v: u16) -> Result<(), Never> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u32(self, v: u32) -> Result<(), Never> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u64(self, v: u64) -> Result<(), Never> {
        self.tag(3);
        self.0.write_u64(v);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Never> {
        self.serialize_f64(f64::from(v))
    }
    fn serialize_f64(self, v: f64) -> Result<(), Never> {
        // JSON has no NaN or infinity: `serde_json` writes them as null.
        if !v.is_finite() {
            return self.serialize_none();
        }
        self.tag(4);
        self.0.write_u64(v.to_bits());
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<(), Never> {
        let mut buffer = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buffer))
    }
    fn serialize_str(self, v: &str) -> Result<(), Never> {
        self.tag(5);
        self.0.write_usize(v.len());
        self.0.write(v.as_bytes());
        Ok(())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Never> {
        self.tag(6);
        self.0.write_usize(v.len());
        self.0.write(v);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Never> {
        self.tag(0);
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Never> {
        // `Some(x)` serializes as `x`, as `serde_json` writes it.
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Never> {
        self.serialize_none()
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Never> {
        self.serialize_none()
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<(), Never> {
        self.serialize_str(variant)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        self.tag(7);
        self.serialize_str(variant)?;
        value.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self, Never> {
        self.tag(8);
        self.0.write_usize(len.unwrap_or(usize::MAX));
        Ok(self)
    }
    fn serialize_tuple(self, len: usize) -> Result<Self, Never> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self, Never> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self, Never> {
        self.tag(7);
        self.serialize_str(variant)?;
        self.serialize_seq(Some(len))
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self, Never> {
        self.tag(9);
        self.0.write_usize(len.unwrap_or(usize::MAX));
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<Self, Never> {
        self.serialize_map(Some(len))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self, Never> {
        self.tag(7);
        self.serialize_str(variant)?;
        self.serialize_map(Some(len))
    }
}

macro_rules! element {
    ($trait:ident, $method:ident) => {
        impl<'a> ser::$trait for &'a mut Marker {
            type Ok = ();
            type Error = Never;
            fn $method<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
                value.serialize(&mut **self)
            }
            fn end(self) -> Result<(), Never> {
                self.tag(10);
                Ok(())
            }
        }
    };
}

element!(SerializeSeq, serialize_element);
element!(SerializeTuple, serialize_element);
element!(SerializeTupleStruct, serialize_field);
element!(SerializeTupleVariant, serialize_field);

impl ser::SerializeMap for &mut Marker {
    type Ok = ();
    type Error = Never;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Never> {
        key.serialize(&mut **self)
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Never> {
        self.tag(10);
        Ok(())
    }
}

macro_rules! fields {
    ($trait:ident) => {
        impl<'a> ser::$trait for &'a mut Marker {
            type Ok = ();
            type Error = Never;
            fn serialize_field<T: ?Sized + Serialize>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<(), Never> {
                ser::Serializer::serialize_str(&mut **self, key)?;
                value.serialize(&mut **self)
            }
            fn end(self) -> Result<(), Never> {
                self.tag(10);
                Ok(())
            }
        }
    };
}

fields!(SerializeStruct);
fields!(SerializeStructVariant);
