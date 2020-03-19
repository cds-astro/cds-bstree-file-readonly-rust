//! Implementation of the `Ord` trait on finite `float`.

use num_traits::{Float, ParseFloatError, FloatErrorKind};
use std::str::FromStr;
use std::cmp::Ordering;

/// A finite float cannot contain NaN, +Inf or -Inf values.
/// We did so in order to be able to implement the `Ord` trait.
/// See e.g. [this](https://stackoverflow.com/questions/28247990/how-to-do-a-binary-search-on-a-vec-of-floats).
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct FiniteFloat<T: Float>(T);

impl <T: Float> FiniteFloat<T>  {
  /// Returns None if the given value is not finite.
  pub fn new(val: T) -> Option<FiniteFloat<T>> {
    if val.is_finite() {
      Some(FiniteFloat(val))
    } else {
      None
    }
  }
  /// Returns the value this float contains.
  /// The return value is finite and can thus be used 
  pub fn get(&self) -> T {
    self.0
  }
}

impl <T: Float> Eq for FiniteFloat<T> {}

impl <T: Float> Ord for FiniteFloat<T> {
  fn cmp(&self, other: &FiniteFloat<T>) -> Ordering {
    // We use the default u32 or u64 comparison knowing that we can only compare finite values.
    self.partial_cmp(other).unwrap()
  }
}

impl <T: FromStr + Float> FromStr for FiniteFloat<T> {
  
  type Err = ParseFloatError; //<T as FromStr>::Err;
  
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    const ERR: ParseFloatError = ParseFloatError { kind: FloatErrorKind::Invalid };
    FiniteFloat::<T>::new(
      T::from_str(s).map_err(|_| ERR)?
    ).ok_or_else(|| ERR)
  }
}
