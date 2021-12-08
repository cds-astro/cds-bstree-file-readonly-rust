extern crate bstree_file;

use structopt::StructOpt;
use csv::Reader;

use bstree_file::cliargs::{
  csvargs::*,
  memsize::*,
  mkargs::*,
  colargs::*,
  coltypeargs::*
};
use bstree_file::mk::MkIndex;

use std::io::{Error, ErrorKind, Read};

#[derive(Debug, StructOpt)]
#[structopt(name = "mkbst")]
/// Binary Search Tree creation in a file.
///
/// Examples:
///     ./mkbst -h --input resources/tests/vals.csv test --id-type u4 --val-type u4
/// Example: cat 2mass.csv | ./mkbst -hnx 2mass.jmag -i oid --id-type u4 -v Jmag --val-type f4
/// Example: single column containing e.g. a magnitude:
///   cat Jmag.txt | ./mkbstree 2mass.jmag --id-type u4 --val-type f4
struct Args {
  #[structopt(flatten)]
  csv_args: CsvArgs,
  #[structopt(flatten)]
  sub_args: SubArgs,
}

impl Args {
  fn exec(self) -> Result<<SubArgs as FnUsingReader>::Output, std::io::Error> {
    self.sub_args.check()?;
    self.csv_args.call_once(self.sub_args)
  }
}

#[derive(Debug, StructOpt)]
struct SubArgs {
  #[structopt(flatten)]
  col_args: ColArgs,
  #[structopt(flatten)]
  coltype_args: ColTypeArgs,
  #[structopt(flatten)]
  mkalgo_args: MkAlgoArgs,
  #[structopt(flatten)]
  mem_args: MemSizeArgs,
}

impl SubArgs {
  fn check(&self) -> Result<(), Error> {
    if !self.col_args.has_id() && !self.coltype_args.is_recno_compatible() {
      return Err(Error::new(ErrorKind::Other, "Id is a recno. Compatible types are: U24, U32, U40, U48, U56 or U62!")); // String::from(
    }
    Ok(())
  }
}

impl FnUsingReader for SubArgs {
  type Output = usize; 
  // type Output = <MkIndex<Read> as Process>::Output;
  
  fn call<R: Read>(self, mut reader: Reader<R>) -> Result<Self::Output, Error> {
    // Read header if required
    let header = if reader.has_headers() {
      Some(reader.headers()?)
    } else {
      None
    };
    // Get column indices for parsing
    let col_indices = self.col_args.get_col_indices(&header)?;
    // According to column indices and supports for null value, use different parsing functions.
    // This also depends on IdType and ValType!
    let process = MkIndex::<R>::new(
      reader, 
      col_indices, 
      self.coltype_args.supports_null(), 
      self.mkalgo_args,
      self.mem_args,
    );
    self.coltype_args.to_idval().exec(process)
  }
}

fn main() -> Result<(), Error> {
  let args = Args::from_args();
  args.exec().map(|_| ())
}
