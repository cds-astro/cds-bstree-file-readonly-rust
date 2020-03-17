//! Library implementing a binary search tree stored in a file.
//!  
//! Entries are tuples made of one identifier plus one value.
//! Searches are performed on the values: they typically are values stored in a table column while 
//! identifiers are rows identifiers (e.g. simple row indices).
//! 
//! Node size should be such that it fits into L1 data cache.
//! Typical values (for eache core): 
//! * L1 cache: 32 kB
//! * L2 cache: 256 kB
//! * L3 cache: 1 MB (6 MB shared between 4 cores)
//! * HDD cache: 8 MB
//! I.e: L2 = 8 * L1; L3 = 4 * L2 = 32 * L1
//! I.e: HDD = 256 * L1
//! 
//! In DBMS, the page size is <= L1 cache size
//! 
//! If designed for HDD, we want to avoid random access (5ms seek time):
//! * we thus prefer to load large parts of the file in RAM
//!     - we favor a single root node (kept in cache), and an array of leaf nodes
//! * each node stored on the disk must be devided into sub-node no larger than the L1 cache capacity 

// L1Node<E> = simple &[E] of size nL1 such that size_of<E> * nL1 < 90% of L1 cache size
// (nL1InL3 - 1) * size_of<E> (1 + nL1) < L3 cache size
// L3Node = &[E] of size (nL1InL3 - 1) + nL1InL3 * L1Node<E>
// HDDNode = &[E] of size (nL3InHDDN - 1) + nL3InHDDN * L3Node

// We recall that: 2^0 + 2^1 + 2^2 + ... + 2^n = 2^(n+1) - 1 = size of a sub-tree

use serde::{Serialize, Deserialize};

use std::fmt::Debug;
use std::str::FromStr;
use std::marker::PhantomData;
use std::cmp::Ordering::{self, Greater, Equal, Less};
use std::io::{Error, ErrorKind, Cursor, Read, Write};

pub mod cliargs;
pub mod float;
pub mod rw;
pub mod mk;
pub mod visitors;
pub mod bstree;

use rw::*;
use float::FiniteFloat;

pub trait FromU64: Sized {
  fn from_u64(s: u64) -> Self;
}

impl FromU64 for u32 {
  fn from_u64(s: u64) -> Self {
    s as u32
  }
}

impl FromU64 for u64 {
  fn from_u64(s: u64) -> Self {
    s
  }
}

impl FromU64 for String {
  fn from_u64(s: u64) -> Self {
    format!("{}", &s)
  }
}

/// Trait defining the minimum requirements to be an identifier
/// * `FromU64` is used to be able to generate the identifier from a line number
pub trait Id: FromStr + FromU64 + Debug {}
impl<T> Id for T where T: FromStr + FromU64 + Debug{}

/// Trait defining the minimum requirements to be a value
pub trait Val: FromStr + Ord + Debug {}
impl<T> Val for T where T: FromStr + Ord + Debug {}



#[derive(Debug)]
pub enum IdInMemType {
  U32,
  U64,
  Str{n_chars: usize},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum IdType {
  U24, //(U24RW),
  U32,
  U40,
  U48,
  U56,
  U64,
  Str{n_chars: usize},
  Custom, // To be written into the file, but need a specific code
}

impl IdType {
  
  pub fn is_recno_compatible(&self) -> bool {
    match self {
      IdType::U24 | 
      IdType::U32 | 
      IdType::U40 | 
      IdType::U48 | 
      IdType::U56 => true,
      _ => false,
    }
  }
  
  pub fn byte_size(&self) -> usize {
    match self {
      IdType::U24 => 3,
      IdType::U32 => 4,
      IdType::U40 => 5,
      IdType::U48 => 6,
      IdType::U56 => 7,
      IdType::U64 => 8,
      IdType::Str{n_chars} => *n_chars,
      IdType::Custom => panic!("Can't be used with Id type Custom"),
    }
  }
  
  pub fn in_mem_type(&self) -> IdInMemType {
    match self {
      IdType::U24 |
      IdType::U32 => IdInMemType::U32,
      IdType::U40 |
      IdType::U48 |
      IdType::U56 |
      IdType::U64 => IdInMemType::U64,
      IdType::Str{n_chars} => IdInMemType::Str{n_chars: *n_chars},
      IdType::Custom => panic!("Can't be used with Id type Custom"),
    }
  }
  
}

/*impl FromU64 for IdType {
  type Err = String;

  fn from_(id_type: &str) -> Result<Self, Self::Err> {
  
}*/

impl FromStr for IdType {
  
  type Err = String;

  /// Get an identifier type from a String
  fn from_str(id_type: &str) -> Result<Self, Self::Err> {
    let c: char = id_type[0..1].parse().unwrap();
    let n_bytes: usize = id_type[1..].parse().unwrap();
    match (c, n_bytes) {
      ('u', 3) => Ok(IdType::U24),
      ('u', 4) => Ok(IdType::U32),
      ('u', 5) => Ok(IdType::U40),
      ('u', 6) => Ok(IdType::U48),
      ('u', 7) => Ok(IdType::U56),
      ('u', 8) => Ok(IdType::U64),
      ('t', nb) => Ok(IdType::Str{n_chars: nb}),
       _ => Err(format!("Could not parse id type: '{}'. Must match 'u[3-8]' or 't[0-9]+'.", &id_type)),
    }
  }
}

#[derive(Debug)]
pub enum ValInMemType {
  U32,
  U64,
  I32,
  I64,
  F32,
  F64,
  Str{n_chars: usize},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValType {
  U24,
  U32,
  U40,
  U48,
  U56,
  U64,
  I24,
  I32,
  I40,
  I48,
  I56,
  I64,
  F32,
  F64,
  Str{n_chars: usize},
  Custom, // Handled externally
}

impl ValType {
  
  pub fn byte_size(&self) -> usize {
    match self {
      ValType::U24 | ValType::I24 => 3,
      ValType::U32 | ValType::I32 | ValType::F32 => 4,
      ValType::U40 | ValType::I40 => 5,
      ValType::U48 | ValType::I48 => 6,
      ValType::U56 | ValType::I56 => 7,
      ValType::U64 | ValType::I64 | ValType::F64 => 8,
      ValType::Str{n_chars} => *n_chars,
      ValType::Custom => panic!("Can't be used with Id type Custom"),
    }
  }

  pub fn in_mem_type(&self) -> ValInMemType {
    match self {
      ValType::U24 |
      ValType::U32 => ValInMemType::U32,
      ValType::U40 |
      ValType::U48 |
      ValType::U56 |
      ValType::U64 => ValInMemType::U64,
      ValType::I24 |
      ValType::I32 => ValInMemType::I32,
      ValType::I40 |
      ValType::I48 |
      ValType::I56 |
      ValType::I64 => ValInMemType::I64,
      ValType::F32 => ValInMemType::F32,
      ValType::F64 => ValInMemType::F64,
      ValType::Str{n_chars} => ValInMemType::Str{n_chars: *n_chars},
      ValType::Custom => panic!("Can't be used with Id type Custom"),
    }
  }
}

impl FromStr for ValType {

  type Err = String;

  /// Get a value type from a String 
  fn from_str(val_type: &str) -> Result<Self, Self::Err> {
    let err = || format!("Could not parse id type: '{}'. Must match 'u[3-8]', 'i[3-8]', 'f[48]' or 't[0-9]+'.", &val_type);
    let c: char = val_type[0..1].parse().map_err(|_| err())?;
    let n_bytes: usize = val_type[1..].parse().map_err(|_| err())?;
    match (c, n_bytes) {
      ('u', 3) => Ok(ValType::U24),
      ('u', 4) => Ok(ValType::U32),
      ('u', 5) => Ok(ValType::U40),
      ('u', 6) => Ok(ValType::U48),
      ('u', 7) => Ok(ValType::U56),
      ('u', 8) => Ok(ValType::U64),
      ('i', 3) => Ok(ValType::I24),
      ('i', 4) => Ok(ValType::I32),
      ('i', 5) => Ok(ValType::I40),
      ('i', 6) => Ok(ValType::I48),
      ('i', 7) => Ok(ValType::I56),
      ('i', 8) => Ok(ValType::I64),
      ('f', 4) => Ok(ValType::F32),
      ('f', 8) => Ok(ValType::F64),
      ('t', nb) => Ok(ValType::Str{n_chars: nb}),
      _ => Err(err()),
    }
  }
}

/// Defines an action which has to read and/or write given identifier and value types.
/// It is made to be used with the `IdVal` type.
/// The reason behind is that `IdVal` will contains the giant `match` for all possible 
/// (IdType, ValType) tuples.And we want to write this `match` only once!
pub trait Process {
  type Output;
  
  fn exec<I, V, D, IRW, VRW>(self, types: &IdVal, id_rw: &IRW, val_rw: &VRW, dist: D) -> Result<Self::Output, std::io::Error>
    where I: Id,
          V: Val,
          D: Fn(&V, &V) -> V,
          IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V>;
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdVal(IdType, ValType);

//const STRING_DIST: impl Fn(&String, &String) -> String = |a: &String, b: String| panic!("No string distance!");

// pub struct StringDist {}

/*impl Fn(&String, &String) for StringDist {
  type Output = String;

  fn call(&self, args: (&String, &String)) -> Self::Output {
    panic!("No string distance!");
  }
}*/



impl IdVal {

  pub fn val_type(&self) -> &ValType {
    &self.1
  }

  pub fn exec<P>(&self, p: P) -> Result<P::Output, std::io::Error> // P::Output
    where P: Process {
    let ds = |a: &String, b: String| panic!("No string distance!");
    // Here we use static dispatch with monomorphization
    // - pro: one version of the code per possible tuple => very good performances!!
    // - con: one version of the code per possible tuple => compiled code may be large!!
    match (&self.0, &self.1) {
      // IdType U24, ValType: All
      (IdType::U24, ValType::U24) => p.exec(&self, &U24RW, &U24RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U24, ValType::U32) => p.exec(&self, &U24RW, &U32RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U24, ValType::U40) => p.exec(&self, &U24RW, &U40RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U24, ValType::U48) => p.exec(&self, &U24RW, &U48RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U24, ValType::U56) => p.exec(&self, &U24RW, &U56RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U24, ValType::U64) => p.exec(&self, &U24RW, &U64RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),

      (IdType::U24, ValType::I24) => p.exec(&self, &U24RW, &I24RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U24, ValType::I32) => p.exec(&self, &U24RW, &I32RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U24, ValType::I40) => p.exec(&self, &U24RW, &I40RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U24, ValType::I48) => p.exec(&self, &U24RW, &I48RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U24, ValType::I56) => p.exec(&self, &U24RW, &I56RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U24, ValType::I64) => p.exec(&self, &U24RW, &I64RW, |a: &i64, b: &i64| (a - b).abs()),

      (IdType::U24, ValType::F32) => p.exec(&self, &U24RW, &F32RW, |a: &FiniteFloat<f32>, b: &FiniteFloat<f32>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),
      (IdType::U24, ValType::F64) => p.exec(&self, &U24RW, &F64RW, |a: &FiniteFloat<f64>, b: &FiniteFloat<f64>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),

      (IdType::U24, ValType::Str{n_chars}) => p.exec(&self, &U24RW, &StrRW{n_bytes: *n_chars}, |a: &String, b: &String| panic!("Distance not implemented for Strings")),

      // IdType U32, ValType: All
      (IdType::U32, ValType::U24) => p.exec(&self, &U32RW, &U24RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U32, ValType::U32) => p.exec(&self, &U32RW, &U32RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U32, ValType::U40) => p.exec(&self, &U32RW, &U40RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U32, ValType::U48) => p.exec(&self, &U32RW, &U48RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U32, ValType::U56) => p.exec(&self, &U32RW, &U56RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U32, ValType::U64) => p.exec(&self, &U32RW, &U64RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      
      (IdType::U32, ValType::I24) => p.exec(&self, &U32RW, &I24RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U32, ValType::I32) => p.exec(&self, &U32RW, &I32RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U32, ValType::I40) => p.exec(&self, &U32RW, &I40RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U32, ValType::I48) => p.exec(&self, &U32RW, &I48RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U32, ValType::I56) => p.exec(&self, &U32RW, &I56RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U32, ValType::I64) => p.exec(&self, &U32RW, &I64RW, |a: &i64, b: &i64| (a - b).abs()),

      (IdType::U32, ValType::F32) => p.exec(&self, &U32RW, &F32RW, |a: &FiniteFloat<f32>, b: &FiniteFloat<f32>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),
      (IdType::U32, ValType::F64) => p.exec(&self, &U32RW, &F64RW, |a: &FiniteFloat<f64>, b: &FiniteFloat<f64>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),

      (IdType::U32, ValType::Str{n_chars}) => p.exec(&self, &U32RW, &StrRW{n_bytes: *n_chars}, |a: &String, b: &String| panic!("Distance not implemented for Strings")),

      // IdType U40, ValType: All
      (IdType::U40, ValType::U24) => p.exec(&self, &U40RW, &U24RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U40, ValType::U32) => p.exec(&self, &U40RW, &U32RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U40, ValType::U40) => p.exec(&self, &U40RW, &U40RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U40, ValType::U48) => p.exec(&self, &U40RW, &U48RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U40, ValType::U56) => p.exec(&self, &U40RW, &U56RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U40, ValType::U64) => p.exec(&self, &U40RW, &U64RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),

      (IdType::U40, ValType::I24) => p.exec(&self, &U40RW, &I24RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U40, ValType::I32) => p.exec(&self, &U40RW, &I32RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U40, ValType::I40) => p.exec(&self, &U40RW, &I40RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U40, ValType::I48) => p.exec(&self, &U40RW, &I48RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U40, ValType::I56) => p.exec(&self, &U40RW, &I56RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U40, ValType::I64) => p.exec(&self, &U40RW, &I64RW, |a: &i64, b: &i64| (a - b).abs()),

      (IdType::U40, ValType::F32) => p.exec(&self, &U40RW, &F32RW, |a: &FiniteFloat<f32>, b: &FiniteFloat<f32>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),
      (IdType::U40, ValType::F64) => p.exec(&self, &U40RW, &F64RW, |a: &FiniteFloat<f64>, b: &FiniteFloat<f64>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),

      (IdType::U40, ValType::Str{n_chars}) => p.exec(&self, &U40RW, &StrRW{n_bytes: *n_chars}, |a: &String, b: &String| panic!("Distance not implemented for Strings")),

      // IdType U48, ValType: All
      (IdType::U48, ValType::U24) => p.exec(&self, &U48RW, &U24RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U48, ValType::U32) => p.exec(&self, &U48RW, &U32RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U48, ValType::U40) => p.exec(&self, &U48RW, &U40RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U48, ValType::U48) => p.exec(&self, &U48RW, &U48RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U48, ValType::U56) => p.exec(&self, &U48RW, &U56RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U48, ValType::U64) => p.exec(&self, &U48RW, &U64RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),

      (IdType::U48, ValType::I24) => p.exec(&self, &U48RW, &I24RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U48, ValType::I32) => p.exec(&self, &U48RW, &I32RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U48, ValType::I40) => p.exec(&self, &U48RW, &I40RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U48, ValType::I48) => p.exec(&self, &U48RW, &I48RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U48, ValType::I56) => p.exec(&self, &U48RW, &I56RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U48, ValType::I64) => p.exec(&self, &U48RW, &I64RW, |a: &i64, b: &i64| (a - b).abs()),

      (IdType::U48, ValType::F32) => p.exec(&self, &U48RW, &F32RW, |a: &FiniteFloat<f32>, b: &FiniteFloat<f32>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),
      (IdType::U48, ValType::F64) => p.exec(&self, &U48RW, &F64RW, |a: &FiniteFloat<f64>, b: &FiniteFloat<f64>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),

      (IdType::U48, ValType::Str{n_chars}) => p.exec(&self, &U48RW, &StrRW{n_bytes: *n_chars}, |a: &String, b: &String| panic!("Distance not implemented for Strings")),
      
      // IdType U56, ValType: All
      (IdType::U56, ValType::U24) => p.exec(&self, &U56RW, &U24RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U56, ValType::U32) => p.exec(&self, &U56RW, &U32RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U56, ValType::U40) => p.exec(&self, &U56RW, &U40RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U56, ValType::U48) => p.exec(&self, &U56RW, &U48RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U56, ValType::U56) => p.exec(&self, &U56RW, &U56RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U56, ValType::U64) => p.exec(&self, &U56RW, &U64RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),

      (IdType::U56, ValType::I24) => p.exec(&self, &U56RW, &I24RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U56, ValType::I32) => p.exec(&self, &U56RW, &I32RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U56, ValType::I40) => p.exec(&self, &U56RW, &I40RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U56, ValType::I48) => p.exec(&self, &U56RW, &I48RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U56, ValType::I56) => p.exec(&self, &U56RW, &I56RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U56, ValType::I64) => p.exec(&self, &U56RW, &I64RW, |a: &i64, b: &i64| (a - b).abs()),

      (IdType::U56, ValType::F32) => p.exec(&self, &U56RW, &F32RW, |a: &FiniteFloat<f32>, b: &FiniteFloat<f32>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),
      (IdType::U56, ValType::F64) => p.exec(&self, &U56RW, &F64RW, |a: &FiniteFloat<f64>, b: &FiniteFloat<f64>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),

      (IdType::U56, ValType::Str{n_chars}) => p.exec(&self, &U56RW, &StrRW{n_bytes: *n_chars}, |a: &String, b: &String| panic!("Distance not implemented for Strings")),
      
      // IdType U64, ValType: All
      (IdType::U64, ValType::U24) => p.exec(&self, &U64RW, &U24RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U64, ValType::U32) => p.exec(&self, &U64RW, &U32RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U64, ValType::U40) => p.exec(&self, &U64RW, &U40RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U64, ValType::U48) => p.exec(&self, &U64RW, &U48RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U64, ValType::U56) => p.exec(&self, &U64RW, &U56RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::U64, ValType::U64) => p.exec(&self, &U64RW, &U64RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),

      (IdType::U64, ValType::I24) => p.exec(&self, &U64RW, &I24RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U64, ValType::I32) => p.exec(&self, &U64RW, &I32RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::U64, ValType::I40) => p.exec(&self, &U64RW, &I40RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U64, ValType::I48) => p.exec(&self, &U64RW, &I48RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U64, ValType::I56) => p.exec(&self, &U64RW, &I56RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::U64, ValType::I64) => p.exec(&self, &U64RW, &I64RW, |a: &i64, b: &i64| (a - b).abs()),

      (IdType::U64, ValType::F32) => p.exec(&self, &U64RW, &F32RW, |a: &FiniteFloat<f32>, b: &FiniteFloat<f32>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),
      (IdType::U64, ValType::F64) => p.exec(&self, &U64RW, &F64RW, |a: &FiniteFloat<f64>, b: &FiniteFloat<f64>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),

      (IdType::U64, ValType::Str{n_chars}) => p.exec(&self,&U64RW, &StrRW{n_bytes: *n_chars}, |a: &String, b: &String| panic!("Distance not implemented for Strings")),
      
      // IdType Str, ValType: All
      (IdType::Str{n_chars}, ValType::U24) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &U24RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::Str{n_chars}, ValType::U32) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &U32RW, |a: &u32, b: &u32| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::Str{n_chars}, ValType::U40) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &U40RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::Str{n_chars}, ValType::U48) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &U48RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::Str{n_chars}, ValType::U56) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &U56RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),
      (IdType::Str{n_chars}, ValType::U64) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &U64RW, |a: &u64, b: &u64| if  *a > *b { *a - *b } else { *b - *a }),

      (IdType::Str{n_chars}, ValType::I24) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &I24RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::Str{n_chars}, ValType::I32) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &I32RW, |a: &i32, b: &i32| (a - b).abs()),
      (IdType::Str{n_chars}, ValType::I40) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &I40RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::Str{n_chars}, ValType::I48) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &I48RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::Str{n_chars}, ValType::I56) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &I56RW, |a: &i64, b: &i64| (a - b).abs()),
      (IdType::Str{n_chars}, ValType::I64) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &I64RW, |a: &i64, b: &i64| (a - b).abs()),

      (IdType::Str{n_chars}, ValType::F32) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &F32RW, |a: &FiniteFloat<f32>, b: &FiniteFloat<f32>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),
      (IdType::Str{n_chars}, ValType::F64) => p.exec(&self, &StrRW{n_bytes: *n_chars}, &F64RW, |a: &FiniteFloat<f64>, b: &FiniteFloat<f64>| FiniteFloat::new((a.get() - b.get()).abs()).unwrap()),

      (IdType::Str{n_chars: n_chars_i}, ValType::Str{n_chars: n_chars_v}) => p.exec(&self, &StrRW{n_bytes: *n_chars_i}, &StrRW{n_bytes: *n_chars_v}, |a: &String, b: &String| panic!("Distance not implemented for Strings")),

      _ => Err(std::io::Error::new(ErrorKind::Other, "Case not supported yet!")),
    }
  }
  
  /*pub fn test(&self) {
    let mut buf = vec![0u8; 10];
    
    // self.exec(|id_rw, val_rw| (id_rw.read(&buf), val_rw.read(&buf)))
  }*/
}






pub struct EntryOpt<I, V> where V: Ord { // I: Sized, V: Sized + Ord {
  /// Row identifier
  id: I,
  /// Indexed value
  val: Option<V>,
}

pub struct Entry<I, V> where V: Ord { // I: Sized, V: Sized + Ord {
  /// Row identifier
  pub id: I,
  /// Indexed value
  pub val: V,
}

impl <I, V> Entry<I, V> where V: Ord {
  
  /*fn n_bytes<IRW, VRW>(&self, id_codec: &IRW, val_codec: &VRW) -> usize
  where IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V> {
    id_codec.n_bytes() + val_codec.n_bytes()
  }

  fn read_id<IRW, VRW>(&self, mut reader: &mut [u8], id_codec: &IRW, val_codec: &VRW) -> Result<I, std::io::Error>
    where IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    assert_eq!(reader.len(), self.n_bytes(id_codec, val_codec));
    id_codec.read(&mut reader)
  }

  fn read_val<IRW, VRW>(&self, reader: &mut [u8], id_codec: &IRW, val_codec: &VRW) -> Result<V, std::io::Error>
    where IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    assert_eq!(reader.len(), self.n_bytes(id_codec, val_codec));
    val_codec.read(&mut reader[id_codec.n_bytes()..])
  }*/
  
  fn read<R, IRW, VRW>(reader: &mut R, id_codec: &IRW, val_codec: &VRW) -> Result<Self, std::io::Error>
    where R: Read,
          IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    Ok(
      Entry {
        id: id_codec.read(reader)?,
        val: val_codec.read(reader)?,
      }
    )
  }
  
  fn write<W, IRW, VRW>(&self, writer: &mut W, id_codec: &IRW, val_codec: &VRW) -> Result<(), std::io::Error> 
    where W: Write,
          IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    id_codec.write(writer, &self.id)?;
    val_codec.write( writer, &self.val)
  }
}

impl <I, V> PartialOrd for Entry<I, V> where V: Ord {
  fn partial_cmp(&self, other: &Entry<I, V>) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl <I, V> Ord for Entry<I, V> where V: Ord {
  fn cmp(&self, other: &Entry<I, V>) -> Ordering {
    self.val.cmp(&other.val)
  }
}

impl <I, V> PartialEq for Entry<I, V> where V: Ord {
  fn eq(&self, other: &Entry<I, V>) -> bool {
    self.val == other.val
  }
}

impl <I, V> Eq for Entry<I, V> where V: Ord { }


pub struct EntryRaw<'a, I, V> 
  where V: PartialOrd {
  raw: &'a [u8],
  id : PhantomData<&'a I>,
  val: PhantomData<&'a V>,
}

impl <'a, I, V> EntryRaw<'a, I, V> 
  where V: PartialOrd {
  
  fn get_id<IRW, VRW>(&self, id_codec: &IRW, _val_codec: &VRW) -> Result<I, std::io::Error>
    where IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    id_codec.read(&mut Cursor::new(self.raw))
  }
  
  fn get_val<IRW, VRW>(&self, id_codec: &IRW, val_codec: &VRW) -> Result<V, std::io::Error>
    where IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    let mut cursor = Cursor::new(self.raw);
    cursor.set_position(id_codec.n_bytes() as u64);
    val_codec.read(&mut cursor)
  }
}

pub struct RawEntries<'a, I, V, IRW, VRW>
  where I: Id,
        V: Val,
        IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V> {
  raw: Cursor<&'a [u8]>,
  id_rw: &'a IRW,
  val_rw: &'a VRW,
  entry_byte_size: usize,
  n_entries: usize,
}

impl <'a, I, V, IRW, VRW> RawEntries<'a, I, V, IRW, VRW>
  where I: Id,
        V: Val,
        IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V> {
  
  pub fn new(raw: &'a [u8], id_rw: &'a IRW, val_rw: &'a VRW) 
    -> RawEntries<'a, I, V, IRW, VRW> {
    assert!(raw.len() > 0);
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    let n_entries = raw.len() / entry_byte_size;
    RawEntries {
      raw: Cursor::new(raw),
      id_rw,
      val_rw,
      entry_byte_size,
      n_entries
    }
  }
  
  pub fn n_entries(&self) -> usize {
    self.raw.get_ref().len() / self.entry_byte_size
  }

  // For better performances, have a look at raw pointers!!
  fn get_val(&mut self, index: usize) -> Result<V, std::io::Error> {
    self.raw.set_position((self.entry_byte_size * index + self.id_rw.n_bytes()) as u64);
    self.val_rw.read(&mut self.raw)
  }

  // For better performances, have a look at raw pointers!!
  fn get_entry(&mut self, index: usize) -> Result<Entry<I, V>, std::io::Error> {
    self.raw.set_position((self.entry_byte_size * index) as u64);
    Entry::read(&mut self.raw, self.id_rw, self.val_rw) 
  }
  
  pub fn binary_search(&mut self, val: &V) -> Result<Result<usize, usize>, std::io::Error>  {
    // Code taken from Rust slice binary_search:
    // https://doc.rust-lang.org/src/core/slice/mod.rs.html#1470-1474
    let mut size = self.n_entries();
    let mut base = 0_usize;
    while size > 1 {
      let half = size >> 1;
      let mid = base + half;
      // mid is always in [0, size), that means mid is >= 0 and < size.
      // mid >= 0: by definition
      // mid < size: mid = size / 2 + size / 4 + size / 8 ...
      let cmp = self.get_val(mid)?.cmp(val);
      base = if cmp == Greater { base } else { mid };
      size -= half;
    }
    // base is always in [0, size) because base <= mid.
    let cmp = self.get_val(base)?.cmp(val);
    Ok(if cmp == Equal { Ok(base) } else { Err(base + (cmp == Less) as usize) })
  }
  
}

// datastruct:
// - meta
// - null values block (only identifiers, sequentially ordered by `id`)
// - values blocks key,val pairs (ordered by `val` blocks)
