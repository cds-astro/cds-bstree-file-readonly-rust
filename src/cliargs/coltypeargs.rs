//! Arguments used to provide the columns datatypes
use structopt::StructOpt;

use crate::{IdType, ValType, IdVal};

#[derive(Debug, StructOpt)]
pub struct ColTypeArgs {
  #[structopt(long)]
  /// Datatype of the value
  id_type: IdType,
  #[structopt(long)]
  /// Datatype of the value
  val_type: ValType,
  #[structopt(short = "u", long)]
  /// Support null values in the value field
  null_val: bool,
}

impl ColTypeArgs {
  
  pub fn is_recno_compatible(&self) -> bool {
    self.id_type.is_recno_compatible()
  }
  
  pub fn supports_null(&self) -> bool {
    self.null_val
  }
  
  pub fn to_idval(self) -> IdVal {
    IdVal(self.id_type, self.val_type)
  }
  
}


