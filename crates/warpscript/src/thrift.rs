// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Thrift structures `GTSWrapper` and `Metadata` that `WRAP` / `UNWRAP` serialize, with
//! the compact protocol of libthrift 0.17 (`TCompactProtocol` over a `TMemoryInputTransport`)
//! that Warp 10 uses through `TSerializer` / `TDeserializer`.
//!
//! Decoding reproduces the reader faithfully, malformed input included: the unchecked "fast"
//! varint path that runs off the end of the array, the missing negative length check of
//! binary fields, the `MaxMessageSize` accounting of containers, the skipping of unknown
//! fields and of fields of an unexpected type. A [`ThriftError`] is either a malformed payload
//! (what `TDeserializer` reports as a `TException`, which callers wrap as the cause of their own
//! message) or a defect of the reader that escapes unwrapped (see
//! [`ThriftError::is_malformed_input`]).

use indexmap::IndexMap;

use crate::functions::strings::decode_utf8_java;
use crate::gts::Metadata;

/// A failure of the compact protocol reader.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThriftError {
    /// The payload ends before the value being read does.
    #[error(
        "Cannot read. Remote side has closed. Tried to read {wanted} bytes, but only got {got} \
         bytes. (This is often indicative of an internal error on the server side. Please check \
         your server logs.)"
    )]
    EndOfInput {
        /// The number of bytes the value needs.
        wanted: usize,
        /// The number of bytes left.
        got: usize,
    },
    /// A container or binary larger than what is left of the message size budget.
    #[error("MaxMessageSize reached")]
    MessageTooLarge,
    /// A container, string or binary with a negative length.
    #[error("Negative length: {0}")]
    NegativeLength(i32),
    /// A value of a type `TProtocolUtil.skip` cannot skip.
    #[error("Unrecognized type {0}")]
    UnrecognizedType(u8),
    /// A compact type nibble that is not a type.
    #[error("don't know what type: {0}")]
    UnknownCompactType(u8),
    /// The unchecked varint fast path read past the end of the payload.
    #[error("Index {index} out of bounds for length {length}")]
    VarintOverrun {
        /// The offset read.
        index: usize,
        /// The length of the payload.
        length: usize,
    },
    /// The slice of a binary with a negative length (it has no length check of its own).
    #[error("IndexOutOfBoundsException")]
    NegativeBinaryLength,
    /// Skipped values nested deeper than the stack of the reader allows.
    #[error("StackOverflowError")]
    SkipTooDeep,
}

impl ThriftError {
    /// `true` when the payload is malformed (a `TException`), `false` for the defects of the
    /// reader that Warp 10 lets escape as runtime failures. Callers wrap the former as the
    /// cause of their own message and let the latter through.
    pub fn is_malformed_input(&self) -> bool {
        match self {
            ThriftError::EndOfInput { .. }
            | ThriftError::MessageTooLarge
            | ThriftError::NegativeLength(_)
            | ThriftError::UnrecognizedType(_)
            | ThriftError::UnknownCompactType(_) => true,
            ThriftError::VarintOverrun { .. }
            | ThriftError::NegativeBinaryLength
            | ThriftError::SkipTooDeep => false,
        }
    }
}

type Result<T> = std::result::Result<T, ThriftError>;

/// `TType.STOP`.
const STOP: u8 = 0;
/// `TType.BOOL`.
const BOOL: u8 = 2;
/// `TType.BYTE`.
const BYTE: u8 = 3;
/// `TType.DOUBLE`.
const DOUBLE: u8 = 4;
/// `TType.I16`.
const I16: u8 = 6;
/// `TType.I32`.
const I32: u8 = 8;
/// `TType.I64`.
const I64: u8 = 10;
/// `TType.STRING`.
const STRING: u8 = 11;
/// `TType.STRUCT`.
const STRUCT: u8 = 12;
/// `TType.MAP`.
const MAP: u8 = 13;
/// `TType.SET`.
const SET: u8 = 14;
/// `TType.LIST`.
const LIST: u8 = 15;

/// Nesting depth at which skipping unknown fields gives up. Java recurses without limit
/// until its thread stack overflows (a `StackOverflowError` after thousands of levels); the
/// port bounds the recursion instead so that a hostile payload cannot exhaust its stack.
const MAX_SKIP_DEPTH: usize = 512;

/// The `GTSWrapper` Thrift structure: an encoded series with its metadata and bucketization.
/// Every field is optional; `None` is "not set".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GtsWrapper {
    /// Field 1, the metadata.
    pub metadata: Option<Metadata>,
    /// Field 2.
    pub lastbucket: Option<i64>,
    /// Field 3.
    pub bucketspan: Option<i64>,
    /// Field 4.
    pub bucketcount: Option<i64>,
    // Field 5, the AES wrapping key of the content, is never set from WarpScript: it is
    // skipped like an unknown field (which consumes the same bytes as reading it).
    /// Field 6, the base timestamp of the encoder.
    pub base: Option<i64>,
    /// Field 7, the content of the encoder, possibly gzipped.
    pub encoded: Option<Vec<u8>>,
    /// Field 8, the number of datapoints.
    pub count: Option<i64>,
    /// Field 9, whether `encoded` is gzipped.
    pub compressed: Option<bool>,
    /// Field 10, how many times `encoded` was gzipped (1 when unset).
    pub compression_passes: Option<i32>,
}

impl GtsWrapper {
    /// `getBase()`.
    pub fn base(&self) -> i64 {
        self.base.unwrap_or_default()
    }

    /// `getCount()`.
    pub fn count(&self) -> i64 {
        self.count.unwrap_or_default()
    }

    /// `isCompressed()`.
    pub fn is_compressed(&self) -> bool {
        self.compressed.unwrap_or_default()
    }

    /// `getCompressionPasses()`.
    pub fn compression_passes(&self) -> i32 {
        self.compression_passes.unwrap_or(1)
    }
}

// -----------------------------------------------------------------------------------------
// Reading
// -----------------------------------------------------------------------------------------

/// `TCompactProtocol.getTType`: the `TType` of a compact type nibble.
fn ttype(compact: u8) -> Result<u8> {
    Ok(match compact & 0x0f {
        0 => STOP,
        1 | 2 => BOOL,
        3 => BYTE,
        4 => I16,
        5 => I32,
        6 => I64,
        7 => DOUBLE,
        8 => STRING,
        9 => LIST,
        10 => SET,
        11 => MAP,
        12 => STRUCT,
        other => return Err(ThriftError::UnknownCompactType(other)),
    })
}

/// `TCompactProtocol.getMinSerializedSize` of the types `getTType` can return.
fn min_size(ttype: u8) -> i64 {
    match ttype {
        STOP | STRUCT => 0,
        DOUBLE => 8,
        _ => 1,
    }
}

/// `zigzagToInt` / `zigzagToLong`.
fn unzigzag(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

/// A `TCompactProtocol` reading a whole payload.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    /// `TEndpointTransport.remainingMessageSize`, only decreased by `readAll`.
    remaining_message: i64,
    last_field_id: i16,
    field_ids: Vec<i16>,
    /// The value of a boolean field, carried by its field header.
    bool_value: Option<bool>,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader {
            buf,
            pos: 0,
            remaining_message: i64::from(i32::MAX),
            last_field_id: 0,
            field_ids: Vec::new(),
            bool_value: None,
        }
    }

    fn available(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn read_byte(&mut self) -> Result<u8> {
        let byte = *self
            .buf
            .get(self.pos)
            .ok_or(ThriftError::EndOfInput { wanted: 1, got: 0 })?;
        self.pos += 1;
        Ok(byte)
    }

    /// The next `len` bytes, which the caller checked are available.
    fn take(&mut self, len: usize) -> &'a [u8] {
        let bytes = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        bytes
    }

    /// `TTransport.checkReadBytesAvailable`.
    fn check_read_bytes(&self, len: i64) -> Result<()> {
        if self.remaining_message < len {
            return Err(ThriftError::MessageTooLarge);
        }
        Ok(())
    }

    /// `readVarint32` (`bits` 32, `fast` 5) / `readVarint64` (`bits` 64, `fast` 10). With at
    /// least `fast` bytes left, Java reads straight from the array without bound checks and
    /// fails with an `ArrayIndexOutOfBoundsException` at its end; otherwise it reads byte by
    /// byte and fails like any short read. Shifts wrap like Java's.
    fn read_varint(&mut self, bits: u32, fast: usize) -> Result<u64> {
        let unchecked = self.available() >= fast;
        let mut result = 0u64;
        let mut shift = 0u32;
        loop {
            let Some(&byte) = self.buf.get(self.pos) else {
                return Err(if unchecked {
                    ThriftError::VarintOverrun {
                        index: self.pos,
                        length: self.buf.len(),
                    }
                } else {
                    ThriftError::EndOfInput { wanted: 1, got: 0 }
                });
            };
            self.pos += 1;
            result |= u64::from(byte & 0x7f) << (shift % bits);
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift = shift.wrapping_add(7);
        }
    }

    fn read_varint32(&mut self) -> Result<i32> {
        Ok(self.read_varint(32, 5)? as u32 as i32)
    }

    fn read_i32(&mut self) -> Result<i32> {
        Ok(unzigzag(u64::from(self.read_varint(32, 5)? as u32)) as i32)
    }

    fn read_i64(&mut self) -> Result<i64> {
        Ok(unzigzag(self.read_varint(64, 10)?))
    }

    fn read_bool(&mut self) -> Result<bool> {
        match self.bool_value.take() {
            Some(value) => Ok(value),
            None => Ok(self.read_byte()? == 1),
        }
    }

    /// `readDouble`, whose value no structure needs: it is only ever skipped.
    fn skip_double(&mut self) -> Result<()> {
        if self.available() < 8 {
            return Err(ThriftError::EndOfInput {
                wanted: 8,
                got: self.available(),
            });
        }
        self.take(8);
        self.remaining_message -= 8;
        Ok(())
    }

    /// `readBinary`: no negative length check, the slice of a negative length fails.
    fn read_binary(&mut self) -> Result<Vec<u8>> {
        let len = self.read_varint32()?;
        if len < 0 {
            return Err(ThriftError::NegativeBinaryLength);
        }
        self.read_bytes(len)
    }

    /// The `len` (non negative) bytes of a binary or string.
    fn read_bytes(&mut self, len: i32) -> Result<Vec<u8>> {
        self.check_read_bytes(i64::from(len))?;
        let len = len as usize;
        if self.available() < len {
            return Err(ThriftError::EndOfInput {
                wanted: len,
                got: self.available(),
            });
        }
        Ok(self.take(len).to_vec())
    }

    /// `checkContainerReadLength` / `checkStringReadLength`.
    fn check_length(len: i32) -> Result<i64> {
        if len < 0 {
            return Err(ThriftError::NegativeLength(len));
        }
        Ok(i64::from(len))
    }

    fn read_string(&mut self) -> Result<String> {
        let len = self.read_varint32()?;
        Self::check_length(len)?;
        Ok(decode_utf8_java(&self.read_bytes(len)?))
    }

    /// `readMapBegin`: key type, value type and size.
    fn read_map_begin(&mut self) -> Result<(u8, u8, i64)> {
        let size = Self::check_length(self.read_varint32()?)?;
        let types = if size == 0 { 0 } else { self.read_byte()? };
        let (key, value) = (ttype(types >> 4)?, ttype(types)?);
        self.check_read_bytes(size * (min_size(key) + min_size(value)))?;
        Ok((key, value, size))
    }

    /// `readListBegin` (and `readSetBegin`): element type and size.
    fn read_list_begin(&mut self) -> Result<(u8, i64)> {
        let size_and_type = self.read_byte()?;
        let mut size = i32::from(size_and_type >> 4);
        if size == 15 {
            size = self.read_varint32()?;
        }
        let size = Self::check_length(size)?;
        let element = ttype(size_and_type)?;
        self.check_read_bytes(size * min_size(element))?;
        Ok((element, size))
    }

    fn read_struct_begin(&mut self) {
        self.field_ids.push(self.last_field_id);
        self.last_field_id = 0;
    }

    fn read_struct_end(&mut self) {
        self.last_field_id = self.field_ids.pop().unwrap_or_default();
    }

    /// `readFieldBegin`: the type and id of the next field, `STOP` at the end of a struct.
    fn read_field_begin(&mut self) -> Result<(u8, i16)> {
        let header = self.read_byte()?;
        if header == STOP {
            return Ok((STOP, 0));
        }
        let delta = header >> 4;
        let id = if delta == 0 {
            self.read_i32()? as i16
        } else {
            self.last_field_id.wrapping_add(i16::from(delta))
        };
        let field_type = ttype(header)?;
        if field_type == BOOL {
            self.bool_value = Some(header & 0x0f == 1);
        }
        self.last_field_id = id;
        Ok((field_type, id))
    }

    /// `TProtocolUtil.skip`.
    fn skip(&mut self, field_type: u8, depth: usize) -> Result<()> {
        if depth > MAX_SKIP_DEPTH {
            return Err(ThriftError::SkipTooDeep);
        }
        match field_type {
            BOOL => {
                self.read_bool()?;
            }
            BYTE => {
                self.read_byte()?;
            }
            I16 | I32 => {
                self.read_varint(32, 5)?;
            }
            I64 => {
                self.read_varint(64, 10)?;
            }
            DOUBLE => self.skip_double()?,
            STRING => {
                self.read_binary()?;
            }
            STRUCT => {
                self.read_struct_begin();
                loop {
                    let (field_type, _) = self.read_field_begin()?;
                    if field_type == STOP {
                        break;
                    }
                    self.skip(field_type, depth + 1)?;
                }
                self.read_struct_end();
            }
            MAP => {
                let (key, value, size) = self.read_map_begin()?;
                for _ in 0..size {
                    self.skip(key, depth + 1)?;
                    self.skip(value, depth + 1)?;
                }
            }
            SET | LIST => {
                let (element, size) = self.read_list_begin()?;
                for _ in 0..size {
                    self.skip(element, depth + 1)?;
                }
            }
            other => return Err(ThriftError::UnrecognizedType(other)),
        }
        Ok(())
    }

    /// A `map<string,string>`: the generated code reads strings whatever the declared types.
    fn read_string_map(&mut self) -> Result<IndexMap<String, String>> {
        let (_, _, size) = self.read_map_begin()?;
        let mut map = IndexMap::new();
        for _ in 0..size {
            let key = self.read_string()?;
            let value = self.read_string()?;
            map.insert(key, value);
        }
        Ok(map)
    }

    fn read_metadata(&mut self) -> Result<Metadata> {
        let mut metadata = Metadata::default();
        self.read_struct_begin();
        loop {
            match self.read_field_begin()? {
                (STOP, _) => break,
                (STRING, 1) => metadata.name = Some(self.read_string()?),
                (MAP, 2) => metadata.labels = self.read_string_map()?,
                (I64, 3) => {
                    metadata.class_id = self.read_i64()?;
                    metadata.class_id_set = true;
                }
                (I64, 4) => {
                    metadata.labels_id = self.read_i64()?;
                    metadata.labels_id_set = true;
                }
                (MAP, 5) => metadata.attributes = self.read_string_map()?,
                (STRING, 6) => metadata.source = Some(self.read_string()?),
                (I64, 7) => {
                    metadata.last_activity = self.read_i64()?;
                    metadata.last_activity_set = true;
                }
                (field_type, _) => self.skip(field_type, 1)?,
            }
        }
        self.read_struct_end();
        Ok(metadata)
    }

    fn read_wrapper(&mut self) -> Result<GtsWrapper> {
        let mut wrapper = GtsWrapper::default();
        self.read_struct_begin();
        loop {
            match self.read_field_begin()? {
                (STOP, _) => break,
                (STRUCT, 1) => wrapper.metadata = Some(self.read_metadata()?),
                (I64, 2) => wrapper.lastbucket = Some(self.read_i64()?),
                (I64, 3) => wrapper.bucketspan = Some(self.read_i64()?),
                (I64, 4) => wrapper.bucketcount = Some(self.read_i64()?),

                (I64, 6) => wrapper.base = Some(self.read_i64()?),
                (STRING, 7) => wrapper.encoded = Some(self.read_binary()?),
                (I64, 8) => wrapper.count = Some(self.read_i64()?),
                // The value of a boolean field is in its header.
                (BOOL, 9) => wrapper.compressed = self.bool_value.take(),
                (I32, 10) => wrapper.compression_passes = Some(self.read_i32()?),
                (field_type, _) => self.skip(field_type, 0)?,
            }
        }
        self.read_struct_end();
        Ok(wrapper)
    }
}

/// `TDeserializer.deserialize` of a `GTSWrapper`. Trailing bytes are ignored.
pub fn deserialize_wrapper(bytes: &[u8]) -> Result<GtsWrapper> {
    Reader::new(bytes).read_wrapper()
}

// -----------------------------------------------------------------------------------------
// Writing
// -----------------------------------------------------------------------------------------

/// A `TCompactProtocol` writing a structure whose fields have increasing ids less than 16
/// apart, which is all `GTSWrapper` and `Metadata` need.
#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    last_field_id: u8,
}

impl Writer {
    fn varint(&mut self, mut n: u64) {
        while n >= 0x80 {
            self.out.push((n as u8 & 0x7f) | 0x80);
            n >>= 7;
        }
        self.out.push(n as u8);
    }

    fn field(&mut self, compact_type: u8, id: u8) {
        self.out
            .push(((id - self.last_field_id) << 4) | compact_type);
        self.last_field_id = id;
    }

    fn i64_field(&mut self, id: u8, value: i64) {
        self.field(6, id);
        self.varint(((value << 1) ^ (value >> 63)) as u64);
    }

    fn binary_field(&mut self, id: u8, bytes: &[u8]) {
        self.field(8, id);
        self.binary(bytes);
    }

    fn binary(&mut self, bytes: &[u8]) {
        self.varint(bytes.len() as u64);
        self.out.extend_from_slice(bytes);
    }

    fn map_field(&mut self, id: u8, map: &IndexMap<String, String>) {
        self.field(11, id);
        if map.is_empty() {
            self.out.push(0);
            return;
        }
        self.varint(map.len() as u64);
        self.out.push(0x88);
        for (key, value) in map {
            self.binary(key.as_bytes());
            self.binary(value.as_bytes());
        }
    }

    fn metadata(&mut self, metadata: &Metadata) {
        let outer = self.last_field_id;
        self.last_field_id = 0;
        if let Some(name) = &metadata.name {
            self.binary_field(1, name.as_bytes());
        }
        self.map_field(2, &metadata.labels);
        self.i64_field(3, metadata.class_id);
        self.i64_field(4, metadata.labels_id);
        self.map_field(5, &metadata.attributes);
        if let Some(source) = &metadata.source {
            self.binary_field(6, source.as_bytes());
        }
        if metadata.last_activity_set {
            self.i64_field(7, metadata.last_activity);
        }
        self.out.push(STOP);
        self.last_field_id = outer;
    }
}

/// `TSerializer.serialize` of a `GTSWrapper`.
pub fn serialize_wrapper(wrapper: &GtsWrapper) -> Vec<u8> {
    let mut writer = Writer::default();
    if let Some(metadata) = &wrapper.metadata {
        writer.field(12, 1);
        writer.metadata(metadata);
    }
    let longs = [
        (2, wrapper.lastbucket),
        (3, wrapper.bucketspan),
        (4, wrapper.bucketcount),
    ];
    for (id, value) in longs {
        if let Some(value) = value {
            writer.i64_field(id, value);
        }
    }

    if let Some(base) = wrapper.base {
        writer.i64_field(6, base);
    }
    if let Some(encoded) = &wrapper.encoded {
        writer.binary_field(7, encoded);
    }
    if let Some(count) = wrapper.count {
        writer.i64_field(8, count);
    }
    if let Some(compressed) = wrapper.compressed {
        writer.field(if compressed { 1 } else { 2 }, 9);
    }
    if let Some(passes) = wrapper.compression_passes {
        writer.field(5, 10);
        writer.varint(u64::from(((passes << 1) ^ (passes >> 31)) as u32));
    }
    writer.out.push(STOP);
    writer.out
}
