use rand::prelude::*;
use structopt::StructOpt;

use std::io::{self, Error, Write, BufWriter};
use std::fs::File;
use std::path::PathBuf;

#[derive(Debug, StructOpt)]
#[structopt(name = "genfile")]
/// Generate a data file to be indexed in order to test `mkbst` and `qbst`.
///
///  Example:
///   genfile 10000000000 randf64 | mkbst -h test.10b.randf64 --id-type u5 --val-type f4
struct Args {
  #[structopt(short, long)]
  /// Generate a sequential index identifier in addition to the value
  oid: bool,
  #[structopt(subcommand)]
  mode: Mode,
  /// Number of rows to be generated
  n: usize,
  /// File storing the binary search tree
  output: Option<PathBuf>,
}

impl Args {

  fn exec(&self) -> Result<(), Error> {
    match &self.output {
      Some(path) => self.mode.write(self.oid, self.n, BufWriter::new(File::create(path)?)),
      None => self.mode.write(self.oid, self.n, io::stdout()),
    }
  }

}

#[derive(Debug, StructOpt)]
enum Mode {
  #[structopt(name = "seqint")]
  /// Generate sequential integers frm 0 to `n`
  SeqInt,
  #[structopt(name = "randint")]
  /// Generate random integer in `[0, n]`.
  RandInt,
  #[structopt(name = "seqf64")]
  /// Generate sequential doubles in `[0, 1]` (double at index `i` = `i / n`).
  SeqF64,
  #[structopt(name = "randf64")]
  /// Generate random doubles in `[0, 1]`.
  RandF64,
}

impl Mode {

  fn write<W: Write>(&self, oid: bool, n: usize, mut writer: W) -> Result<(), Error> {
    if oid {
      writer.write_all("id,val\n".as_bytes())?;
      match self {
        Mode::SeqInt => {
          for i in 0..n {
            writer.write_all(format!("{},{}\n", i, i).as_bytes())?;
          }
        },
        Mode::SeqF64 => {
          let nf64 = n as f64;
          for i in 0..n {
            writer.write_all(format!("{},{}\n", i, i as f64 / nf64).as_bytes())?;
          }
        },
        Mode::RandInt => {
          let mut rng = thread_rng();
          for i in 0..n {
            let j = rng.gen_range(0, n);
            writer.write_all(format!("{},{}\n", i, j).as_bytes())?;
          }
        },
        Mode::RandF64 => {
          let mut rng = thread_rng();
          for i in 0..n {
            let x: f64 = rng.gen(); // random number in range [0, 1)
            writer.write_all(format!("{},{}\n", i, x).as_bytes())?;
          }
        },
      }
    } else {
      writer.write_all("val\n".as_bytes())?;
      match self {
        Mode::SeqInt => {
          for i in 0..n {
            writer.write_all(format!("{}\n", i).as_bytes())?;
          }
        },
        Mode::SeqF64 => {
          let nf64 = n as f64;
          for i in 0..n {
            writer.write_all(format!("{}\n", i as f64 / nf64).as_bytes())?;
          }
        },
        Mode::RandInt => {
          let mut rng = thread_rng();
          for _ in 0..n {
            let j = rng.gen_range(0, n);
            writer.write_all(format!("{}\n", j).as_bytes())?;
          }
        },
        Mode::RandF64 => {
          let mut rng = thread_rng();
          for _ in 0..n {
            let x: f64 = rng.gen(); // random number in range [0, 1)
            writer.write_all(format!("{}\n", x).as_bytes())?;
          }
        },
      }
    }
    Ok(())
  }

}


fn main() -> Result<(), Error> {
  // Parse commande line arguments
  let args = Args::from_args();
  args.exec()?;
  Ok(())
}
