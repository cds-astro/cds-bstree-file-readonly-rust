//! Visitors are the structures allowing ot perform queries on the binary-search tree.
use std::cmp::{Ord, Ordering};
use std::collections::BinaryHeap;
use std::marker::PhantomData;

use crate::{Entry, Id, Val};

pub trait Visitor {
  type I: Id;
  type V: Val;

  fn center(&self) -> &Self::V;

  /// Returns `true` if the visitor intersects the given range.
  // fn intersects(&self, from: Self::V, to: Self::V) -> bool;

  /// Visit the given entry
  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>);

  fn visit_le_center(&mut self, entry: Entry<Self::I, Self::V>);
  fn visit_he_center(&mut self, entry: Entry<Self::I, Self::V>);

  /// Continue visiting the left (descending) side of the tree (with respect to `center`)?
  fn visit_desc(&self) -> bool;

  /// Continue visiting the right (ascending) side of the tree (with respect to `center`)?
  fn visit_asc(&self) -> bool;
}

/// Defines a neighbour
pub struct Neigbhour<I, V, U>
where
  I: Id,
  V: Val,
  U: Ord,
{
  pub distance: U,
  pub neighbour: Entry<I, V>,
}

impl<I, V, U> PartialOrd for Neigbhour<I, V, U>
where
  I: Id,
  V: Val,
  U: Ord,
{
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.distance.partial_cmp(&other.distance)
  }
}

impl<I, V, U> PartialEq for Neigbhour<I, V, U>
where
  I: Id,
  V: Val,
  U: Ord,
{
  fn eq(&self, other: &Self) -> bool {
    self.distance == other.distance
  }
}

impl<I, V, U> Eq for Neigbhour<I, V, U>
where
  I: Id,
  V: Val,
  U: Ord,
{
}

impl<I, V, U> Ord for Neigbhour<I, V, U>
where
  I: Id,
  V: Val,
  U: Ord,
{
  fn cmp(&self, other: &Self) -> Ordering {
    self.distance.cmp(&other.distance)
  }
}

/// Look for an exact value
pub struct VisitorExact<I: Id, V: Val> {
  center: V,
  pub entry: Option<Entry<I, V>>,
}

impl<I: Id, V: Val> VisitorExact<I, V> {
  pub fn new(center: V) -> Self {
    Self {
      center,
      entry: None,
    }
  }
}

impl<I: Id, V: Val> Visitor for VisitorExact<I, V> {
  type I = I;
  type V = V;

  fn center(&self) -> &Self::V {
    &self.center
  }

  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>) {
    debug_assert_eq!(entry.val, self.center);
    self.entry = Some(entry)
  }

  fn visit_le_center(&mut self, _entry: Entry<Self::I, Self::V>) {
    unreachable!() // because visit_desc always returns false
  }

  fn visit_he_center(&mut self, _entry: Entry<Self::I, Self::V>) {
    unreachable!() // because visit_asc always returns false
  }

  fn visit_desc(&self) -> bool {
    false
  }
  fn visit_asc(&self) -> bool {
    false
  }
}

/// Look for all values, count only
pub struct VisitorAllCount<I: Id, V: Val> {
  center: V,
  limit: usize,
  pub n_entries: usize,
  desc: bool,
  asc: bool,
  _id: PhantomData<I>,
}

impl<I: Id, V: Val> VisitorAllCount<I, V> {
  pub fn new(center: V, limit: usize) -> Self {
    Self {
      center,
      limit,
      n_entries: 0,
      desc: true,
      asc: true,
      _id: PhantomData,
    }
  }
}

impl<I: Id, V: Val> Visitor for VisitorAllCount<I, V> {
  type I = I;
  type V = V;

  fn center(&self) -> &Self::V {
    &self.center
  }

  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>) {
    debug_assert_eq!(entry.val, self.center);
    self.n_entries += 1;
  }

  fn visit_le_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val == self.center && self.n_entries < self.limit {
      self.n_entries += 1;
    } else {
      self.desc = false;
    }
  }

  fn visit_he_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val == self.center && self.n_entries < self.limit {
      self.n_entries += 1;
    } else {
      self.asc = false;
    }
  }

  fn visit_desc(&self) -> bool {
    self.desc
  }
  fn visit_asc(&self) -> bool {
    self.asc
  }
}

/// Look for all values
pub struct VisitorAll<I: Id, V: Val> {
  center: V,
  limit: usize,
  pub entries: Vec<Entry<I, V>>,
  desc: bool,
  asc: bool,
}

impl<I: Id, V: Val> VisitorAll<I, V> {
  pub fn new(center: V, limit: usize) -> Self {
    Self {
      center,
      limit,
      entries: Default::default(),
      desc: true,
      asc: true,
    }
  }
}

impl<I: Id, V: Val> Visitor for VisitorAll<I, V> {
  type I = I;
  type V = V;

  fn center(&self) -> &Self::V {
    &self.center
  }

  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>) {
    debug_assert_eq!(entry.val, self.center);
    self.entries.push(entry);
  }

  fn visit_le_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val == self.center && self.entries.len() < self.limit {
      self.entries.push(entry);
    } else {
      self.desc = false;
    }
  }

  fn visit_he_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val == self.center && self.entries.len() < self.limit {
      self.entries.push(entry);
    } else {
      self.asc = false;
    }
  }

  fn visit_desc(&self) -> bool {
    self.desc
  }
  fn visit_asc(&self) -> bool {
    self.asc
  }
}

/*impl<I: Id, V: Val> IntoIterator for VisitorAll<I, V> {
  type Item = Entry<I, V>;
  type IntoIter = IntoIter<Self::Item>;

  fn into_iter(self) -> Self::IntoIter {

  }
}*/

/*impl<I: Id, V: Val> Iterator for VisitorAll<I, V> {
  type Item = Entry<I, V>;

  fn next(&mut self) -> Option<Self::Item> {
    self.entries.clone()
  }
}*/

/// Look for the nearest neighbour
pub struct VisitorNn<'a, I, V, U, D>
where
  I: Id,
  V: Val,
  U: Ord,
  D: Fn(&V, &V) -> U,
{
  center: V,
  dist: &'a D,
  d_max: Option<U>,
  pub nn: Option<Neigbhour<I, V, U>>,
  desc: bool,
  asc: bool,
}

impl<'a, I, V, U, D> VisitorNn<'a, I, V, U, D>
where
  I: Id,
  V: Val,
  U: Ord,
  D: Fn(&V, &V) -> U,
{
  pub fn new(center: V, distance: &'a D, d_max: Option<U>) -> Self {
    Self {
      center,
      dist: distance,
      d_max,
      nn: None,
      desc: true,
      asc: true,
    }
  }
}

impl<'a, I, V, U, D> Visitor for VisitorNn<'a, I, V, U, D>
where
  I: Id,
  V: Val,
  U: Ord,
  D: Fn(&V, &V) -> U,
{
  type I = I;
  type V = V;

  fn center(&self) -> &Self::V {
    &self.center
  }

  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>) {
    debug_assert_eq!(entry.val, self.center);
    let distance = (self.dist)(&self.center, &entry.val);
    self.nn = Some(Neigbhour {
      distance,
      neighbour: entry,
    });
    self.desc = false;
    self.asc = false;
  }

  fn visit_le_center(&mut self, entry: Entry<Self::I, Self::V>) {
    let distance = (self.dist)(&self.center, &entry.val);
    if let Some(dm) = &self.d_max {
      if distance.cmp(dm) == Ordering::Greater {
        self.desc = false;
        return;
      }
    }
    match &self.nn {
      Some(neig) => {
        if distance.lt(&neig.distance) {
          self.nn = Some(Neigbhour {
            distance,
            neighbour: entry,
          });
        }
      }
      None => {
        self.nn = Some(Neigbhour {
          distance,
          neighbour: entry,
        });
      }
    }
    self.desc = false;
  }

  fn visit_he_center(&mut self, entry: Entry<Self::I, Self::V>) {
    let distance = (self.dist)(&self.center, &entry.val);
    if let Some(dm) = &self.d_max {
      if distance.cmp(dm) == Ordering::Greater {
        self.asc = false;
        return;
      }
    }
    match &self.nn {
      Some(neig) => {
        if distance.lt(&neig.distance) {
          self.nn = Some(Neigbhour {
            distance,
            neighbour: entry,
          });
        }
      }
      None => {
        self.nn = Some(Neigbhour {
          distance,
          neighbour: entry,
        });
      }
    }
    self.asc = false;
  }

  fn visit_desc(&self) -> bool {
    self.desc
  }
  fn visit_asc(&self) -> bool {
    self.asc
  }
}

/// Look for the K Nearest Neighbours
pub struct VisitorKnn<I, V, U, D>
where
  I: Id,
  V: Val,
  U: Ord,
  D: Fn(&V, &V) -> U,
{
  center: V,
  dist: D,
  k: usize,
  d_max: Option<U>,
  pub knn: BinaryHeap<Neigbhour<I, V, U>>,
  desc: bool,
  asc: bool,
}

impl<I, V, U, D> VisitorKnn<I, V, U, D>
where
  I: Id,
  V: Val,
  U: Ord,
  D: Fn(&V, &V) -> U,
{
  pub fn new(center: V, distance: D, k: usize, d_max: Option<U>) -> Self {
    Self {
      center,
      dist: distance,
      k,
      d_max,
      knn: Default::default(),
      desc: true,
      asc: true,
    }
  }
}

impl<I, V, U, D> Visitor for VisitorKnn<I, V, U, D>
where
  I: Id,
  V: Val,
  U: Ord,
  D: Fn(&V, &V) -> U,
{
  type I = I;
  type V = V;

  fn center(&self) -> &Self::V {
    &self.center
  }

  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>) {
    debug_assert_eq!(entry.val, self.center);
    let distance = (self.dist)(&self.center, &entry.val);
    if self.k > 0 {
      self.knn.push(Neigbhour {
        distance,
        neighbour: entry,
      });
    } else {
      self.desc = false;
      self.asc = false;
    }
  }

  fn visit_le_center(&mut self, entry: Entry<Self::I, Self::V>) {
    let distance = (self.dist)(&self.center, &entry.val);
    if let Some(dm) = &self.d_max {
      if distance.gt(dm) {
        self.desc = false;
        return;
      }
    }
    if self.knn.len() < self.k || distance.lt(&self.knn.peek().unwrap().distance) {
      self.knn.push(Neigbhour {
        distance,
        neighbour: entry,
      });
      if self.knn.len() > self.k {
        self.knn.pop();
      }
    } else {
      self.desc = false;
    }
  }

  fn visit_he_center(&mut self, entry: Entry<Self::I, Self::V>) {
    let distance = (self.dist)(&self.center, &entry.val);
    if let Some(dm) = &self.d_max {
      if distance.gt(dm) {
        self.asc = false;
        return;
      }
    }
    if self.knn.len() < self.k || distance.lt(&self.knn.peek().unwrap().distance) {
      self.knn.push(Neigbhour {
        distance,
        neighbour: entry,
      });
      if self.knn.len() > self.k {
        self.knn.pop();
      }
    } else {
      self.asc = false;
    }
  }

  fn visit_desc(&self) -> bool {
    self.desc
  }
  fn visit_asc(&self) -> bool {
    self.asc
  }
}

/// Count all values in a given range
pub struct VisitorRangeCount<I, V>
where
  I: Id,
  V: Val,
{
  lo: V,
  hi: V,
  limit: usize,
  pub n_entries: usize,
  desc: bool,
  asc: bool,
  _id: PhantomData<I>,
}

impl<I, V> VisitorRangeCount<I, V>
where
  I: Id,
  V: Val,
{
  pub fn new(lo: V, hi: V, limit: usize) -> Self {
    VisitorRangeCount {
      lo,
      hi,
      limit,
      n_entries: 0,
      desc: true, // in case of equality with the lower value...
      asc: true,
      _id: PhantomData,
    }
  }
}

impl<I: Id, V: Val> Visitor for VisitorRangeCount<I, V> {
  type I = I;
  type V = V;

  fn center(&self) -> &Self::V {
    &self.lo
  }

  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>) {
    debug_assert_eq!(entry.val, self.lo);
    self.n_entries += 1;
  }

  fn visit_le_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val.lt(&self.lo) || self.n_entries >= self.limit {
      self.desc = false;
    } else {
      self.n_entries += 1;
    }
  }

  fn visit_he_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val.gt(&self.hi) || self.n_entries >= self.limit {
      self.asc = false;
    } else {
      self.n_entries += 1;
    }
  }

  fn visit_desc(&self) -> bool {
    self.desc
  }
  fn visit_asc(&self) -> bool {
    self.asc
  }
}

/// Look for all values in a given range
pub struct VisitorRange<I, V>
where
  I: Id,
  V: Val,
{
  lo: V,
  hi: V,
  limit: usize,
  pub entries: Vec<Entry<I, V>>,
  desc: bool,
  asc: bool,
  _id: PhantomData<I>,
}

impl<I, V> VisitorRange<I, V>
where
  I: Id,
  V: Val,
{
  pub fn new(lo: V, hi: V, limit: usize) -> Self {
    VisitorRange {
      lo,
      hi,
      limit,
      entries: Default::default(),
      desc: true, // in case of equality with the lower value...
      asc: true,
      _id: PhantomData,
    }
  }
}

impl<I: Id, V: Val> Visitor for VisitorRange<I, V> {
  type I = I;
  type V = V;

  fn center(&self) -> &Self::V {
    &self.lo
  }

  fn visit_center(&mut self, entry: Entry<Self::I, Self::V>) {
    debug_assert_eq!(entry.val, self.lo);
    self.entries.push(entry);
  }

  fn visit_le_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val.lt(&self.lo) || self.entries.len() >= self.limit {
      self.desc = false;
    } else {
      self.entries.push(entry);
    }
  }

  fn visit_he_center(&mut self, entry: Entry<Self::I, Self::V>) {
    if entry.val.gt(&self.hi) || self.entries.len() >= self.limit {
      self.asc = false;
    } else {
      self.entries.push(entry);
    }
  }

  fn visit_desc(&self) -> bool {
    self.desc
  }
  fn visit_asc(&self) -> bool {
    self.asc
  }
}
