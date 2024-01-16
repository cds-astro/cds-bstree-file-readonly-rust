//! Implementation of the `Ord` trait on finite `float`.

use num_traits::{Float, FloatErrorKind, ParseFloatError};
use std::{
  cmp::Ordering,
  fmt::{self, Display, Formatter},
  str::FromStr,
};

/// A finite float cannot contain NaN, +Inf or -Inf values.
/// We did so in order to be able to implement the `Ord` trait.
/// See e.g. [this](https://stackoverflow.com/questions/28247990/how-to-do-a-binary-search-on-a-vec-of-floats).
#[derive(Debug, Clone, PartialEq)]
pub struct FiniteFloat<T: Float + Display>(T);

impl<T: Float + Display> FiniteFloat<T> {
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

impl<T: Float + Display> Eq for FiniteFloat<T> {}

impl<T: Float + Display> Ord for FiniteFloat<T> {
  fn cmp(&self, other: &FiniteFloat<T>) -> Ordering {
    // We use the default u32 or u64 comparison knowing that we can only compare finite values.
    self.0.partial_cmp(&other.0).unwrap()
  }
}

impl<T: Float + Display> PartialOrd for FiniteFloat<T> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<T: FromStr + Float + Display> FromStr for FiniteFloat<T> {
  type Err = ParseFloatError; //<T as FromStr>::Err;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    const ERR: ParseFloatError = ParseFloatError {
      kind: FloatErrorKind::Invalid,
    };
    T::from_str(s)
      .map_err(|_| ERR)
      .and_then(|v| FiniteFloat::<T>::new(v).ok_or(ERR))
  }
}

impl<T: Float + Display> Display for FiniteFloat<T> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", &self.get())
  }
}
