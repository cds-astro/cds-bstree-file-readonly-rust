//! This module contains the main code able to build and store in a file a bs-tree.

use std::{
  io::{Error, ErrorKind, Read},
  str::FromStr,
};

use csv::{Reader, StringRecord};
use itertools::Itertools;
use log::{debug, info, warn};

use crate::{
  bstree,
  cliargs::{
    colargs::ColIndices,
    memsize::MemSizeArgs,
    mkargs::{MkAlgoArgs, TmpDir},
  },
  rw::ReadWrite,
  Entry, EntryOpt, Id, IdVal, Process, Val,
};

// See cds.index.general.impl.bstree.BSTreeFile
// and
// /data/pineau/Eclipse/Documents/Communication/Conf/TechnoForumStras08052012/TechnoForum2012.pdf

/// Create an index from a CSV file.
pub struct MkIndex<R>
where
  R: Read,
{
  reader: Reader<R>,
  col_indices: ColIndices,
  supports_null: bool,
  args: MkAlgoArgs,
  mem_args: MemSizeArgs,
}

impl<R> MkIndex<R>
where
  R: Read,
{
  pub fn new<S: Read>(
    reader: Reader<S>,
    col_indices: ColIndices,
    supports_null: bool,
    args: MkAlgoArgs,
    mem_args: MemSizeArgs,
  ) -> MkIndex<S> {
    MkIndex {
      reader,
      col_indices,
      supports_null,
      args,
      mem_args,
    }
  }

  fn mk_no_null<I, V, IRW, VRW, P>(
    mut self,
    types: &IdVal,
    id_rw: &IRW,
    val_rw: &VRW,
    csv2entry: P,
  ) -> Result<<Self as Process>::Output, std::io::Error>
  //Self::Output
  where
    I: Id,                    // Identifier type (from u64 if recno option)
    V: Val,                   // Value type (that is comparable)
    IRW: ReadWrite<Type = I>, // Object able to read/write an identifier
    VRW: ReadWrite<Type = V>, // Object able to read/write a value
    P: Fn(usize, &StringRecord) -> Result<Entry<I, V>, Error>,
  {
    let to_io_err = From::from;
    let mut tmp_dir = self.args.get_tmp_dir()?;
    let mut count = 0_usize;
    // Create all tmp files
    for chunk in &self
      .reader
      .records()
      .enumerate()
      .chunks(self.args.chunk_size)
    {
      let mut entries: Vec<Entry<I, V>> = chunk
        .map(|(i, rec_res)| {
          rec_res
            .map_err(to_io_err)
            .and_then(|rec| csv2entry(i, &rec))
        })
        .collect::<Result<_, Error>>()?;
      entries.sort_unstable();
      count += entries.len();
      tmp_dir.write_tmp_file(id_rw, val_rw, entries)?;
      eprint!("\r\x1b[2K - n rows parsed and written: {}", &count);
    }
    info!("Reduce to max {} files be merge sort...", self.args.kway);
    tmp_dir = tmp_dir.reduce_to_k_files(id_rw, val_rw, self.args.kway)?;
    info!("Re-read all tmp files to build the final bstree file...");
    let sorted_entry_iter = tmp_dir.to_sorted_iter(id_rw, val_rw);
    #[cfg(not(target_arch = "wasm32"))]
    bstree::build(
      self.args.get_output(),
      &self.mem_args,
      count,
      sorted_entry_iter,
      types,
      id_rw,
      val_rw,
    )?;
    Ok(count)
  }

  fn mk_with_null<I, V, IRW, VRW, P>(
    self,
    _types: &IdVal,
    _id_rw: &IRW,
    _val_rw: &VRW,
    _csv2entry: P,
  ) -> Result<<Self as Process>::Output, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    P: Fn(usize, &StringRecord) -> Result<EntryOpt<I, V>, Error>,
  {
    todo!()
  }
}

impl<R: Read> Process for MkIndex<R> {
  type Output = usize;

  fn exec<I, V, D, IRW, VRW>(
    self,
    types: IdVal,
    id_rw: IRW,
    val_rw: VRW,
    _dist: D,
  ) -> Result<Self::Output, Error>
  where
    I: Id,
    V: Val,
    D: Fn(&V, &V) -> V,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    info!("Parse CSV and write tmp files...");
    let i_val = self.col_indices.val;
    if self.supports_null {
      match self.col_indices.id {
        None => self.mk_with_null(&types, &id_rw, &val_rw, |i, csv_row| {
          Ok(EntryOpt {
            id: I::from_u64(i as u64),
            val: get::<V>(csv_row, i_val, "value"),
          })
        }),
        Some(i_id) => self.mk_with_null(&types, &id_rw, &val_rw, |_, csv_row| {
          Ok(EntryOpt {
            id: get_with_err::<I>(csv_row, i_id, "id")?,
            val: get::<V>(csv_row, i_val, "value"),
          })
        }),
      }
    } else {
      match self.col_indices.id {
        None => self.mk_no_null(&types, &id_rw, &val_rw, |i, csv_row| {
          Ok(Entry {
            id: I::from_u64(i as u64),
            val: get_with_err::<V>(csv_row, i_val, "value")?,
          })
        }),
        Some(i_id) => self.mk_no_null(&types, &id_rw, &val_rw, |_, csv_row| {
          Ok(Entry {
            id: get_with_err::<I>(csv_row, i_id, "id")?,
            val: get_with_err::<V>(csv_row, i_val, "value")?,
          })
        }),
      }
    }
  }
}

fn get<F: FromStr>(record: &StringRecord, index: usize, col_name: &'static str) -> Option<F> {
  let res = record.get(index);
  match res {
    Some(str_ref) => {
      if str_ref.is_empty() {
        warn!("Empty col '{}' at {}!", col_name, get_position_str(record));
        None
      } else {
        match str_ref.parse::<F>() {
          Ok(val) => Some(val),
          Err(_) => {
            warn!(
              "WARNING: error parsing col '{}' value '{}' at {}, the value is set to NULL!",
              col_name,
              str_ref,
              get_position_str(record)
            );
            None
          }
        }
      }
    }
    None => {
      // unreachable if mode is not 'flexible'
      warn!(
        "No col '{}' at {}, the line is ignored!",
        col_name,
        get_position_str(record)
      );
      None
    }
  }
}

fn get_with_err<F: FromStr>(
  record: &StringRecord,
  index: usize,
  col_name: &'static str,
) -> Result<F, Error> {
  let res = record.get(index);
  match res {
    Some(str_ref) => {
      if str_ref.is_empty() {
        // Err(From::from(format!("Empty col '{}' at {}!", col_name, get_position_str(&record))))
        Err(Error::new(
          ErrorKind::Other,
          format!("Empty col '{}' at {}!", col_name, get_position_str(record)),
        ))
      } else {
        match str_ref.parse::<F>() {
          Ok(val) => Ok(val),
          Err(_) => Err(Error::new(
            ErrorKind::Other,
            format!(
              "Error parsing col '{}' value '{}' at {}!",
              col_name,
              str_ref,
              get_position_str(record)
            ),
          )),
        }
      }
    } // unreachable if mode is not 'flexible'
    None => Err(Error::new(
      ErrorKind::Other,
      format!("No col '{}' at {}!", col_name, get_position_str(record)),
    )),
  }
}

fn get_position_str(record: &StringRecord) -> String {
  match record.position() {
    Some(pos) => format!("{:?}", pos),
    None => String::from("(no position information available)"),
  }
}

/// Structure building a index element by element.
pub struct BSTreeFileBuilder<I, V, IRW, VRW>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
{
  /// Args
  args: MkAlgoArgs,
  /// Memory size args
  mem_args: MemSizeArgs,
  /// Tmp dir obj
  tmp_dir: TmpDir,
  /// Id and value types (to write in the file)
  types: IdVal,
  /// Writers
  id_rw: IRW,
  val_rw: VRW,
  /// Chunk
  entries: Vec<Entry<I, V>>,
  /// Number of elements in the tree
  count: usize,
}

impl<I, V, IRW, VRW> BSTreeFileBuilder<I, V, IRW, VRW>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
{
  pub fn new(
    args: MkAlgoArgs,
    mem_args: MemSizeArgs,
    types: IdVal,
    id_rw: IRW,
    val_rw: VRW,
  ) -> Result<Self, Error> {
    let chunk_size = args.chunk_size;
    args.get_tmp_dir().map(|tmp_dir| Self {
      args,
      mem_args,
      tmp_dir,
      types,
      id_rw,
      val_rw,
      entries: Vec::with_capacity(chunk_size),
      count: 0,
    })
  }

  /// We recall that the value is indexed (it is th key) and the identifier correspond, e.g., to
  /// a recno and is the information we want to retrieve from a query of the indexed values.
  pub fn append(&mut self, id: I, val: V) -> Result<(), Error> {
    if self.chunk_is_full() {
      self.sort_and_write_entries()?;
    }
    self.entries.push(Entry::new(id, val));
    Ok(())
  }

  fn chunk_is_full(&self) -> bool {
    self.entries.len() == self.args.chunk_size
  }

  fn sort_and_write_entries(&mut self) -> Result<(), Error> {
    let mut chunk = std::mem::replace(&mut self.entries, Vec::with_capacity(self.args.chunk_size));
    debug!("Sort chunk...");
    chunk.sort();
    info!(
      "Write chunk {}..{} in temporary file...",
      self.count,
      self.count + chunk.len()
    );
    self.count += chunk.len();
    self
      .tmp_dir
      .write_tmp_file(&self.id_rw, &self.val_rw, chunk)
  }

  /// Returns the number en entries in the BSTree.s
  pub fn build_index(mut self) -> Result<usize, Error> {
    // Write last elements in a temporray file
    if !self.entries.is_empty() {
      self.sort_and_write_entries()?;
    }
    info!("Reduce to max {} files be merge sort...", self.args.kway);
    let mut tmp_dir = self
      .tmp_dir
      .reduce_to_k_files(&self.id_rw, &self.val_rw, self.args.kway)?;
    info!("Re-read all tmp files to build the final bstree file...");
    let sorted_entry_iter = tmp_dir.to_sorted_iter(&self.id_rw, &self.val_rw);
    #[cfg(not(target_arch = "wasm32"))]
    bstree::build(
      self.args.get_output(),
      &self.mem_args,
      self.count,
      sorted_entry_iter,
      &self.types,
      &self.id_rw,
      &self.val_rw,
    )
    .map(|()| self.count)
  }
}
