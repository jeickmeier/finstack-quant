//! A fork of `serde_json::value::Serializer` (serde_json 1.x) that rejects
//! non-finite floats instead of serializing them as `null`.
//!
//! The upstream value serializer cannot be reused because its `null` output
//! for NaN and infinities is indistinguishable from a genuine `None`, so
//! canonical hashing could not detect the corruption afterwards. Keep this
//! module in step with the upstream serializer when serde_json is upgraded.

use serde::ser::{
    self, Impossible, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};
use serde::{Serialize, Serializer};
use serde_json::{Map, Number, Value};

use crate::NonFiniteKind;

// RawValue advertises this pseudo-struct to serializers. Delegate that complete
// pseudo-struct to serde_json's public Value serializer so serde_json, rather
// than this module, owns the embedded-JSON decoding behavior.
const SERDE_JSON_RAW_VALUE_TOKEN: &str = "$serde_json::private::RawValue";

pub(super) fn to_value<T: ?Sized + Serialize>(value: &T) -> Result<Value, CanonicalValueError> {
    value.serialize(ValueSerializer)
}

#[derive(Debug)]
pub(super) enum CanonicalValueError {
    NonFinite(NonFiniteKind),
    Custom(String),
}

impl std::fmt::Display for CanonicalValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite(kind) => write!(formatter, "non-finite value: {kind}"),
            Self::Custom(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CanonicalValueError {}

impl ser::Error for CanonicalValueError {
    fn custom<T: std::fmt::Display>(message: T) -> Self {
        Self::Custom(message.to_string())
    }
}

struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;
    type SerializeSeq = SequenceSerializer;
    type SerializeTuple = SequenceSerializer;
    type SerializeTupleStruct = SequenceSerializer;
    type SerializeTupleVariant = TupleVariantSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = StructSerializer;
    type SerializeStructVariant = StructVariantSerializer;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(value))
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(Number::from(value)))
    }

    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        if let Ok(unsigned) = u64::try_from(value) {
            self.serialize_u64(unsigned)
        } else if let Ok(signed) = i64::try_from(value) {
            self.serialize_i64(signed)
        } else {
            Err(CanonicalValueError::Custom(
                "JSON number is outside the supported integer range".to_string(),
            ))
        }
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(Number::from(value)))
    }

    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        u64::try_from(value).map_or_else(
            |_| {
                Err(CanonicalValueError::Custom(
                    "JSON number is outside the supported integer range".to_string(),
                ))
            },
            |unsigned| self.serialize_u64(unsigned),
        )
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        finite_number(f64::from(value), NonFiniteKind::classify(f64::from(value)))
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        finite_number(value, NonFiniteKind::classify(value))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Array(
            value
                .iter()
                .map(|byte| Value::Number(Number::from(*byte)))
                .collect(),
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        let mut object = Map::new();
        object.insert(variant.to_string(), to_value(value)?);
        Ok(Value::Object(object))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SequenceSerializer {
            values: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(TupleVariantSerializer {
            variant: variant.to_string(),
            values: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(MapSerializer {
            values: Map::with_capacity(len.unwrap_or(0)),
            next_key: None,
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        if name == SERDE_JSON_RAW_VALUE_TOKEN {
            serde_json::value::Serializer
                .serialize_struct(name, len)
                .map(StructSerializer::SerdeJson)
                .map_err(|error| CanonicalValueError::Custom(error.to_string()))
        } else {
            self.serialize_map(Some(len))
                .map(StructSerializer::Canonical)
        }
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(StructVariantSerializer {
            variant: variant.to_string(),
            values: Map::with_capacity(len),
        })
    }

    fn collect_str<T: ?Sized + std::fmt::Display>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

struct SequenceSerializer {
    values: Vec<Value>,
}

impl SerializeSeq for SequenceSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.values.push(to_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Array(self.values))
    }
}

impl SerializeTuple for SequenceSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleStruct for SequenceSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

struct TupleVariantSerializer {
    variant: String,
    values: Vec<Value>,
}

impl SerializeTupleVariant for TupleVariantSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.values.push(to_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut object = Map::new();
        object.insert(self.variant, Value::Array(self.values));
        Ok(Value::Object(object))
    }
}

struct MapSerializer {
    values: Map<String, Value>,
    next_key: Option<String>,
}

type SerdeJsonStructSerializer = <serde_json::value::Serializer as Serializer>::SerializeStruct;

enum StructSerializer {
    Canonical(MapSerializer),
    SerdeJson(SerdeJsonStructSerializer),
}

impl SerializeStruct for StructSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        match self {
            Self::Canonical(serializer) => serializer.serialize_field(key, value),
            Self::SerdeJson(serializer) => serializer
                .serialize_field(key, value)
                .map_err(|error| CanonicalValueError::Custom(error.to_string())),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Self::Canonical(serializer) => SerializeStruct::end(serializer),
            Self::SerdeJson(serializer) => SerializeStruct::end(serializer)
                .map_err(|error| CanonicalValueError::Custom(error.to_string())),
        }
    }
}

impl SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.next_key = Some(key.serialize(MapKeySerializer)?);
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let key = self.next_key.take().ok_or_else(|| {
            CanonicalValueError::Custom(
                "serialize_value was called before serialize_key".to_string(),
            )
        })?;
        self.values.insert(key, to_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.next_key.is_some() {
            return Err(CanonicalValueError::Custom(
                "serialize_map ended before serializing a value".to_string(),
            ));
        }
        Ok(Value::Object(self.values))
    }
}

impl SerializeStruct for MapSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.values.insert(key.to_string(), to_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Object(self.values))
    }
}

struct StructVariantSerializer {
    variant: String,
    values: Map<String, Value>,
}

impl SerializeStructVariant for StructVariantSerializer {
    type Ok = Value;
    type Error = CanonicalValueError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.values.insert(key.to_string(), to_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut object = Map::new();
        object.insert(self.variant, Value::Object(self.values));
        Ok(Value::Object(object))
    }
}

struct MapKeySerializer;

impl Serializer for MapKeySerializer {
    type Ok = String;
    type Error = CanonicalValueError;
    type SerializeSeq = Impossible<String, CanonicalValueError>;
    type SerializeTuple = Impossible<String, CanonicalValueError>;
    type SerializeTupleStruct = Impossible<String, CanonicalValueError>;
    type SerializeTupleVariant = Impossible<String, CanonicalValueError>;
    type SerializeMap = Impossible<String, CanonicalValueError>;
    type SerializeStruct = Impossible<String, CanonicalValueError>;
    type SerializeStructVariant = Impossible<String, CanonicalValueError>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            Ok(value.to_string())
        } else {
            Err(CanonicalValueError::NonFinite(NonFiniteKind::classify(
                f64::from(value),
            )))
        }
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            Ok(value.to_string())
        } else {
            Err(CanonicalValueError::NonFinite(NonFiniteKind::classify(
                value,
            )))
        }
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(variant.to_string())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(key_must_be_string())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(key_must_be_string())
    }

    fn collect_str<T: ?Sized + std::fmt::Display>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

fn finite_number(value: f64, non_finite_kind: NonFiniteKind) -> Result<Value, CanonicalValueError> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or(CanonicalValueError::NonFinite(non_finite_kind))
}

fn key_must_be_string() -> CanonicalValueError {
    CanonicalValueError::Custom("JSON object key must serialize as a string".to_string())
}
