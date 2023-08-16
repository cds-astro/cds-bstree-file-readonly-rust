#[cfg(not(target_arch = "wasm32"))]
use memmap::{Mmap, MmapOptions};
use structopt::StructOpt;

use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::iter;
use std::path::{Path, PathBuf};

use crate::{
  bstree::{read_meta, BSTreeMeta, SubTreeR},
  rw::ReadWrite,
  visitors::*,
  Id, IdVal, Process, Val,
};

#[derive(Clone, Debug, StructOpt, serde::Serialize, serde::Deserialize)]
pub enum Mode {
  #[structopt(name = "info")]
  /// Returns tree metadata information
  Info,
  #[structopt(name = "get")]
  /// Returns the first entry having a value equal to the given value
  GetFirst {
    #[structopt(subcommand)]
    val_or_file: ValOrFile,
  },
  #[structopt(name = "all")]
  /// Returns all entries having a value equal to the given value
  All {
    #[structopt(short = "v", long)]
    value: String,
    #[structopt(short = "l", long)]
    /// Limits the number of entries in output
    limit: Option<usize>,
    #[structopt(short = "c", long)]
    /// Returns the size of the result instead of the result itself
    count: bool,
  },
  #[structopt(name = "nn")]
  /// Returns the entry having its the nearest value from the the given value
  Nn {
    #[structopt(subcommand)]
    val_or_file: ValOrFile,
    #[structopt(long)]
    d_max: Option<String>,
  },
  #[structopt(name = "knn")]
  /// Returns the k entries having the nearest value from the the given value
  Knn {
    #[structopt(short = "v", long)]
    value: String,
    #[structopt(short = "k", long)]
    k: u16,
    #[structopt(long)]
    d_max: Option<String>,
  },
  #[structopt(name = "range")]
  /// Returns all entries having a value in the given value range
  Range {
    #[structopt(short = "f", long = "from")]
    /// HLower value of the range
    lo: String,
    #[structopt(short = "t", long = "to")]
    /// Higher value of the range
    hi: String,
    #[structopt(short = "l", long)]
    /// Limits the number of entries in output
    limit: Option<usize>,
    #[structopt(short = "c", long)]
    /// Returns the size of the result instead of the result itself
    count: bool,
  },
}

#[derive(Clone, Debug, StructOpt, serde::Serialize, serde::Deserialize)]
pub enum ValOrFile {
  #[structopt(name = "value")]
  /// Execute the command for the specific given value
  Value { value: String },
  #[structopt(name = "list")]
  /// Execute the command for each value in the given file
  List { file: PathBuf },
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_iter(path: &Path, mode: Mode) -> Result<Box<dyn Iterator<Item = u64> + Send>, Error> {
  // Get the size of the file
  // let metadata = fs::metadata(&path)?;
  // let byte_size = metadata.len();
  // Open the file and read the metadata part
  let file = File::open(path)?;
  let mmap = unsafe { MmapOptions::new().map(&file)? };
  let (_version, data_starting_byte, meta) = read_meta(&mmap)?;
  if !meta.types.id_type().is_recno_compatible() {
    return Err(Error::new(
      ErrorKind::Other,
      "Index identifier type not compatible with a record number",
    ));
  }
  let idval = meta.types.clone();
  idval.exec(QueryIter {
    mode,
    meta,
    mmap,
    data_starting_byte,
  })
}

#[cfg(not(target_arch = "wasm32"))]
struct QueryIter {
  mode: Mode,
  meta: BSTreeMeta,
  mmap: Mmap,
  data_starting_byte: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl Process for QueryIter {
  type Output = Box<dyn Iterator<Item = u64> + Send>;

  fn exec<I, V, D, IRW, VRW>(
    self,
    _types: IdVal,
    id_rw: IRW,
    val_rw: VRW,
    dist: D,
  ) -> Result<Self::Output, Error>
  where
    I: 'static + Id,
    V: 'static + Val,
    D: 'static + Fn(&V, &V) -> V + Send,
    IRW: 'static + ReadWrite<Type = I>,
    VRW: 'static + ReadWrite<Type = V>,
  {
    let root = self.meta.get_root();
    match self.mode {
      Mode::Info => {
        println!("{}", serde_json::to_string_pretty(&self.meta)?);
        Ok(Box::new(iter::empty()))
      }
      Mode::GetFirst { ref val_or_file } => match val_or_file {
        ValOrFile::Value { value } => {
          let v = value
            .parse::<V>()
            .map_err(|_e| Error::new(ErrorKind::Other, "Wrong value type"))?;
          let visitor = VisitorExact::new(v);
          let visitor = root.visit(
            visitor,
            &self.mmap[self.data_starting_byte..],
            &id_rw,
            &val_rw,
          )?;
          Ok(Box::new(visitor.entry.into_iter().map(|e| e.id.to_u64())))
        }
        ValOrFile::List { file } => Ok(Box::new(
          BufReader::new(File::open(file)?)
            .lines()
            .filter_map(move |line| {
              line
                .and_then(|v| {
                  v.parse::<V>()
                    .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))
                })
                .and_then(|v| {
                  root.visit(
                    VisitorExact::new(v),
                    &self.mmap[self.data_starting_byte..],
                    &id_rw,
                    &val_rw,
                  )
                })
                .ok()
                .and_then(|v| v.entry)
            })
            .map(|e| e.id.to_u64()),
        )),
      },
      Mode::All {
        value,
        limit,
        count,
      } => {
        let v = value
          .parse::<V>()
          .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))?;
        if count {
          let v = VisitorAllCount::new(v, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
          println!("index output count");
          println!("{}", v.n_entries);
          Ok(Box::new(iter::empty()))
        } else {
          let v = VisitorAll::new(v, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
          Ok(Box::new(v.entries.into_iter().map(|e| e.id.to_u64())))
        }
      }
      Mode::Nn {
        ref val_or_file,
        ref d_max,
      } => {
        let d_max = d_max
          .as_ref()
          .map(|d| {
            d.parse::<V>()
              .map_err(|_| Error::new(ErrorKind::Other, "Wrong distance type"))
          })
          .transpose()?;
        match val_or_file {
          ValOrFile::Value { value } => {
            let v = value
              .parse::<V>()
              .map_err(|_| Error::new(ErrorKind::Other, ""))?;
            let v = VisitorNn::new(v, &dist, d_max);
            let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
            Ok(Box::new(
              v.nn.into_iter().map(|neig| neig.neighbour.id.to_u64()),
            ))
          }
          ValOrFile::List { file } => Ok(Box::new(
            BufReader::new(File::open(file)?)
              .lines()
              .filter_map(move |line| {
                line
                  .and_then(|v| {
                    v.parse::<V>()
                      .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))
                  })
                  .and_then(|v| {
                    root.visit(
                      VisitorNn::new(v, &dist, d_max.clone()),
                      &self.mmap[self.data_starting_byte..],
                      &id_rw,
                      &val_rw,
                    )
                  })
                  .ok()
                  .and_then(|v| v.nn)
              })
              .map(|neig| neig.neighbour.id.to_u64()),
          )),
        }
      }
      Mode::Knn { value, k, d_max } => {
        let v = value
          .parse::<V>()
          .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))?;
        let v: VisitorKnn<I, V, V, _> = VisitorKnn::new(
          v,
          dist,
          k as usize,
          d_max
            .map(|d| {
              d.parse::<V>()
                .map_err(|_| Error::new(ErrorKind::Other, "Wrong distance type"))
            })
            .transpose()?,
        );
        let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
        Ok(Box::new(
          v.knn
            .into_sorted_vec()
            .into_iter()
            .map(|neig| neig.neighbour.id.to_u64()),
        ))
      }
      Mode::Range {
        lo,
        hi,
        limit,
        count,
      } => {
        let lo = lo
          .parse::<V>()
          .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))?;
        let hi = hi
          .parse::<V>()
          .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))?;
        if count {
          let v = VisitorRangeCount::new(lo, hi, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
          println!("index output count");
          println!("{}", v.n_entries);
          Ok(Box::new(iter::empty()))
        } else {
          let v = VisitorRange::new(lo, hi, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
          Ok(Box::new(v.entries.into_iter().map(|e| e.id.to_u64())))
        }
      }
    }
  }
}
