//! Generic arguments used to read CSV data from a file or from stdin.

use structopt::StructOpt;
use csv::{Reader, ReaderBuilder};
use std::path::PathBuf;
use std::fs::File;
use std::io::{stdin, Read, Error};

#[derive(Debug, StructOpt)]
pub struct CsvArgs {
  #[structopt(long, default_value = "8096")]
  /// Input buffer capacity in bytes
  capacity: usize,
  #[structopt(name = "separator", short = "s", long, default_value = ",")]
  /// ASCII input file delimiter
  delimiter: char,
  #[structopt(short = "q", long)]
  /// Support fields double quote parsing
  use_double_quote: bool,
  #[structopt(short = "e", long)]
  /// Use the \ escape character
  use_escape: bool,
  #[structopt(short = "x", long)]
  /// Do not generate an Error but a Warning if the number of columns is variable
  flexible: bool,
  #[structopt(short = "h", long)]
  /// The input file contains a header line (the first non-commented line)
  header: bool,
  #[structopt(short = "c", long)]
  /// The input file contains comments (lines starting by #)
  comments: bool,
  #[structopt(long, parse(from_os_str))]
  /// Input file (stdin if not present)
  input: Option<PathBuf>,
}

impl CsvArgs {

  fn create_reader_builder(&self) -> ReaderBuilder {
    let mut reader_builder = csv::ReaderBuilder::new();
    reader_builder.delimiter(self.delimiter as u8);
    reader_builder.has_headers(self.header);
    reader_builder.double_quote(self.use_double_quote);
    reader_builder.flexible(self.flexible);
    reader_builder.buffer_capacity(self.capacity);
    if self.use_escape {
      reader_builder.escape(Some(b'\\'));
    }
    if self.comments {
      reader_builder.comment(Some(b'#'));
    }
    reader_builder
  }
  
  
  /// Configure the reader, get it, and call the given function providing the created reader.
  /// ## Note
  /// * we use this strategy to use monomorphization instead of returning a trait object of type
  ///   `Reader<Box<dyn Read>>`
  pub fn call_once<F>(self, func: F) -> Result<F::Output, Error>
    where F: FnUsingReader {
    // Configure the reader (before creating it)
    let reader_builder = self.create_reader_builder();
    // Create a reader from file or stdin
    match self.input {
      Some(ref path_buf) => {
        let reader = reader_builder.from_reader(File::open(path_buf)?);
        func.call(reader)
      },
      None => {
        let reader = reader_builder.from_reader(stdin());
        func.call(reader)
      },
    }
  }

}

/// Defines a function which depends on a specific `Read` instance.
pub trait FnUsingReader {
  type Output;

  fn call<R: Read>(self, reader: Reader<R>) -> Result<Self::Output, Error>;
}
