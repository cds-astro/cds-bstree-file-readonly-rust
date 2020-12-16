use std::io::{
  Read, Write, 
  Error, ErrorKind
};
use byteorder::{
  LittleEndian, 
  ReadBytesExt, WriteBytesExt
};
use crate::float::FiniteFloat;

/// Trait used to read and write element of the associated type `Type`.
pub trait ReadWrite: Clone + Send {
  type Type;
  /*fn id_type(&self) -> IdType; // Used to be written in the file
  fn val_type(&self) -> ValType;*/
  /// Number of bytes redden or written
  fn n_bytes(&self) -> usize;
  /// Read an element of type `Type` from the given `Reader` 
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error>;
  /// Write an element of type `Type` to the given `Writer` 
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error>;
}

// Unsigned integers

#[derive(Clone)]
pub struct U24RW;

impl ReadWrite for U24RW {
  type Type = u32;
  fn n_bytes(&self) -> usize {
    3
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_u24::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_u24::<LittleEndian>(*val)
  }
}

#[derive(Clone)]
pub struct U32RW;

impl ReadWrite for U32RW {
  type Type = u32;
  fn n_bytes(&self) -> usize {
    4
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_u32::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_u32::<LittleEndian>(*val)
  }
}

#[derive(Clone)]
pub struct U40RW;

impl ReadWrite for U40RW {
  type Type = u64;
  fn n_bytes(&self) -> usize {
    5
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_uint::<LittleEndian>(5)
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_uint::<LittleEndian>(*val, 5)
  }
}

#[derive(Clone)]
pub struct U48RW;

impl ReadWrite for U48RW {
  type Type = u64;
  fn n_bytes(&self) -> usize {
    6
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_u48::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_u48::<LittleEndian>(*val)
  }
}

#[derive(Clone)]
pub struct U56RW;

impl ReadWrite for U56RW {
  type Type = u64;
  fn n_bytes(&self) -> usize {
    7
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_uint::<LittleEndian>(7)
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_uint::<LittleEndian>(*val, 7)
  }
}

#[derive(Clone)]
pub struct U64RW;

impl ReadWrite for U64RW {
  type Type = u64;
  fn n_bytes(&self) -> usize {
    8
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_u64::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_u64::<LittleEndian>(*val)
  }
}

// Signed integers

#[derive(Clone)]
pub struct I24RW;

impl ReadWrite for I24RW {
  type Type = i32;
  fn n_bytes(&self) -> usize {
    3
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_i24::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_i24::<LittleEndian>(*val)
  }
}

#[derive(Clone)]
pub struct I32RW;

impl ReadWrite for I32RW {
  type Type = i32;
  fn n_bytes(&self) -> usize {
    4
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_i32::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_i32::<LittleEndian>(*val)
  }
}

#[derive(Clone)]
pub struct I40RW;

impl ReadWrite for I40RW {
  type Type = i64;
  fn n_bytes(&self) -> usize {
    5
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_int::<LittleEndian>(5)
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_int::<LittleEndian>(*val, 5)
  }
}

#[derive(Clone)]
pub struct I48RW;

impl ReadWrite for I48RW {
  type Type = i64;
  fn n_bytes(&self) -> usize {
    6
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_i48::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_i48::<LittleEndian>(*val)
  }
}

#[derive(Clone)]
pub struct I56RW;

impl ReadWrite for I56RW {
  type Type = i64;
  fn n_bytes(&self) -> usize {
    7
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_int::<LittleEndian>(7)
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_int::<LittleEndian>(*val, 7)
  }
}

#[derive(Clone)]
pub struct I64RW;

impl ReadWrite for I64RW {
  type Type = i64;
  fn n_bytes(&self) -> usize {
    8
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    reader.read_i64::<LittleEndian>()
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_i64::<LittleEndian>(*val)
  }
}

// Float

#[derive(Clone)]
pub struct F32RW;

impl ReadWrite for F32RW {
  type Type = FiniteFloat<f32>;
  fn n_bytes(&self) -> usize {
    4
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    FiniteFloat::<f32>::new(reader.read_f32::<LittleEndian>()?)
      .ok_or(Error::new(ErrorKind::InvalidData, "Read a not finite f32!"))
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_f32::<LittleEndian>(val.get())
  }
}

#[derive(Clone)]
pub struct F64RW;

impl ReadWrite for F64RW {
  type Type = FiniteFloat<f64>;
  fn n_bytes(&self) -> usize {
    8
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    FiniteFloat::<f64>::new(reader.read_f64::<LittleEndian>()?)
      .ok_or(Error::new(ErrorKind::InvalidData, "Read a not finite f64!"))
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    writer.write_f64::<LittleEndian>(val.get())
  }
}

// String

#[derive(Clone)]
pub struct StrRW {
  pub n_bytes: usize
}

impl ReadWrite for StrRW {
  type Type = String;
  fn n_bytes(&self) -> usize {
    self.n_bytes
  }
  fn read<R: Read>(&self, reader: &mut R) -> Result<Self::Type, Error> {
    let mut buf = vec![0u8; self.n_bytes];
    reader.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| Error::new(ErrorKind::InvalidData, e))
  }
  fn write<W: Write>(&self, writer: &mut W, val: &Self::Type) -> Result<(), Error> {
    let buf = val.as_bytes();
    let l = buf.len();
    if l >= self.n_bytes {
      writer.write_all(&buf[0..self.n_bytes])
    }  else {
      writer.write_all(buf)?; // 0u8 = '\0' = null character
      writer.write_all(&vec![0u8; self.n_bytes - l]) 
    }
  }
}
