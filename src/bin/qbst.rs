extern crate bstree_file;
use bstree_file::{
  Id, Val, IdVal, ValInMemType, Entry, Process,
  rw::ReadWrite,
  bstree::{read_meta, BSTreeMeta, SubTreeR},
  visitors::*,
};

use structopt::StructOpt;
use memmap::{Mmap, MmapOptions};

use std::io::{
  Error, ErrorKind, Read, Write
};
use std::fs::{self, File};
use std::path::PathBuf;

#[derive(Debug, StructOpt)]
#[structopt(name = "qbst")]
/// Query a Binary Search Tree stored in a file.
///
///  Example:
///   qbst test.bstree.bin knn -v 12.5 -k 16
struct Args {
  /// File storing the binary search tree
  input: PathBuf,
  #[structopt(subcommand)]
  mode: Mode,
}

#[derive(Debug, StructOpt)]
enum Mode {
  #[structopt(name = "get")]
  /// Returns the first entry having a value equal to the given value
  GetFirst {
    #[structopt(short = "v", long)]
    value: String,
  },
  #[structopt(name = "all")]
  /// Returns the first entry having a value equal to the given value
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
    #[structopt(short = "v", long)]
    value: String,
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
  }
}

impl Args {

  fn check(&self) {
    if !self.input.exists() {
      panic!("File {:?} does not exists.", self.input);
    }
  }

  fn exec(self) -> Result<(), std::io::Error> {
    // Get the size of the file
    let metadata = fs::metadata(&self.input)?;
    let byte_size = metadata.len();
    // Open the file and read the metadata part
    let file = File::open(&self.input)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let (version, data_starting_byte, meta) = read_meta(&mmap)?;
    let idval = &meta.types;
    idval.exec(
      Query {
        mode: self.mode,
        meta: &meta,
        mmap: &mmap,
        data_starting_byte,
      }
    )
  }
}

struct Query<'a> {
  mode: Mode,
  meta: &'a BSTreeMeta,
  mmap: &'a Mmap,
  data_starting_byte: usize,
}

impl<'a> Process for Query<'a> {
  type Output = ();

  fn exec<I, V, D, IRW, VRW>(self, types: &IdVal, id_rw: &IRW, val_rw: &VRW, dist: D) -> Result<Self::Output, Error>
  where I: Id,
        V: Val,
        D: Fn(&V, &V) -> V,
        IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V> {
    let root = self.meta.get_root();
    match self.mode {
      Mode::GetFirst { value} => {
        let v = value.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, "Wrong value type"))?;
        let visitor = VisitorExact::new(v);
        let visitor = root.visit(visitor, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
        println!("id,val");
        visitor.entry.map(|Entry {id, val}| println!("{:?},{:?}", id, val));
        Ok(())
      },
      Mode::All { value, limit, count } => {
        let v = value.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, "Wrong valie type"))?;
        if count {
          let v = VisitorAllCount::new(v, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
          println!("count");
          println!("{}", v.n_entries);
        } else {
          let v = VisitorAll::new(v, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
          println!("id,val");
          for Entry {id, val} in v.entries {
            println!("{:?},{:?}", id, val);
          }
        }
        Ok(())
      },
      Mode::Nn { value, d_max } => {
        let v = value.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, ""))?;
        let v = VisitorNn::new(
          v, dist,
          d_max.map(|d|  d.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, "Wrong distance type")))
            .transpose()?
        );
        let v = root.visit(v, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
        println!("distance,id,val");
        v.nn.map(|Neigbhour {distance: d, neighbour: Entry {id, val}}| println!("{:?},{:?},{:?}", d, id, val));
        Ok(())
      },
      Mode::Knn { value, k, d_max } => {
        let v = value.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, "Wrong value type"))?;
        let v: VisitorKnn<I, V, V, _> = VisitorKnn::new(
          v, dist, k as usize,
          d_max.map(|d|  d.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, "Wrong distance type")))
            .transpose()?
        );
        let v = root.visit(v, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
        println!("distance,id,val");
        for Neigbhour{distance: d, neighbour: Entry {id, val}} in v.knn.into_sorted_vec().drain(..) {
          println!("{:?},{:?},{:?}", d, id, val);
        }
        Ok(())
      },
      Mode::Range { lo,  hi, limit, count } => {
        let lo = lo.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, "Wrong valie type"))?;
        let hi = hi.parse::<V>().map_err(|e| Error::new(ErrorKind::Other, "Wrong valie type"))?;
        if count {
          let v = VisitorRangeCount::new(lo, hi, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
          println!("count");
          println!("{}", v.n_entries);
        } else {
          let v = VisitorRange::new(lo, hi, limit.unwrap_or(std::usize::MAX));
          let v = root.visit(v, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
          println!("id,val");
          for Entry {id, val} in v.entries {
            println!("{:?},{:?}", id, val);
          }
        }
        Ok(())
      },
    }
/*
     // V::from_str(&self.value).unwrap();

    let opt_entry = root.get(v, &self.mmap[self.data_starting_byte..], id_rw, val_rw)?;
    Ok(opt_entry.map(|Entry {id, val}| (format!("{:?}", id), format!("{:?}", val))))*/
  }
}


fn main() -> Result<(), Error> {
  // Parse commande line arguments
  let args = Args::from_args();
  args.check();
  args.exec()?;
  Ok(())
}
