//! Arguments used to find the proper column indices
use csv::StringRecord;
use std::io::{Error, ErrorKind};
use structopt::StructOpt;

/// Structure storing the indices of:
/// * the column containing the identifier (if any)
/// * the column containing the value to be indexed
pub struct ColIndices {
  pub id: Option<usize>,
  pub val: usize,
}

#[derive(Debug, StructOpt)]
pub struct ColArgs {
  #[structopt(short = "n", long)]
  /// Args of options id and val are column names, not indices (valid with option --header only)
  names: bool,
  #[structopt(short = "i", long)]
  /// Index or Name of the column containing the identifier (else the order in the input file is used, starting at 0)
  id: Option<String>,
  #[structopt(short = "v", long, default_value = "0")]
  /// Index or Name of the column containing the value to be indexed
  val: String,
}

impl ColArgs {
  pub fn has_id(&self) -> bool {
    self.id.is_some()
  }

  pub fn get_col_indices(&self, header: &Option<&StringRecord>) -> Result<ColIndices, Error> {
    match (self.names, header) {
      (false, _) => self.from_col_indices(),
      (true, &Some(string_record)) => self.from_col_names(string_record),
      (true, &None) => Err(Error::new(
        ErrorKind::Other,
        "Option -n, --names requires a header!",
      )),
    }
  }

  fn from_col_indices(&self) -> Result<ColIndices, Error> {
    Ok(ColIndices {
      id: self
        .id
        .as_ref()
        .map(|s| parse_index(s.as_str()))
        .transpose()?,
      val: parse_index(&self.val)?,
    })
  }

  fn from_col_names(&self, header: &StringRecord) -> Result<ColIndices, Error> {
    Ok(ColIndices {
      id: self
        .id
        .as_ref()
        .map(|id_col_name| index_from_name(id_col_name, header))
        .transpose()?,
      val: index_from_name(&self.val, header)?,
    })
  }
}

fn parse_index(icol_str: &str) -> Result<usize, Error> {
  icol_str.parse::<usize>().map_err(|_| {
    Error::new(
      ErrorKind::Other,
      format!(
        "Unable to parse '{}' into an integer. Check option -n, --names.",
        &icol_str
      ),
    )
  })
}

fn index_from_name(col_name: &str, header: &StringRecord) -> Result<usize, Error> {
  header
    .iter()
    .enumerate()
    .filter(|(_, col)| col == &col_name)
    .map(|(i, _)| i)
    .nth(1)
    .ok_or_else(|| {
      Error::new(
        ErrorKind::Other,
        format!("Column name '{}' not found in {:?}", col_name, header),
      )
    })
}
