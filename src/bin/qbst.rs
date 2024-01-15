extern crate bstree_file;
use bstree_file::{
  bstree::{read_meta, BSTreeMeta, SubTreeR},
  cliargs::mode::*,
  rw::ReadWrite,
  visitors::*,
  Entry, Id, IdVal, Process, Val,
};

#[cfg(not(target_arch = "wasm32"))]
use memmap::{Mmap, MmapOptions};
use structopt::{StructOpt, clap::AppSettings};

use std::{
  fs::File,
  io::{BufRead, BufReader, Error, ErrorKind},
  path::PathBuf,
};
use std::io::Cursor;

#[derive(Debug, StructOpt)]
#[structopt(name = "qbst", global_settings = &[AppSettings::ColoredHelp, AppSettings::AllowNegativeNumbers])]
/// Query a Binary Search Tree stored in a file.
///
///  Example:
///   qbst test.bstree.bin knn -v 12.5 -k 16
pub struct Args {
  /// File storing the binary search tree
  input: PathBuf,
  #[structopt(subcommand)]
  mode: Mode,
}

impl Args {
  fn check(&self) {
    if !self.input.exists() {
      panic!("File {:?} does not exists.", self.input);
    }
  }

  #[cfg(not(target_arch = "wasm32"))]
  fn exec(self) -> Result<(), std::io::Error> {
    // Get the size of the file
    //let metadata = fs::metadata(&self.input)?;
    //let byte_size = metadata.len();
    // Open the file and read the metadata part
    let file = File::open(&self.input)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let (_version, data_starting_byte, meta) = read_meta(&mmap)?;
    let idval = &meta.types;
    idval.exec(Query {
      mode: self.mode,
      meta: &meta,
      mmap: &mmap,
      data_starting_byte,
    })
  }
}

#[cfg(not(target_arch = "wasm32"))]
struct Query<'a> {
  mode: Mode,
  meta: &'a BSTreeMeta,
  mmap: &'a Mmap,
  data_starting_byte: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> Process for Query<'a> {
  type Output = ();

  fn exec<I, V, D, IRW, VRW>(
    self,
    _types: IdVal,
    id_rw: IRW,
    val_rw: VRW,
    dist: D,
  ) -> Result<Self::Output, Error>
  where
    I: Id,
    V: Val,
    D: Fn(&V, &V) -> V,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let root = self.meta.get_root();
    match self.mode {
      Mode::Info => {
        println!("{}", serde_json::to_string_pretty(&self.meta)?);
        Ok(())
      }
      Mode::Data { limit } => {
        let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
        println!("id,val");
        match limit {
          Some(limit) => {
            for kv in self.mmap[self.data_starting_byte..].chunks_exact(entry_byte_size).take(limit) {
              let mut cursor = Cursor::new(kv);
              let id = id_rw.read(&mut cursor)?;
              let val = val_rw.read(&mut cursor)?;
              println!("{},{}", id, val);
            }
          },
          None => {
            for kv in self.mmap[self.data_starting_byte..].chunks_exact(entry_byte_size) {
              let mut cursor = Cursor::new(kv);
              let id = id_rw.read(&mut cursor)?;
              let val = val_rw.read(&mut cursor)?;
              println!("{},{}", id, val);
            }   
          }
        }
        Ok(())
      }
      Mode::GetFirst { val_or_file } => match val_or_file {
        ValOrFile::Value { value } => {
          let v = value
            .parse::<V>()
            .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))?;
          let visitor = VisitorExact::new(v);
          let visitor = root.visit(
            visitor,
            &self.mmap[self.data_starting_byte..],
            &id_rw,
            &val_rw,
          )?;
          println!("id,val");
          if let Some(Entry { id, val }) = visitor.entry {
            println!("{},{}", id, val)
          }
          Ok(())
        }
        ValOrFile::List { file } => {
          let file = File::open(file)?;
          println!("id,val");
          for line in BufReader::new(file).lines() {
            let value = line?;
            let v = value
              .parse::<V>()
              .map_err(|_| Error::new(ErrorKind::Other, "Wrong value type"))?;
            let visitor = VisitorExact::new(v);
            let visitor = root.visit(
              visitor,
              &self.mmap[self.data_starting_byte..],
              &id_rw,
              &val_rw,
            )?;
            if let Some(Entry { id, val }) = visitor.entry {
              println!("{},{}", id, val)
            }
          }
          Ok(())
        }
      },
      Mode::All {
        value,
        limit,
        count,
      } => {
        let v = value
          .parse::<V>()
          .map_err(|_| Error::new(ErrorKind::Other, "Wrong valie type"))?;
        if count {
          let v = VisitorAllCount::new(v, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
          println!("count");
          println!("{}", v.n_entries);
        } else {
          let v = VisitorAll::new(v, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
          println!("id,val");
          for Entry { id, val } in v.entries {
            println!("{},{}", id, val);
          }
        }
        Ok(())
      }
      Mode::Nn { val_or_file, d_max } => {
        let d_max = d_max
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
            println!("distance,id,val");
            if let Some(Neigbhour {
              distance: d,
              neighbour: Entry { id, val },
            }) = v.nn
            {
              println!("{},{},{}", d, id, val);
            }
            Ok(())
          }
          ValOrFile::List { file } => {
            let file = File::open(file)?;
            println!("distance,id,val");
            for line in BufReader::new(file).lines() {
              let value = line?;
              let v = value
                .parse::<V>()
                .map_err(|_e| Error::new(ErrorKind::Other, ""))?;
              let v = VisitorNn::new(v, &dist, d_max.clone());
              let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
              if let Some(Neigbhour {
                distance: d,
                neighbour: Entry { id, val },
              }) = v.nn
              {
                println!("{},{},{}", d, id, val);
              }
            }
            Ok(())
          }
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
        println!("distance,id,val");
        for Neigbhour {
          distance: d,
          neighbour: Entry { id, val },
        } in v.knn.into_sorted_vec().drain(..)
        {
          println!("{},{},{}", d, id, val);
        }
        Ok(())
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
          println!("count");
          println!("{}", v.n_entries);
        } else {
          let v = VisitorRange::new(lo, hi, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
          println!("id,val");
          for Entry { id, val } in v.entries {
            println!("{},{}", id, val);
          }
        }
        Ok(())
      }
    }
  }
}

fn main() -> Result<(), Error> {
  // Parse command line arguments
  let args = Args::from_args();
  args.check();
  #[cfg(not(target_arch = "wasm32"))]
  args.exec()?;
  Ok(())
}
