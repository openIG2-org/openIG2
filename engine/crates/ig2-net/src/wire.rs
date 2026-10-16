//! Our compact serde encoding (ADR 0007 §4): snapshots, state hashes and lockstep messages.
//!
//! Little-endian fixed-width integers (`usize` as `u64`), floats as their raw bits (NaN payloads and −0.0
//! survive, so a restored state hashes the same), `bool` and `Option` tags as one byte, `u32` lengths before
//! strings, byte strings, sequences and maps, `u32` variant indices; structs and tuples are their fields in
//! order. Not self-describing: the reader must know the type.

use serde::de::{self, DeserializeSeed, IntoDeserializer, Visitor};
use serde::{ser, Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Custom(String),
    #[error("unexpected end of input")]
    Eof,
    #[error("invalid {0}")]
    Invalid(&'static str),
    #[error("{0} trailing bytes")]
    Trailing(usize),
    #[error("not supported by the wire format: {0}")]
    Unsupported(&'static str),
}

impl ser::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

/// Where the encoder's bytes go: a buffer, or a hasher.
pub trait Sink {
    fn put(&mut self, bytes: &[u8]);
}

impl Sink for Vec<u8> {
    fn put(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl Sink for sha2::Sha256 {
    fn put(&mut self, bytes: &[u8]) {
        sha2::Digest::update(self, bytes);
    }
}

/// `value`'s encoding.
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    to_sink(value, &mut out)?;
    Ok(out)
}

pub fn to_sink<T: Serialize + ?Sized, S: Sink>(value: &T, sink: &mut S) -> Result<(), Error> {
    value.serialize(&mut Encoder { sink })
}

/// The value encoded in all of `bytes`.
pub fn from_slice<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, Error> {
    let mut d = Decoder { input: bytes };
    let v = T::deserialize(&mut d)?;
    match d.input.len() {
        0 => Ok(v),
        n => Err(Error::Trailing(n)),
    }
}

/// A byte string encoded in one piece (`serialize_bytes`) rather than byte by byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bytes<'a>(pub &'a [u8]);

impl Serialize for Bytes<'_> {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(self.0)
    }
}

/// The owned side of [`Bytes`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ByteBuf(pub Vec<u8>);

impl Serialize for ByteBuf {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for ByteBuf {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = ByteBuf;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a byte string")
            }
            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<ByteBuf, E> {
                Ok(ByteBuf(v.to_vec()))
            }
            fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<ByteBuf, E> {
                Ok(ByteBuf(v))
            }
        }
        d.deserialize_byte_buf(V)
    }
}

// ------------------------------------------------------------------------------------------------ encoder

pub struct Encoder<'a, S: Sink> {
    sink: &'a mut S,
}

impl<S: Sink> Encoder<'_, S> {
    fn len(&mut self, n: usize) -> Result<(), Error> {
        let n = u32::try_from(n).map_err(|_| Error::Invalid("length"))?;
        self.sink.put(&n.to_le_bytes());
        Ok(())
    }
}

macro_rules! put_le {
    ($($f:ident: $t:ty),*) => {$(
        fn $f(self, v: $t) -> Result<(), Error> {
            self.sink.put(&v.to_le_bytes());
            Ok(())
        }
    )*};
}

impl<S: Sink> ser::Serializer for &mut Encoder<'_, S> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    put_le!(serialize_i8: i8, serialize_i16: i16, serialize_i32: i32, serialize_i64: i64, serialize_u8: u8, serialize_u16: u16,
        serialize_u32: u32, serialize_u64: u64, serialize_i128: i128, serialize_u128: u128);

    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.sink.put(&[v as u8]);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.sink.put(&v.to_bits().to_le_bytes());
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.sink.put(&v.to_bits().to_le_bytes());
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.sink.put(&(v as u32).to_le_bytes());
        Ok(())
    }
    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.serialize_bytes(v.as_bytes())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.len(v.len())?;
        self.sink.put(v);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Error> {
        self.sink.put(&[0]);
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<(), Error> {
        self.sink.put(&[1]);
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_unit_variant(self, _: &'static str, i: u32, _: &'static str) -> Result<(), Error> {
        self.serialize_u32(i)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, v: &T) -> Result<(), Error> {
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(self, _: &'static str, i: u32, _: &'static str, v: &T) -> Result<(), Error> {
        self.serialize_u32(i)?;
        v.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self, Error> {
        self.len(len.ok_or(Error::Unsupported("sequence without a length"))?)?;
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self, Error> {
        Ok(self)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        Ok(self)
    }
    fn serialize_tuple_variant(self, _: &'static str, i: u32, _: &'static str, _: usize) -> Result<Self, Error> {
        self.serialize_u32(i)?;
        Ok(self)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self, Error> {
        self.len(len.ok_or(Error::Unsupported("map without a length"))?)?;
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        Ok(self)
    }
    fn serialize_struct_variant(self, _: &'static str, i: u32, _: &'static str, _: usize) -> Result<Self, Error> {
        self.serialize_u32(i)?;
        Ok(self)
    }
    fn is_human_readable(&self) -> bool {
        false
    }
}

macro_rules! compound {
    ($($tr:ident :: $f:ident),*) => {$(
        impl<S: Sink> ser::$tr for &mut Encoder<'_, S> {
            type Ok = ();
            type Error = Error;
            fn $f<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
                v.serialize(&mut **self)
            }
            fn end(self) -> Result<(), Error> {
                Ok(())
            }
        }
    )*};
}
compound!(SerializeSeq::serialize_element, SerializeTuple::serialize_element, SerializeTupleStruct::serialize_field,
    SerializeTupleVariant::serialize_field);

impl<S: Sink> ser::SerializeMap for &mut Encoder<'_, S> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, k: &T) -> Result<(), Error> {
        k.serialize(&mut **self)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Error> {
        v.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<S: Sink> ser::SerializeStruct for &mut Encoder<'_, S> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, v: &T) -> Result<(), Error> {
        v.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<S: Sink> ser::SerializeStructVariant for &mut Encoder<'_, S> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, v: &T) -> Result<(), Error> {
        v.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------ decoder

pub struct Decoder<'de> {
    input: &'de [u8],
}

impl<'de> Decoder<'de> {
    fn take(&mut self, n: usize) -> Result<&'de [u8], Error> {
        if self.input.len() < n {
            return Err(Error::Eof);
        }
        let (a, b) = self.input.split_at(n);
        self.input = b;
        Ok(a)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn len(&mut self) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        // Every element takes at least a byte in practice; refuse lengths the input can't hold (no huge allocations).
        if n > self.input.len() {
            return Err(Error::Invalid("length"));
        }
        Ok(n)
    }
    fn tag(&mut self, what: &'static str) -> Result<bool, Error> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Invalid(what)),
        }
    }
}

macro_rules! get_le {
    ($($f:ident: $t:ty => $visit:ident),*) => {$(
        fn $f<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.$visit(<$t>::from_le_bytes(self.array()?))
        }
    )*};
}

impl<'de> de::Deserializer<'de> for &mut Decoder<'de> {
    type Error = Error;

    get_le!(deserialize_i8: i8 => visit_i8, deserialize_i16: i16 => visit_i16, deserialize_i32: i32 => visit_i32,
        deserialize_i64: i64 => visit_i64, deserialize_i128: i128 => visit_i128, deserialize_u8: u8 => visit_u8,
        deserialize_u16: u16 => visit_u16, deserialize_u32: u32 => visit_u32, deserialize_u64: u64 => visit_u64,
        deserialize_u128: u128 => visit_u128);

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Error> {
        Err(Error::Unsupported("self-describing types"))
    }
    fn deserialize_bool<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        v.visit_bool(self.tag("bool")?)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        v.visit_f32(f32::from_bits(u32::from_le_bytes(self.array()?)))
    }
    fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        v.visit_f64(f64::from_bits(u64::from_le_bytes(self.array()?)))
    }
    fn deserialize_char<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        v.visit_char(char::from_u32(self.u32()?).ok_or(Error::Invalid("char"))?)
    }
    fn deserialize_str<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        let n = self.len()?;
        v.visit_borrowed_str(std::str::from_utf8(self.take(n)?).map_err(|_| Error::Invalid("UTF-8"))?)
    }
    fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        self.deserialize_str(v)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        let n = self.len()?;
        v.visit_borrowed_bytes(self.take(n)?)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        self.deserialize_bytes(v)
    }
    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        if self.tag("option")? { v.visit_some(self) } else { v.visit_none() }
    }
    fn deserialize_unit<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        v.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, _: &'static str, v: V) -> Result<V::Value, Error> {
        v.visit_unit()
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _: &'static str, v: V) -> Result<V::Value, Error> {
        v.visit_newtype_struct(self)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        let n = self.len()?;
        v.visit_seq(Counted { d: self, left: n })
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, n: usize, v: V) -> Result<V::Value, Error> {
        v.visit_seq(Counted { d: self, left: n })
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(self, _: &'static str, n: usize, v: V) -> Result<V::Value, Error> {
        v.visit_seq(Counted { d: self, left: n })
    }
    fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        let n = self.len()?;
        v.visit_map(Counted { d: self, left: n })
    }
    fn deserialize_struct<V: Visitor<'de>>(self, _: &'static str, fields: &'static [&'static str], v: V) -> Result<V::Value, Error> {
        v.visit_seq(Counted { d: self, left: fields.len() })
    }
    fn deserialize_enum<V: Visitor<'de>>(self, _: &'static str, _: &'static [&'static str], v: V) -> Result<V::Value, Error> {
        v.visit_enum(self)
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        v.visit_u32(self.u32()?)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Error> {
        Err(Error::Unsupported("skipping values"))
    }
    fn is_human_readable(&self) -> bool {
        false
    }
}

struct Counted<'a, 'de> {
    d: &'a mut Decoder<'de>,
    left: usize,
}

impl<'de> de::SeqAccess<'de> for Counted<'_, 'de> {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>, Error> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(&mut *self.d).map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.left)
    }
}

impl<'de> de::MapAccess<'de> for Counted<'_, 'de> {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, Error> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(&mut *self.d).map(Some)
    }
    fn next_value_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(&mut *self.d)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.left)
    }
}

impl<'de> de::EnumAccess<'de> for &mut Decoder<'de> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<(T::Value, Self), Error> {
        let i = self.u32()?;
        let v = seed.deserialize(i.into_deserializer())?;
        Ok((v, self))
    }
}

impl<'de> de::VariantAccess<'de> for &mut Decoder<'de> {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(self)
    }
    fn tuple_variant<V: Visitor<'de>>(self, n: usize, v: V) -> Result<V::Value, Error> {
        v.visit_seq(Counted { d: self, left: n })
    }
    fn struct_variant<V: Visitor<'de>>(self, fields: &'static [&'static str], v: V) -> Result<V::Value, Error> {
        v.visit_seq(Counted { d: self, left: fields.len() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum E {
        A,
        B(i16),
        C { x: f32, y: Option<u8> },
        D(u8, String),
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S {
        a: u64,
        b: Vec<E>,
        c: BTreeMap<usize, (bool, i8)>,
        d: [f64; 3],
        e: ByteBuf,
        f: char,
    }

    #[test]
    fn round_trips_exactly() {
        let nan = f32::from_bits(0x7fc0_1234);
        let s = S {
            a: u64::MAX - 5,
            b: vec![E::A, E::B(-3), E::C { x: nan, y: Some(7) }, E::C { x: -0.0, y: None }, E::D(9, "Ő".into())],
            c: [(1, (true, -1)), (usize::MAX, (false, 4))].into(),
            d: [1.5, f64::NEG_INFINITY, f64::MIN_POSITIVE],
            e: ByteBuf(vec![1, 2, 3]),
            f: 'é',
        };
        let b = to_vec(&s).unwrap();
        let t: S = from_slice(&b).unwrap();
        assert_eq!(to_vec(&t).unwrap(), b);
        let E::C { x, .. } = t.b[2] else { panic!() };
        assert_eq!(x.to_bits(), nan.to_bits());
        let E::C { x, .. } = t.b[3] else { panic!() };
        assert_eq!(x.to_bits(), (-0.0f32).to_bits());
        assert_eq!(&b[..8], &(u64::MAX - 5).to_le_bytes());
    }

    #[test]
    fn rejects_bad_input() {
        let b = to_vec(&vec![1u32, 2]).unwrap();
        assert_eq!(from_slice::<Vec<u32>>(&b[..b.len() - 1]), Err(Error::Eof));
        assert_eq!(from_slice::<u8>(&[1, 2]), Err(Error::Trailing(1)));
        assert_eq!(from_slice::<bool>(&[2]), Err(Error::Invalid("bool")));
        assert_eq!(from_slice::<Vec<u8>>(&[0xff, 0xff, 0xff, 0x7f]), Err(Error::Invalid("length")));
        assert!(from_slice::<E>(&9u32.to_le_bytes()).is_err());
    }
}
