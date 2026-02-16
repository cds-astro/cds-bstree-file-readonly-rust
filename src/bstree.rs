//! See the tree terminology here: https://en.wikipedia.org/wiki/Tree_(data_structure)
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
#[cfg(not(target_arch = "wasm32"))]
use memmap::{Mmap, MmapMut};
use serde::{self, Deserialize, Serialize};

use std::{
  fs::OpenOptions,
  io::{Error, ErrorKind, Read, Write},
  num::ParseIntError,
  path::PathBuf,
};

use crate::{
  cliargs::memsize::MemSizeArgs, rw::ReadWrite, visitors::*, Entry, Id, IdVal, Process, RawEntries,
  Val,
};

const FILE_TYPE: &[u8; 10] = b"BSTreeFile";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub trait HasByteSize {
  /// Returns the total size of the sub-tree, in bytes.
  /// # Args
  /// * `entry_byte_size`: the size of a single entry, in bytes (this size is constant in the full tree).
  fn byte_size(&self, entry_byte_size: usize) -> usize;
}

/// Trait to write a sub-tree
/// TODO: decorate the iterator to ensure that it is sorted!!
trait SubTreeW: HasByteSize {
  /// Fill this sub-tree with the entries provided in the ordered iterator.
  fn write<I, V, IRW, VRW, T>(
    &self,
    entries_iterator: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>;
}

pub trait SubTreeR: HasByteSize {
  fn get<I, V, IRW, VRW>(
    &self,
    value: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>;

  /// Visit from the largest to the smallest value
  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>;

  /// Visit starting with a binary search of the visitor central value
  fn visit<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>;

  /// Visit from the smallest to the largest value
  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>;
}

#[derive(Debug)]
pub enum Root {
  L1Leaf(L1Leaf),         // L1 very small tree => very unlikely
  L1Node(L1Node), // L1  -> LD few changes to have the exact number of entries => very unlikely
  LDNode(LDNode), // LD  -> LD few changes to have the exact number of entries => very unlikely
  RootL1Node(RootL1Node), // root made of a single L1 leaf pointing to sub-trees
  RootLDNode(RootLDNode), // root made of a LD node pointing to sub-tree (sub-LDNodes)
                  // Remarks:
                  // * a (Root)LDLeaf is a (Root)L1Node made of L1Leaves as sub-tree
                  // * The number of elements in the root array of a root LDNode may be larger than in other
                  //   LD blocks. Thus, the LD block may not fit in the disk cache!
}

impl HasByteSize for Root {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    match &self {
      Root::L1Leaf(leaf) => leaf.byte_size(entry_byte_size),
      Root::L1Node(node) => node.byte_size(entry_byte_size),
      Root::LDNode(node) => node.byte_size(entry_byte_size),
      Root::RootL1Node(node) => node.byte_size(entry_byte_size),
      Root::RootLDNode(node) => node.byte_size(entry_byte_size),
    }
  }
}

impl SubTreeW for Root {
  fn write<I, V, IRW, VRW, T>(
    &self,
    entries_iterator: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    // Simple delegation
    match &self {
      Root::L1Leaf(leaf) => leaf.write(entries_iterator, id_rw, val_rw, dest),
      Root::L1Node(node) => node.write(entries_iterator, id_rw, val_rw, dest),
      Root::LDNode(node) => node.write(entries_iterator, id_rw, val_rw, dest),
      Root::RootL1Node(node) => node.write(entries_iterator, id_rw, val_rw, dest),
      Root::RootLDNode(node) => node.write(entries_iterator, id_rw, val_rw, dest),
    }
  }
}

impl SubTreeR for Root {
  fn get<I, V, IRW, VRW>(
    &self,
    value: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    // Simple delegation
    match &self {
      Root::L1Leaf(leaf) => leaf.get(value, raw_entries, id_rw, val_rw),
      Root::L1Node(node) => node.get(value, raw_entries, id_rw, val_rw),
      Root::LDNode(node) => node.get(value, raw_entries, id_rw, val_rw),
      Root::RootL1Node(node) => node.get(value, raw_entries, id_rw, val_rw),
      Root::RootLDNode(node) => node.get(value, raw_entries, id_rw, val_rw),
    }
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      Root::L1Leaf(leaf) => leaf.visit_desc(visitor, raw_entries, id_rw, val_rw),
      Root::L1Node(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
      Root::LDNode(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
      Root::RootL1Node(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
      Root::RootLDNode(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
    }
  }

  fn visit<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      Root::L1Leaf(leaf) => leaf.visit(visitor, raw_entries, id_rw, val_rw),
      Root::L1Node(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
      Root::LDNode(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
      Root::RootL1Node(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
      Root::RootLDNode(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
    }
  }

  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      Root::L1Leaf(leaf) => leaf.visit_asc(visitor, raw_entries, id_rw, val_rw),
      Root::L1Node(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
      Root::LDNode(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
      Root::RootL1Node(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
      Root::RootLDNode(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
    }
  }
}

#[derive(Debug)]
pub enum SubTree {
  L1Leaf(L1Leaf),
  L1Node(L1Node), // LDLeaf = L1Node with L1Leaf as sub-tree. The LDLeaf must fit into the disk cache (except if it is the root).
  LDNode(LDNode),
}

impl HasByteSize for SubTree {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    match &self {
      SubTree::L1Leaf(leaf) => leaf.byte_size(entry_byte_size),
      SubTree::L1Node(node) => node.byte_size(entry_byte_size),
      SubTree::LDNode(node) => node.byte_size(entry_byte_size),
    }
  }
}

impl SubTreeW for SubTree {
  fn write<I, V, IRW, VRW, T>(
    &self,
    entries_iterator: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    // Simple delegation
    match &self {
      SubTree::L1Leaf(leaf) => leaf.write(entries_iterator, id_rw, val_rw, dest),
      SubTree::L1Node(node) => node.write(entries_iterator, id_rw, val_rw, dest),
      SubTree::LDNode(node) => node.write(entries_iterator, id_rw, val_rw, dest),
    }
  }
}

impl SubTreeR for SubTree {
  fn get<I, V, IRW, VRW>(
    &self,
    value: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    // Simple delegation
    match &self {
      SubTree::L1Leaf(leaf) => leaf.get(value, raw_entries, id_rw, val_rw),
      SubTree::L1Node(node) => node.get(value, raw_entries, id_rw, val_rw),
      SubTree::LDNode(node) => node.get(value, raw_entries, id_rw, val_rw),
    }
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      SubTree::L1Leaf(leaf) => leaf.visit_desc(visitor, raw_entries, id_rw, val_rw),
      SubTree::L1Node(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
      SubTree::LDNode(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
    }
  }

  fn visit<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      SubTree::L1Leaf(leaf) => leaf.visit(visitor, raw_entries, id_rw, val_rw),
      SubTree::L1Node(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
      SubTree::LDNode(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
    }
  }

  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      SubTree::L1Leaf(leaf) => leaf.visit_asc(visitor, raw_entries, id_rw, val_rw),
      SubTree::L1Node(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
      SubTree::LDNode(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
    }
  }
}

#[derive(Debug)]
pub enum LDSubTree {
  L1Node(L1Node), // LDLeaf = L1Node with L1Leaf as sub-tree
  LDNode(LDNode),
}

impl HasByteSize for LDSubTree {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    match &self {
      LDSubTree::L1Node(node) => node.byte_size(entry_byte_size),
      LDSubTree::LDNode(node) => node.byte_size(entry_byte_size),
    }
  }
}

impl SubTreeW for LDSubTree {
  fn write<I, V, IRW, VRW, T>(
    &self,
    entries_iterator: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    match &self {
      LDSubTree::L1Node(node) => node.write(entries_iterator, id_rw, val_rw, dest),
      LDSubTree::LDNode(node) => node.write(entries_iterator, id_rw, val_rw, dest),
    }
  }
}

impl SubTreeR for LDSubTree {
  fn get<I, V, IRW, VRW>(
    &self,
    value: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    match &self {
      LDSubTree::L1Node(node) => node.get(value, raw_entries, id_rw, val_rw),
      LDSubTree::LDNode(node) => node.get(value, raw_entries, id_rw, val_rw),
    }
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      LDSubTree::L1Node(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
      LDSubTree::LDNode(node) => node.visit_desc(visitor, raw_entries, id_rw, val_rw),
    }
  }
  fn visit<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      LDSubTree::L1Node(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
      LDSubTree::LDNode(node) => node.visit(visitor, raw_entries, id_rw, val_rw),
    }
  }

  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    // Simple delegation
    match &self {
      LDSubTree::L1Node(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
      LDSubTree::LDNode(node) => node.visit_asc(visitor, raw_entries, id_rw, val_rw),
    }
  }
}

#[derive(Debug)]
pub struct RootL1Node {
  // Same as LDLeaf with sub-tree instead of Leaf!!
  n_elems: usize,
  sub_tree: SubTree,
  rightmost_subtree: Box<Root>,
}

impl RootL1Node {
  fn new(n_elems: usize, sub_tree: SubTree, rightmost_subtree: Root) -> RootL1Node {
    RootL1Node {
      n_elems,
      sub_tree,
      rightmost_subtree: Box::new(rightmost_subtree),
    }
  }
}

impl HasByteSize for RootL1Node {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    self.n_elems * entry_byte_size
      + self.n_elems * self.sub_tree.byte_size(entry_byte_size)
      + self.rightmost_subtree.byte_size(entry_byte_size)
  }
}

impl SubTreeW for RootL1Node {
  fn write<I, V, IRW, VRW, T>(
    &self,
    mut it: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(
      self.byte_size(entry_byte_size),
      dest.len(),
      "Wrong byte size: {} != {}",
      self.byte_size(entry_byte_size),
      dest.len()
    );
    // Same algo as L1Node except that the last element is the righmost-subtree
    let subtree_byte_size = self.sub_tree.byte_size(entry_byte_size);
    let (mut l1_buff, r_buff) = dest.split_at_mut(self.n_elems * entry_byte_size);
    let (mut st_buff, r_buff) = r_buff.split_at_mut(self.n_elems * subtree_byte_size);
    for _ in 0..self.n_elems {
      let (curr_buff, subtree_buff) = st_buff.split_at_mut(subtree_byte_size);
      it = self.sub_tree.write(it, id_rw, val_rw, curr_buff)?;
      st_buff = subtree_buff;
      // Write the current entry
      it.next()
        .ok_or_else(|| Error::new(ErrorKind::Other, "Iterator depleted!"))?
        .write(&mut l1_buff, id_rw, val_rw)?;
    }
    // Plus the rightmost subtree
    it = self.rightmost_subtree.write(it, id_rw, val_rw, r_buff)?;
    assert_eq!(st_buff.len(), 0);
    Ok(it)
  }
}

impl SubTreeR for RootL1Node {
  fn get<I, V, IRW, VRW>(
    &self,
    value: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(
      self.byte_size(entry_byte_size),
      raw_entries.len(),
      "Wrong byte size: {} != {}",
      self.byte_size(entry_byte_size),
      raw_entries.len()
    );
    // Same algo as L1Node except that the last element is the righmost-subtree
    let subtree_byte_size = self.sub_tree.byte_size(entry_byte_size);
    let (l1_buff, r_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let mut l1_entries = RawEntries::new(l1_buff, id_rw, val_rw);
    match l1_entries.binary_search(&value)? {
      Ok(i) => Ok(Some(l1_entries.get_entry(i)?)),
      Err(i) => {
        if i == self.n_elems {
          self
            .rightmost_subtree
            .get(value, &r_buff[i * subtree_byte_size..], id_rw, val_rw)
        } else {
          let from = i * subtree_byte_size;
          let to = from + subtree_byte_size;
          self.sub_tree.get(value, &r_buff[from..to], id_rw, val_rw)
        }
      }
    }
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    mut _visitor: T,
    _raw_entries: &[u8],
    _id_rw: &IRW,
    _val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    unreachable!() // not supposed to be called at the root level
  }

  fn visit<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    debug_assert!(!raw_entries.is_empty());
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(
      self.byte_size(entry_byte_size),
      raw_entries.len(),
      "Wrong byte size: {} != {}",
      self.byte_size(entry_byte_size),
      raw_entries.len()
    );
    // Same algo as L1Node except that the last element is the righmost-subtree
    let subtree_byte_size = self.sub_tree.byte_size(entry_byte_size);
    let (l1_buff, r_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let mut l1_entries = RawEntries::new(l1_buff, id_rw, val_rw);
    let (mut l, mut r) = match l1_entries.binary_search(visitor.center())? {
      Ok(i) => {
        visitor.visit_center(l1_entries.get_entry(i)?);
        if visitor.visit_desc() {
          let from = i * subtree_byte_size;
          let to = from + subtree_byte_size;
          visitor = self
            .sub_tree
            .visit_desc(visitor, &r_buff[from..to], id_rw, val_rw)?;
        }
        if visitor.visit_asc() {
          if i < self.n_elems {
            let from = (i + 1) * subtree_byte_size;
            let to = from + subtree_byte_size;
            visitor = self
              .sub_tree
              .visit_asc(visitor, &r_buff[from..to], id_rw, val_rw)?;
          } else {
            visitor = self.rightmost_subtree.visit_asc(
              visitor,
              &r_buff[i * subtree_byte_size..],
              id_rw,
              val_rw,
            )?;
          }
        }
        (i as i32 - 1, i + 1)
      }
      Err(i) => {
        if i < self.n_elems {
          let from = i * subtree_byte_size;
          let to = from + subtree_byte_size;
          visitor = self
            .sub_tree
            .visit(visitor, &r_buff[from..to], id_rw, val_rw)?;
        } else {
          debug_assert_eq!(i, self.n_elems);
          visitor = self.rightmost_subtree.visit(
            visitor,
            &r_buff[i * subtree_byte_size..],
            id_rw,
            val_rw,
          )?;
        }
        (i as i32 - 1, i)
      }
    };
    while l >= 0 {
      if !visitor.visit_desc() {
        break;
      }
      visitor.visit_le_center(l1_entries.get_entry(l as usize)?);
      if !visitor.visit_desc() {
        break;
      }
      let from = l as usize * subtree_byte_size;
      let to = from + subtree_byte_size;
      visitor = self
        .sub_tree
        .visit_desc(visitor, &r_buff[from..to], id_rw, val_rw)?;
      l -= 1;
    }
    while r < self.n_elems {
      if !visitor.visit_asc() {
        break;
      }
      visitor.visit_he_center(l1_entries.get_entry(r)?);
      if !visitor.visit_asc() {
        break;
      }
      r += 1;
      if r < self.n_elems {
        let from = (r + 1) * subtree_byte_size;
        let to = from + subtree_byte_size;
        visitor = self
          .sub_tree
          .visit_asc(visitor, &r_buff[from..to], id_rw, val_rw)?;
      } else {
        visitor = self.rightmost_subtree.visit_asc(
          visitor,
          &r_buff[r * subtree_byte_size..],
          id_rw,
          val_rw,
        )?;
      }
    }
    Ok(visitor)
  }

  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    mut _visitor: T,
    _raw_entries: &[u8],
    _id_rw: &IRW,
    _val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    unreachable!() // not supposed to be called at the root level
  }
}

#[derive(Debug)]
pub struct RootLDNode {
  n_elems: usize,
  n_l1page_elems: usize,
  sub_tree: LDSubTree,
  rightmost_subtree: Box<Root>,
}

impl RootLDNode {
  fn new(
    n_elems: usize,
    n_l1page_elems: usize,
    sub_tree: LDSubTree,
    rightmost_subtree: Root,
  ) -> RootLDNode {
    RootLDNode {
      n_elems,
      n_l1page_elems,
      sub_tree,
      rightmost_subtree: Box::new(rightmost_subtree),
    }
  }
}

impl HasByteSize for RootLDNode {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    (self.n_elems + self.n_elems * self.n_l1page_elems) * entry_byte_size
      + (self.n_elems * (self.n_l1page_elems + 1)) * self.sub_tree.byte_size(entry_byte_size)
      + self.rightmost_subtree.byte_size(entry_byte_size)
  }
}

impl SubTreeW for RootLDNode {
  fn write<I, V, IRW, VRW, T>(
    &self,
    mut it: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(
      self.byte_size(entry_byte_size),
      dest.len(),
      "Wrong byte size: {} != {}",
      self.byte_size(entry_byte_size),
      dest.len()
    );
    // Same algo as LDNode except that the las element is the rightmost sub-tree
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    // Split the 4 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST][RootSubTree]
    let (mut ld_buff, r_buff) = dest.split_at_mut(self.n_elems * entry_byte_size);
    let (mut l1_buff, r_buff) = r_buff.split_at_mut(self.n_elems * l1page_byte_size);
    let (mut st_buff, r_buff) = r_buff.split_at_mut(self.n_elems * subtree_group_byte_size);
    assert_eq!(
      r_buff.len(),
      self.rightmost_subtree.byte_size(entry_byte_size)
    );
    for _ in 0..self.n_elems {
      // Sub-split the [l1, l1, ..., l1] and [ST, ST, ..., ST] blocks
      let (cl1_buff, tl1_buff) = l1_buff.split_at_mut(l1page_byte_size);
      let (cst_buff, tst_buff) = st_buff.split_at_mut(subtree_group_byte_size);
      it = write_l1page(it, id_rw, val_rw, cl1_buff, &self.sub_tree, cst_buff)?;
      l1_buff = tl1_buff;
      st_buff = tst_buff;
      // Write current entry
      it.next()
        .ok_or_else(|| Error::new(ErrorKind::Other, "Iterator depleted!"))?
        .write(&mut ld_buff, id_rw, val_rw)?;
    }
    // And write the rightmost subtree
    it = self.rightmost_subtree.write(it, id_rw, val_rw, r_buff)?;
    assert_eq!(l1_buff.len(), 0, "Wrong L1 buff size: {}", l1_buff.len());
    assert_eq!(st_buff.len(), 0, "Wrong ST buff size: {}", st_buff.len());
    Ok(it)
  }
}

impl SubTreeR for RootLDNode {
  fn get<I, V, IRW, VRW>(
    &self,
    value: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    // Same algo as LDNode except that the las element is the rightmost sub-tree
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    // Split the 4 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST][RootSubTree]
    let (ld_buff, r_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let mut entries = RawEntries::new(ld_buff, id_rw, val_rw);
    match entries.binary_search(&value)? {
      Ok(i) => Ok(Some(entries.get_entry(i)?)),
      Err(i) => {
        if i == self.n_elems {
          let limit = self.n_elems * (l1page_byte_size + subtree_group_byte_size);
          let (_, r_buff) = r_buff.split_at(limit);
          assert_eq!(
            r_buff.len(),
            self.rightmost_subtree.byte_size(entry_byte_size)
          );
          self.rightmost_subtree.get(value, r_buff, id_rw, val_rw)
        } else {
          let (l1_buff, st_buff) = r_buff.split_at(self.n_elems * l1page_byte_size);
          let from_l1 = i * l1page_byte_size;
          let to_l1 = from_l1 + l1page_byte_size;
          let from_st = i * subtree_group_byte_size;
          let to_st = from_st + subtree_group_byte_size;
          get_l1page(
            value,
            id_rw,
            val_rw,
            &l1_buff[from_l1..to_l1],
            &self.sub_tree,
            &st_buff[from_st..to_st],
          )
        }
      }
    }
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    _visitor: T,
    _raw_entries: &[u8],
    _id_rw: &IRW,
    _val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    unreachable!() // not supposed to be called at the root level
  }

  fn visit<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    // Same algo as LDNode except that the las element is the rightmost sub-tree
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    // Split the 4 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST][RootSubTree]
    let (ld_buff, r_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let (l1_buff, r_buff) = r_buff.split_at(self.n_elems * l1page_byte_size);
    let (st_buff, r_buff) = r_buff.split_at(self.n_elems * subtree_group_byte_size);
    let mut entries = RawEntries::new(ld_buff, id_rw, val_rw);
    let (mut l, mut r) = match entries.binary_search(visitor.center())? {
      Ok(i) => {
        visitor.visit_center(entries.get_entry(i)?);
        if visitor.visit_desc() {
          let from_l1 = i * l1page_byte_size;
          let to_l1 = from_l1 + l1page_byte_size;
          let from_st = i * subtree_group_byte_size;
          let to_st = from_st + subtree_group_byte_size;
          visitor = visit_desc_l1page(
            visitor,
            id_rw,
            val_rw,
            &l1_buff[from_l1..to_l1],
            &self.sub_tree,
            &st_buff[from_st..to_st],
          )?;
        }
        if visitor.visit_asc() {
          if i < self.n_elems {
            let from_l1 = (i + 1) * l1page_byte_size;
            let to_l1 = from_l1 + l1page_byte_size;
            let from_st = (i + 1) * subtree_group_byte_size;
            let to_st = from_st + subtree_group_byte_size;
            visitor = visit_asc_l1page(
              visitor,
              id_rw,
              val_rw,
              &l1_buff[from_l1..to_l1],
              &self.sub_tree,
              &st_buff[from_st..to_st],
            )?;
          } else {
            visitor = self
              .rightmost_subtree
              .visit_asc(visitor, r_buff, id_rw, val_rw)?;
          }
        }
        (i as i32 - 1, i + 1)
      }
      Err(i) => {
        if i < self.n_elems {
          let from_l1 = i * l1page_byte_size;
          let to_l1 = from_l1 + l1page_byte_size;
          let from_st = i * subtree_group_byte_size;
          let to_st = from_st + subtree_group_byte_size;
          visitor = visit_l1page(
            visitor,
            id_rw,
            val_rw,
            &l1_buff[from_l1..to_l1],
            &self.sub_tree,
            &st_buff[from_st..to_st],
          )?;
        } else {
          visitor = self
            .rightmost_subtree
            .visit(visitor, r_buff, id_rw, val_rw)?;
        }
        (i as i32 - 1, i)
      }
    };
    while l >= 0 {
      if !visitor.visit_desc() {
        break;
      }
      visitor.visit_le_center(entries.get_entry(l as usize)?);
      if !visitor.visit_desc() {
        break;
      }
      let from_l1 = l as usize * l1page_byte_size;
      let to_l1 = from_l1 + l1page_byte_size;
      let from_st = l as usize * subtree_group_byte_size;
      let to_st = from_st + subtree_group_byte_size;
      visitor = visit_desc_l1page(
        visitor,
        id_rw,
        val_rw,
        &l1_buff[from_l1..to_l1],
        &self.sub_tree,
        &st_buff[from_st..to_st],
      )?;
      l -= 1;
    }
    while r < self.n_elems {
      if !visitor.visit_asc() {
        break;
      }
      visitor.visit_he_center(entries.get_entry(r)?);
      if !visitor.visit_asc() {
        break;
      }
      r += 1;
      if r < self.n_elems {
        let from_l1 = r * l1page_byte_size;
        let to_l1 = from_l1 + l1page_byte_size;
        let from_st = r * subtree_group_byte_size;
        let to_st = from_st + subtree_group_byte_size;
        visitor = visit_asc_l1page(
          visitor,
          id_rw,
          val_rw,
          &l1_buff[from_l1..to_l1],
          &self.sub_tree,
          &st_buff[from_st..to_st],
        )?;
      } else {
        visitor = self
          .rightmost_subtree
          .visit_asc(visitor, r_buff, id_rw, val_rw)?;
      }
    }
    Ok(visitor)
  }

  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    _visitor: T,
    _raw_entries: &[u8],
    _id_rw: &IRW,
    _val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    unreachable!() // not supposed to be called at the root level
  }
}

#[derive(Debug)]
pub struct L1Leaf {
  n_elems: usize,
}

impl L1Leaf {
  fn new(n_elems: usize) -> L1Leaf {
    L1Leaf { n_elems }
  }
}

impl HasByteSize for L1Leaf {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    self.n_elems * entry_byte_size
  }
}

impl SubTreeW for L1Leaf {
  fn write<I, V, IRW, VRW, T>(
    &self,
    mut it: T,
    id_rw: &IRW,
    val_rw: &VRW,
    mut dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(
      self.byte_size(entry_byte_size),
      dest.len(),
      "Wrong byte size: {} != {}",
      self.byte_size(entry_byte_size),
      dest.len()
    );
    for _ in 0..self.n_elems {
      it.next()
        .ok_or_else(|| Error::new(ErrorKind::Other, "Iterator depleted!"))?
        .write(&mut dest, id_rw, val_rw)?;
    }
    assert_eq!(dest.len(), 0);
    Ok(it)
  }
}

impl SubTreeR for L1Leaf {
  fn get<I, V, IRW, VRW>(
    &self,
    val: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    debug_assert_eq!(
      self.byte_size(id_rw.n_bytes() + val_rw.n_bytes()),
      raw_entries.len()
    );
    let mut entries = RawEntries::new(raw_entries, id_rw, val_rw);
    entries
      .binary_search(&val)?
      .ok()
      .map(|i| entries.get_entry(i))
      .transpose()
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    debug_assert_eq!(
      self.byte_size(id_rw.n_bytes() + val_rw.n_bytes()),
      raw_entries.len()
    );
    debug_assert!(visitor.visit_desc());
    let mut entries = RawEntries::new(raw_entries, id_rw, val_rw);
    for i in (0..self.n_elems).rev() {
      visitor.visit_le_center(entries.get_entry(i)?);
      if !visitor.visit_desc() {
        return Ok(visitor);
      }
    }
    Ok(visitor)
  }

  fn visit<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    debug_assert_eq!(
      self.byte_size(id_rw.n_bytes() + val_rw.n_bytes()),
      raw_entries.len()
    );
    let mut entries = RawEntries::new(raw_entries, id_rw, val_rw);
    let (mut l, mut r) = match entries.binary_search(visitor.center())? {
      Ok(i) => {
        visitor.visit_center(entries.get_entry(i)?);
        (i as i32 - 1, i + 1)
      }
      Err(i) => (i as i32 - 1, i),
    };
    // Visit left part if needed
    while l >= 0 && visitor.visit_desc() {
      visitor.visit_le_center(entries.get_entry(l as usize)?);
      l -= 1;
    }
    // Visit right part if needed
    while r < self.n_elems && visitor.visit_asc() {
      visitor.visit_he_center(entries.get_entry(r)?);
      r += 1;
    }
    Ok(visitor)
  }

  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    debug_assert_eq!(
      self.byte_size(id_rw.n_bytes() + val_rw.n_bytes()),
      raw_entries.len()
    );
    debug_assert!(visitor.visit_asc());
    let mut entries = RawEntries::new(raw_entries, id_rw, val_rw);
    for i in 0..self.n_elems {
      visitor.visit_he_center(entries.get_entry(i)?);
      if !visitor.visit_asc() {
        return Ok(visitor);
      }
    }
    Ok(visitor)
  }
}

#[derive(Debug)]
pub struct L1Node {
  // Only the root can be a L1Node
  n_elems: usize,
  sub_tree: Box<SubTree>, // Like LDLeaf with leaf being a sub-tree
}

impl L1Node {
  fn new(n_elems: usize, sub_tree: SubTree) -> L1Node {
    L1Node {
      n_elems,
      sub_tree: Box::new(sub_tree),
    }
  }
}

impl HasByteSize for L1Node {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    self.n_elems * entry_byte_size + (self.n_elems + 1) * self.sub_tree.byte_size(entry_byte_size)
  }
}

impl SubTreeW for L1Node {
  fn write<I, V, IRW, VRW, T>(
    &self,
    mut it: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(
      self.byte_size(entry_byte_size),
      dest.len(),
      "Wrong buffer size"
    );
    let (l1_buff, st_buff) = dest.split_at_mut(self.n_elems * entry_byte_size);
    it = write_l1page(it, id_rw, val_rw, l1_buff, self.sub_tree.as_ref(), st_buff)?;
    Ok(it)
  }
}

impl SubTreeR for L1Node {
  fn get<I, V, IRW, VRW>(
    &self,
    val: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    debug_assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    let (l1_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    get_l1page(val, id_rw, val_rw, l1_buff, self.sub_tree.as_ref(), st_buff)
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    debug_assert!(visitor.visit_desc());
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    debug_assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    let (l1_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    visit_desc_l1page(
      visitor,
      id_rw,
      val_rw,
      l1_buff,
      self.sub_tree.as_ref(),
      st_buff,
    )
  }

  fn visit<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    debug_assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    let (l1_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    visit_l1page(
      visitor,
      id_rw,
      val_rw,
      l1_buff,
      self.sub_tree.as_ref(),
      st_buff,
    )
  }

  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    debug_assert!(visitor.visit_asc());
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    debug_assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    let (l1_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    visit_asc_l1page(
      visitor,
      id_rw,
      val_rw,
      l1_buff,
      self.sub_tree.as_ref(),
      st_buff,
    )
  }
}

#[derive(Debug)]
pub struct LDNode {
  n_elems: usize,
  n_l1page_elems: usize,
  sub_tree: Box<LDSubTree>,
}

impl LDNode {
  fn new(n_elems: usize, n_l1page_elems: usize, sub_tree: LDSubTree) -> LDNode {
    LDNode {
      n_elems,
      n_l1page_elems,
      sub_tree: Box::new(sub_tree),
    }
  }
}

impl HasByteSize for LDNode {
  fn byte_size(&self, entry_byte_size: usize) -> usize {
    self.n_elems * entry_byte_size
      + (self.n_elems + 1) * self.n_l1page_elems * entry_byte_size
      + (self.n_elems + 1) * (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size)
  }
}

impl SubTreeW for LDNode {
  fn write<I, V, IRW, VRW, T>(
    &self,
    mut it: T,
    id_rw: &IRW,
    val_rw: &VRW,
    dest: &mut [u8],
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Iterator<Item = Entry<I, V>>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(
      self.byte_size(entry_byte_size),
      dest.len(),
      "Wrong byte size: {} != {}",
      self.byte_size(entry_byte_size),
      dest.len()
    );
    // Split the 3 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST]
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    let (mut ld_buff, st_buff) = dest.split_at_mut(self.n_elems * entry_byte_size);
    let (mut l1_buff, mut st_buff) = st_buff.split_at_mut((self.n_elems + 1) * l1page_byte_size);
    assert_eq!(st_buff.len(), (self.n_elems + 1) * subtree_group_byte_size);
    for _ in 0..self.n_elems {
      // Sub-split the [l1, l1, ..., l1] and [ST, ST, ..., ST] blocks
      let (cl1_buff, tl1_buff) = l1_buff.split_at_mut(l1page_byte_size);
      let (cst_buff, tst_buff) = st_buff.split_at_mut(subtree_group_byte_size);
      it = write_l1page(
        it,
        id_rw,
        val_rw,
        cl1_buff,
        self.sub_tree.as_ref(),
        cst_buff,
      )?;
      l1_buff = tl1_buff;
      st_buff = tst_buff;
      // Write the current entry
      it.next()
        .ok_or_else(|| Error::new(ErrorKind::Other, "Iterator depleted!"))?
        .write(&mut ld_buff, id_rw, val_rw)?;
    }
    // Write the last sub-tree
    it = write_l1page(it, id_rw, val_rw, l1_buff, self.sub_tree.as_ref(), st_buff)?;
    assert_eq!(ld_buff.len(), 0);
    Ok(it)
  }
}

impl SubTreeR for LDNode {
  fn get<I, V, IRW, VRW>(
    &self,
    val: V,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<Option<Entry<I, V>>, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    // Split the 3 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST]
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    let (ld_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let mut entries = RawEntries::new(ld_buff, id_rw, val_rw);
    match entries.binary_search(&val)? {
      Ok(i) => Ok(Some(entries.get_entry(i)?)),
      Err(i) => {
        let (l1_buff, st_buff) = st_buff.split_at((self.n_elems + 1) * l1page_byte_size);
        let from_l1 = i * l1page_byte_size;
        let to_l1 = from_l1 + l1page_byte_size;
        let from_st = i * subtree_group_byte_size;
        let to_st = from_st + subtree_group_byte_size;
        get_l1page(
          val,
          id_rw,
          val_rw,
          &l1_buff[from_l1..to_l1],
          self.sub_tree.as_ref(),
          &st_buff[from_st..to_st],
        )
      }
    }
  }

  fn visit_desc<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    // Split the 3 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST]
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    let (_ld_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let (l1_buff, st_buff) = st_buff.split_at((self.n_elems + 1) * l1page_byte_size);
    // let mut entries = RawEntries::new(ld_buff, id_rw, val_rw);

    let from_l1 = self.n_elems * l1page_byte_size;
    let to_l1 = from_l1 + l1page_byte_size;
    let from_st = self.n_elems * subtree_group_byte_size;
    let to_st = from_st + subtree_group_byte_size;
    visitor = visit_desc_l1page(
      visitor,
      id_rw,
      val_rw,
      &l1_buff[from_l1..to_l1],
      self.sub_tree.as_ref(),
      &st_buff[from_st..to_st],
    )?;
    for i in (0..self.n_elems).rev() {
      let from_l1 = i * l1page_byte_size;
      let to_l1 = from_l1 + l1page_byte_size;
      let from_st = i * subtree_group_byte_size;
      let to_st = from_st + subtree_group_byte_size;
      visitor = visit_desc_l1page(
        visitor,
        id_rw,
        val_rw,
        &l1_buff[from_l1..to_l1],
        self.sub_tree.as_ref(),
        &st_buff[from_st..to_st],
      )?;
    }
    Ok(visitor)
  }
  fn visit<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    // Split the 3 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST]
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    let (ld_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let (l1_buff, st_buff) = st_buff.split_at((self.n_elems + 1) * l1page_byte_size);
    let mut entries = RawEntries::new(ld_buff, id_rw, val_rw);
    let (mut l, mut r) = match entries.binary_search(visitor.center())? {
      Ok(i) => {
        visitor.visit_center(entries.get_entry(i)?);
        if visitor.visit_desc() {
          let from_l1 = i * l1page_byte_size;
          let to_l1 = from_l1 + l1page_byte_size;
          let from_st = i * subtree_group_byte_size;
          let to_st = from_st + subtree_group_byte_size;
          visitor = visit_desc_l1page(
            visitor,
            id_rw,
            val_rw,
            &l1_buff[from_l1..to_l1],
            self.sub_tree.as_ref(),
            &st_buff[from_st..to_st],
          )?;
        }
        if visitor.visit_asc() {
          let from_l1 = (i + 1) * l1page_byte_size;
          let to_l1 = from_l1 + l1page_byte_size;
          let from_st = (i + 1) * subtree_group_byte_size;
          let to_st = from_st + subtree_group_byte_size;
          visitor = visit_asc_l1page(
            visitor,
            id_rw,
            val_rw,
            &l1_buff[from_l1..to_l1],
            self.sub_tree.as_ref(),
            &st_buff[from_st..to_st],
          )?;
        }
        (i as i32 - 1, i + 1)
      }
      Err(i) => {
        let from_l1 = i * l1page_byte_size;
        let to_l1 = from_l1 + l1page_byte_size;
        let from_st = i * subtree_group_byte_size;
        let to_st = from_st + subtree_group_byte_size;
        visitor = visit_l1page(
          visitor,
          id_rw,
          val_rw,
          &l1_buff[from_l1..to_l1],
          self.sub_tree.as_ref(),
          &st_buff[from_st..to_st],
        )?;
        (i as i32 - 1, i)
      }
    };
    while l >= 0 {
      if !visitor.visit_desc() {
        break;
      }
      visitor.visit_le_center(entries.get_entry(l as usize)?);
      if !visitor.visit_desc() {
        break;
      }
      let from_l1 = l as usize * l1page_byte_size;
      let to_l1 = from_l1 + l1page_byte_size;
      let from_st = l as usize * subtree_group_byte_size;
      let to_st = from_st + subtree_group_byte_size;
      visitor = visit_desc_l1page(
        visitor,
        id_rw,
        val_rw,
        &l1_buff[from_l1..to_l1],
        self.sub_tree.as_ref(),
        &st_buff[from_st..to_st],
      )?;
      l -= 1;
    }
    while r < self.n_elems {
      if !visitor.visit_asc() {
        break;
      }
      visitor.visit_he_center(entries.get_entry(r)?);
      if !visitor.visit_asc() {
        break;
      }
      let from_l1 = (r + 1) * l1page_byte_size;
      let to_l1 = from_l1 + l1page_byte_size;
      let from_st = (r + 1) * subtree_group_byte_size;
      let to_st = from_st + subtree_group_byte_size;
      visitor = visit_asc_l1page(
        visitor,
        id_rw,
        val_rw,
        &l1_buff[from_l1..to_l1],
        self.sub_tree.as_ref(),
        &st_buff[from_st..to_st],
      )?;
      r += 1;
    }
    Ok(visitor)
  }
  fn visit_asc<I, V, IRW, VRW, T>(
    &self,
    mut visitor: T,
    raw_entries: &[u8],
    id_rw: &IRW,
    val_rw: &VRW,
  ) -> Result<T, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: Visitor<I = I, V = V>,
  {
    let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
    assert_eq!(self.byte_size(entry_byte_size), raw_entries.len());
    // Split the 3 blocs [ld][l1, l1, ..., l1][ST, ST, ..., ST]
    let l1page_byte_size = self.n_l1page_elems * entry_byte_size;
    let subtree_group_byte_size =
      (self.n_l1page_elems + 1) * self.sub_tree.byte_size(entry_byte_size);
    let (_ld_buff, st_buff) = raw_entries.split_at(self.n_elems * entry_byte_size);
    let (l1_buff, st_buff) = st_buff.split_at((self.n_elems + 1) * l1page_byte_size);
    // let mut entries = RawEntries::new(ld_buff, id_rw, val_rw);

    visitor = visit_asc_l1page(
      visitor,
      id_rw,
      val_rw,
      &l1_buff[0..l1page_byte_size],
      self.sub_tree.as_ref(),
      &st_buff[0..subtree_group_byte_size],
    )?;
    for i in 1..=self.n_elems {
      let from_l1 = i * l1page_byte_size;
      let to_l1 = from_l1 + l1page_byte_size;
      let from_st = i * subtree_group_byte_size;
      let to_st = from_st + subtree_group_byte_size;
      visitor = visit_asc_l1page(
        visitor,
        id_rw,
        val_rw,
        &l1_buff[from_l1..to_l1],
        self.sub_tree.as_ref(),
        &st_buff[from_st..to_st],
      )?;
    }
    Ok(visitor)
  }
}

///
/// # Remark:
/// A LD Leaf can be considered as a L1 page (with a small number of entries) having L1 pages
/// as sub-tree. In this particular case, `offset_to_subtree` = `l1page_byte_size`.
///
/// # Args
/// * `dest`: slice containing a group of L1 pages (or a single L1 page) followed by sub-trees.
fn write_l1page<I, V, IRW, VRW, S, T>(
  mut it: T,
  id_rw: &IRW,
  val_rw: &VRW,
  mut l1_buff: &mut [u8],
  sub_tree: &S,
  mut subtree_buff: &mut [u8],
) -> Result<T, Error>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
  S: SubTreeW,
  T: Iterator<Item = Entry<I, V>>,
{
  assert!(!l1_buff.is_empty());
  let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
  let subtree_byte_size = sub_tree.byte_size(entry_byte_size);
  let n_l1 = l1_buff.len() / entry_byte_size;
  assert_eq!(
    l1_buff.len(),
    n_l1 * entry_byte_size,
    "Wrong L1 buff size: {} != {}",
    l1_buff.len(),
    n_l1 * entry_byte_size
  );
  assert_eq!(
    subtree_buff.len(),
    (n_l1 + 1) * subtree_byte_size,
    "Wrong SubTree buff size: {} != {}",
    subtree_buff.len(),
    (n_l1 + 1) * subtree_byte_size
  );
  for _ in 0..n_l1 {
    let (curr_buff, st_buff) = subtree_buff.split_at_mut(subtree_byte_size);
    it = sub_tree.write(it, id_rw, val_rw, curr_buff)?;
    subtree_buff = st_buff;
    it.next()
      .ok_or_else(|| Error::new(ErrorKind::Other, "Iterator depleted!"))?
      .write(&mut l1_buff, id_rw, val_rw)?;
  }
  it = sub_tree.write(it, id_rw, val_rw, subtree_buff)?;
  assert!(l1_buff.is_empty());
  Ok(it)
}

fn get_l1page<I, V, IRW, VRW, S>(
  val: V,
  id_rw: &IRW,
  val_rw: &VRW,
  l1_buff: &[u8],
  sub_tree: &S,
  subtree_buff: &[u8],
) -> Result<Option<Entry<I, V>>, Error>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
  S: SubTreeR,
{
  assert!(!l1_buff.is_empty());
  let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
  let subtree_byte_size = sub_tree.byte_size(entry_byte_size);
  let n_l1 = l1_buff.len() / entry_byte_size;
  assert_eq!(l1_buff.len(), n_l1 * entry_byte_size);
  assert_eq!(subtree_buff.len(), (n_l1 + 1) * subtree_byte_size);
  let mut l1_entries = RawEntries::new(l1_buff, id_rw, val_rw);
  match l1_entries.binary_search(&val)? {
    Ok(i) => Ok(Some(l1_entries.get_entry(i)?)),
    Err(i) => {
      let from = i * subtree_byte_size;
      let to = from + subtree_byte_size;
      sub_tree.get(val, &subtree_buff[from..to], id_rw, val_rw)
    }
  }
}

fn visit_l1page<I, V, IRW, VRW, S, T>(
  mut visitor: T,
  id_rw: &IRW,
  val_rw: &VRW,
  l1_buff: &[u8],
  sub_tree: &S,
  subtree_buff: &[u8],
) -> Result<T, Error>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
  S: SubTreeR,
  T: Visitor<I = I, V = V>,
{
  assert!(!l1_buff.is_empty());
  let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
  let subtree_byte_size = sub_tree.byte_size(entry_byte_size);
  let n_l1 = l1_buff.len() / entry_byte_size;
  assert_eq!(l1_buff.len(), n_l1 * entry_byte_size);
  assert_eq!(subtree_buff.len(), (n_l1 + 1) * subtree_byte_size);
  let mut l1_entries = RawEntries::new(l1_buff, id_rw, val_rw);
  let (mut l, mut r) = match l1_entries.binary_search(visitor.center())? {
    Ok(i) => {
      visitor.visit_center(l1_entries.get_entry(i)?);
      if visitor.visit_desc() {
        let from = i * subtree_byte_size;
        let to = from + subtree_byte_size;
        visitor = sub_tree.visit_desc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
      }
      if visitor.visit_asc() {
        let from = (i + 1) * subtree_byte_size;
        let to = from + subtree_byte_size;
        visitor = sub_tree.visit_asc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
      }
      (i as i32 - 1, i + 1)
    }
    Err(i) => {
      let from = i * subtree_byte_size;
      let to = from + subtree_byte_size;
      visitor = sub_tree.visit(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
      (i as i32 - 1, i)
    }
  };
  while l >= 0 {
    if !visitor.visit_desc() {
      break;
    }
    visitor.visit_le_center(l1_entries.get_entry(l as usize)?);
    if !visitor.visit_desc() {
      break;
    }
    let from = l as usize * subtree_byte_size;
    let to = from + subtree_byte_size;
    visitor = sub_tree.visit_desc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
    l -= 1;
  }
  while r < n_l1 {
    if !visitor.visit_asc() {
      break;
    }
    visitor.visit_he_center(l1_entries.get_entry(r)?);
    if !visitor.visit_asc() {
      break;
    }
    let from = (r + 1) * subtree_byte_size;
    let to = from + subtree_byte_size;
    visitor = sub_tree.visit_asc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
    r += 1;
  }
  Ok(visitor)
}

fn visit_desc_l1page<I, V, IRW, VRW, S, T>(
  mut visitor: T,
  id_rw: &IRW,
  val_rw: &VRW,
  l1_buff: &[u8],
  sub_tree: &S,
  subtree_buff: &[u8],
) -> Result<T, Error>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
  S: SubTreeR,
  T: Visitor<I = I, V = V>,
{
  assert!(!l1_buff.is_empty());
  let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
  let subtree_byte_size = sub_tree.byte_size(entry_byte_size);
  let n_l1 = l1_buff.len() / entry_byte_size;
  assert_eq!(l1_buff.len(), n_l1 * entry_byte_size);
  assert_eq!(subtree_buff.len(), (n_l1 + 1) * subtree_byte_size);
  let mut l1_entries = RawEntries::new(l1_buff, id_rw, val_rw);
  let from = n_l1 * subtree_byte_size;
  let to = from + subtree_byte_size;
  visitor = sub_tree.visit_desc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
  let mut i = 0;
  while i < n_l1 && visitor.visit_desc() {
    visitor.visit_le_center(l1_entries.get_entry(i)?);
    if !visitor.visit_desc() {
      break;
    }
    let from = i * subtree_byte_size;
    let to = from + subtree_byte_size;
    visitor = sub_tree.visit_desc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
    i += 1;
  }
  Ok(visitor)
}

fn visit_asc_l1page<I, V, IRW, VRW, S, T>(
  mut visitor: T,
  id_rw: &IRW,
  val_rw: &VRW,
  l1_buff: &[u8],
  sub_tree: &S,
  subtree_buff: &[u8],
) -> Result<T, Error>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
  S: SubTreeR,
  T: Visitor<I = I, V = V>,
{
  assert!(!l1_buff.is_empty());
  let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
  let subtree_byte_size = sub_tree.byte_size(entry_byte_size);
  let n_l1 = l1_buff.len() / entry_byte_size;
  assert_eq!(l1_buff.len(), n_l1 * entry_byte_size);
  assert_eq!(subtree_buff.len(), (n_l1 + 1) * subtree_byte_size);
  let mut l1_entries = RawEntries::new(l1_buff, id_rw, val_rw);
  let mut i = 0;
  while i < n_l1 {
    let from = i * subtree_byte_size;
    let to = from + subtree_byte_size;
    visitor = sub_tree.visit_asc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
    if !visitor.visit_asc() {
      break;
    }
    visitor.visit_he_center(l1_entries.get_entry(i)?);
    if !visitor.visit_asc() {
      break;
    }
    i += 1;
  }
  if i == n_l1 {
    let from = i * subtree_byte_size;
    let to = from + subtree_byte_size;
    visitor = sub_tree.visit_asc(visitor, &subtree_buff[from..to], id_rw, val_rw)?;
  }
  Ok(visitor)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BSTreeMeta {
  pub types: IdVal,
  constants: BSTreeConstants,
  pub layout: BSTreeLayout,
}

impl BSTreeMeta {
  fn from(
    types: IdVal,
    n_entries: usize,
    entry_byte_size: usize,
    l1_byte_size: usize,
    ld_byte_size: usize,
  ) -> BSTreeMeta {
    let constants = BSTreeConstants::new(n_entries, entry_byte_size, l1_byte_size, ld_byte_size);
    let layout = BSTreeLayout::new(&constants);
    BSTreeMeta {
      types,
      constants,
      layout,
    }
  }

  pub fn get_root(&self) -> Root {
    self.layout.get_root(&self.constants)
  }

  /*fn get_data_byte_size(&self) -> usize {
    (self.constants.n_entries * (self.constants.entry_byte_size as u64)) as usize
  }*/
}

#[derive(Debug, Serialize, Deserialize)]
struct BSTreeConstants {
  /// Total number of entries in the tree
  n_entries: u64,
  /// Number of bytes used to store a single entry.
  entry_byte_size: u8,
  /// Number of entries (`nL1`) per L1-D block (i.e. per memory page).
  /// For performances reasons, `nL1` time the size of an entry (in bytes) must be equals to
  /// or lower than the L1-D cache size.
  n_entries_per_l1page: u16,
  /// Number of L1-D blocks per LD block (`nL1InLD`).
  /// A LD (D for Disk) block is supposed to fit into the HDD cache
  /// (using SSDs, we could have only considered L1-D blocks).
  /// A LD block contains `nL1InLD - 1` entries plus the `nL1InLD * nL1` entries in the L1 pages.
  /// Thus, the total number of entries in a LD block is `nLD = (nL1InLD - 1 + nL1InLD * nL1`
  n_l1page_per_ldpage: u16,
}

impl BSTreeConstants {
  /// * `n_entries`: total number of entries in the tree.
  /// * `entry_byte_size`: e.g. for (kev, value) = (u64, f64), the entry byte size typically = 16
  /// * `l1_byte_size`: L1-D cache size in bytes, a typical value is 32,768 (i.e. `32 KB`).
  fn new(
    n_entries: usize,
    entry_byte_size: usize,
    l1_byte_size: usize,
    ld_byte_size: usize,
  ) -> BSTreeConstants {
    let n_entries_per_l1page = l1_byte_size / entry_byte_size;
    let n_entries_per_ldpage_max = ld_byte_size / entry_byte_size;
    // nLD = number of entries per LD page
    //     = (nL1InLD - 1) + nL1InLD * nL1
    //     = nL1InLD * (nL1 + 1) - 1
    //    <= nLDmax
    // => nL1InLD <= (nLDmax + 1) / (nL1 + 1)
    let n_l1page_per_ldpage = (n_entries_per_ldpage_max + 1) / (n_entries_per_l1page + 1);
    BSTreeConstants {
      n_entries: n_entries as u64,
      entry_byte_size: entry_byte_size as u8,
      n_entries_per_l1page: n_entries_per_l1page as u16, // : l1_byte_size as u16,
      n_l1page_per_ldpage: n_l1page_per_ldpage as u16,   //: ld_byte_size as u16
    }
  }

  /*
  fn l1_byte_size(&self) -> usize {
    self.entry_byte_size * self.n_entries_per_l1page
  }

  fn ld_byte_size(&self) -> usize {
    self.entry_byte_size * self.n_entries_per_ldpage()
  }

  fn n_entries_per_ldpage(&self) -> usize {
    (self.n_l1page_per_ldpage - 1) + self.n_l1page_per_ldpage * self.n_entries_per_l1page
  }*/
}

///
/// * The depth alternates between the inter L1 blocks values in a LD blocks and the L1 blocks
/// * One LD blocks is made of 2 depths:
///     + depth 0: the `n` inter L1 blocks values
///     + depth 1: the `n + 1` L1 blocks values
/// * The deepest depth is always made of L1 blocks
/// * The number of elements l1 and ld are fixed, except in the root.
/// * It means that
///     + Tree of depth 0: (l1)
///         - depth = 0, one L1 block
///     + Tree of depth 1: (LD) = (ld)(l1...l1)
///         - depth = 0: one LD block (number of elements max = number of elements in a L1 block)
///         - depth = 1: L1 blocks
///     + Tree of depth 2: (l1)/(LD...LD) = (l1)(LD...lD) = (l1)( (ld)(l1...l1)...(ld)(l1...l1) )
///         - depth = 0: one L1 block
///         - depth = 1: LD blocks
///         - depth = 2: L1 blocks
///     + Tree of depth 3: (LD)/(LD...LD) = (ld)(l1...l1)( (ld)(l1...l1)...(ld)(l1...l1) )
///         - depth = 0: one LD block (number of elements max = number of elements in a L1 block)
///         - depth = 1: L1 blocks pointing to LD blocks
///         - depth = 2: LD blocks pointing to L1 blocks
///         - depth = 3: L1 blocks
///     + Tree of depth 4: (L1)( ((LD)/(LD...LD))...((LD)/(LD...LD)) ) = ...
///         - depth = 0: one L1 block
///         - depth = 1: one LD block
///         - depth = 2: one L1 block pointing to LD blocks
///         - depth = 3: one LD blocks pointing to LD blocks
///         - depth = 4: one L1 blocks pointing to LD blocks pointing to LD blocks
///     + ...
#[derive(Debug, Serialize, Deserialize)]
pub struct BSTreeLayout {
  /// Depth of the tree (a tree made of a single root has a depth = 0).
  /// * If the depth is even (2, 4), the root points to LD blocks
  ///     + the decision to put (or not) everything in memory depends on the total size
  /// * If the depth is idd (1, 3), the root points to L1 blocks
  ///     + the decision to put (or not) everything in memory also depends on the total size
  depth: u8,
  /// Number of entries in the root array
  n_entries_root: u16,
  /// Number of entries in the regular part of the tree,
  /// i.e. in the full tree, including the root, minus the rightmost sub-tree (if any).
  n_entries_main: u64,
  /// The number of elements in the right-most sub-tree is `n_entries - n_entries_main` and it depth
  /// is at most equals to `self.depth - 1`.
  rigthmost_subtree: Option<Box<BSTreeLayout>>,
}

impl BSTreeLayout {
  fn new(cte: &BSTreeConstants) -> BSTreeLayout {
    BSTreeLayout::from(cte.n_entries, cte)
  }

  fn from(n_entries: u64, cte: &BSTreeConstants) -> BSTreeLayout {
    let n_l1 = cte.n_entries_per_l1page as u64;
    let n_ld_elem = cte.n_l1page_per_ldpage as u64 - 1;
    let n_ld = n_ld_elem + (n_ld_elem + 1) * n_l1;
    // L1
    if n_entries <= n_l1 {
      return BSTreeLayout {
        depth: 0,
        n_entries_root: n_entries as u16,
        n_entries_main: n_entries,
        rigthmost_subtree: None,
      };
    }
    // Test if a single LD block containing max (n_entries_per_l1page + 1) sub-elements is enough
    // L1 -> L1
    let mut n_sub = n_l1;
    if n_entries <= n_l1 + (n_l1 + 1) * n_sub {
      return BSTreeLayout::from_known_depth(1, n_entries, n_sub, cte);
    }
    n_sub = n_ld;
    // Else continue ... (we put a hard limit on the maximum depth).
    for depth in (2..=8).step_by(2) {
      // Transforms L1 -> L1 (-> ...) into L1 -> LD (-> ...)
      if n_entries <= n_l1 + (n_l1 + 1) * n_sub {
        return BSTreeLayout::from_known_depth(depth, n_entries, n_sub, cte);
      }
      n_sub = n_l1 + (n_l1 + 1) * n_sub;
      // Transforms L1 -> LD (-> ...) into L1 -> L1 -> LD (-> ...)
      if n_entries <= n_l1 + (n_l1 + 1) * n_sub {
        return BSTreeLayout::from_known_depth(depth + 1, n_entries, n_sub, cte);
      }
      n_sub = n_ld_elem + (n_ld_elem + 1) * n_sub;
    }
    // If you this point is reached, there is a problem somewhere (entry size in bytes, ...)
    panic!("Too deep tree. Check your inputs (entry size in bytes, ...).");
  }

  /// * `n_subtree`: number of entries in each sub-tree starting a depth (depth + 1).
  fn from_known_depth(
    depth: u8,
    n_entries: u64,
    n_subtree: u64,
    cte: &BSTreeConstants,
  ) -> BSTreeLayout {
    // nE <= nR + (nR + 1) * nSub
    // => nE - nSub <= nR * (1 + nSub)
    // => nR >= (nE - nSub) / (1 + nSub)
    let n_root = (n_entries - n_subtree) / (1 + n_subtree);
    let n_rem = n_entries - (n_root + (n_root + 1) * n_subtree);
    assert!(n_root as u16 <= cte.n_entries_per_l1page);
    assert!(n_root as u16 <= cte.n_entries_per_l1page);
    if n_rem == 0 {
      // Very unlikely!
      BSTreeLayout {
        depth,
        n_entries_root: n_root as u16,
        n_entries_main: n_entries,
        rigthmost_subtree: None,
      }
    } else {
      let n_entries_main = (n_root + 1) + (n_root + 1) * n_subtree;
      let n_entries_sub = n_entries - n_entries_main;
      BSTreeLayout {
        depth,
        n_entries_root: n_root as u16 + 1,
        n_entries_main,
        rigthmost_subtree: Some(Box::new(BSTreeLayout::from(n_entries_sub, cte))),
      }
    }
  }

  // d = 0; L1
  // d = 1; L1 -> L1
  // d = 2; L1 -> LD
  // d = 3; LD -> LD
  // d = 4; L1 -> LD -> LD
  fn get_root(&self, cte: &BSTreeConstants) -> Root {
    match (self.depth, self.depth & 1, self.rigthmost_subtree.as_ref()) {
      // Depth 0
      (0, _, _) => Root::L1Leaf(L1Leaf::new(self.n_entries_root as usize)),
      // Depth 1
      (1, _, None) => Root::L1Node(
        // Used as a LDLeaf
        L1Node::new(self.n_entries_root as usize, self.get_subtree(1, cte)),
      ),
      (1, _, Some(sub_layout)) => Root::RootL1Node(
        // Used as a LDLeaf
        RootL1Node::new(
          self.n_entries_root as usize,
          self.get_subtree(1, cte),
          sub_layout.get_root(cte),
        ),
      ),
      // Other depth
      // - unlikely cases
      (_, 0, None) => Root::L1Node(L1Node::new(
        self.n_entries_root as usize,
        self.get_subtree(1, cte),
      )),
      (_, 1, None) => Root::LDNode(LDNode::new(
        self.n_entries_root as usize,
        cte.n_entries_per_l1page as usize,
        self.get_ld_subtree(2, cte),
      )),
      // - frequent cases
      (_, 0, Some(sub_layout)) => Root::RootL1Node(RootL1Node::new(
        self.n_entries_root as usize,
        self.get_subtree(1, cte),
        sub_layout.get_root(cte),
      )),
      (_, 1, Some(sub_layout)) => Root::RootLDNode(RootLDNode::new(
        self.n_entries_root as usize,
        cte.n_entries_per_l1page as usize,
        self.get_ld_subtree(2, cte),
        sub_layout.get_root(cte),
      )),
      (_, _, _) => unreachable!(),
    }
  }

  fn get_subtree(&self, d: u8, cte: &BSTreeConstants) -> SubTree {
    if d == self.depth {
      SubTree::L1Leaf(L1Leaf::new(cte.n_entries_per_l1page as usize))
    } else if d == (self.depth - 1) {
      SubTree::L1Node(L1Node::new(
        // Used as a LDLeaf
        cte.n_l1page_per_ldpage as usize - 1,
        self.get_subtree(d + 1, cte),
      ))
    } else {
      SubTree::LDNode(LDNode::new(
        cte.n_l1page_per_ldpage as usize - 1,
        cte.n_entries_per_l1page as usize,
        self.get_ld_subtree(d + 2, cte),
      ))
    }
  }

  fn get_ld_subtree(&self, d: u8, cte: &BSTreeConstants) -> LDSubTree {
    assert!(d < self.depth);
    if d == (self.depth - 1) {
      LDSubTree::L1Node(L1Node::new(
        // Used as a LDLeaf
        cte.n_l1page_per_ldpage as usize - 1,
        self.get_subtree(d + 1, cte),
      ))
    } else {
      LDSubTree::LDNode(LDNode::new(
        cte.n_l1page_per_ldpage as usize - 1,
        cte.n_entries_per_l1page as usize,
        self.get_ld_subtree(d + 2, cte),
      ))
    }
  }
}

///
/// # Args
/// * `output_file`: file that will store the tree
/// * `mem_args`: tree characteristics
/// * `n_entries`: number of entries the iterator contains (we may try to rely on size_int?)
/// * `entries_iterator`: entries to be stored in the tree, must be sorted
/// * `id_rw`: object allowing to read and write the identifier part of an entry
/// * `val_rw`: object allowing to read and write the value part of an entry
///
/// # Panic
/// * Panics if the entries in the input iterator are not ordered with respect to their values
// WE SHOULE IMPLEMENT IdRW(ReadWrite) and ValReadWrite(ReadWrite) with methods get_id_type() and get_val_type() respectively,
// not to have to pass 'types' in parameters (added to write the metadata!)
#[cfg(not(target_arch = "wasm32"))]
pub fn build<I, V, IRW, VRW, T>(
  output_file: PathBuf,
  mem_args: &MemSizeArgs,
  n_entries: usize,
  entries_iterator: T,
  types: &IdVal,
  id_rw: &IRW,
  val_rw: &VRW,
) -> Result<(), Error>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
  T: Iterator<Item = Entry<I, V>>,
{
  // KMerge<TmpFileIter<'a, I, V, IRW, VRW>>

  // Decorate with an iterator that ensure that the input iterator is sorted?
  let entry_byte_size = id_rw.n_bytes() + val_rw.n_bytes();
  let meta = dbg!(BSTreeMeta::from(
    types.clone(),
    n_entries,
    entry_byte_size,
    mem_args.l1_byte_size(),
    mem_args.disk_byte_size()
  ));
  let encoded_meta: Vec<u8> = bincode::serialize(&meta).unwrap();
  // Open file
  let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .open(output_file)?;
  // dbg!(File::create(&output_file))?;
  let before_meta_len = FILE_TYPE.len() + 3 + 2;
  let data_starting_byte = before_meta_len + encoded_meta.len();
  let file_byte_size = data_starting_byte + n_entries * entry_byte_size;
  // Reserve space
  file.set_len(file_byte_size as u64)?;
  // Write file
  let mut mmap = unsafe { MmapMut::map_mut(&file)? };
  // - meta
  write_meta(&mut mmap[0..data_starting_byte], encoded_meta)?;
  mmap.flush_range(0, data_starting_byte)?;
  // - data
  let root = meta.get_root();
  root.write(
    entries_iterator,
    id_rw,
    val_rw,
    &mut mmap[data_starting_byte..file_byte_size],
  )?;
  mmap.flush()?;
  file.sync_all()
}

fn write_meta(mut buff: &mut [u8], encoded_meta: Vec<u8>) -> Result<(), Error> {
  let v_nums = parse_version().unwrap();
  buff.write_all(FILE_TYPE)?;
  buff.write_all(&v_nums)?;
  buff.write_u16::<LittleEndian>(encoded_meta.len() as u16)?;
  assert_eq!(buff.len(), encoded_meta.len());
  buff.copy_from_slice(&encoded_meta[..]);
  Ok(())
}

// Plan a read taking readers!
/*
fn read(input_file: PathBuf) -> Result<Root, Error> {
  // Get the size of the file
  let metadata = fs::metadata(&input_file)?;
  let byte_size = metadata.len();
  // Open the file and read the metadata part
  let file = File::open(&input_file)?;
  let mmap = unsafe { MmapOptions::new().map(&file)? };
  let (_version, data_starting_byte, meta) = read_meta(&mmap)?;
  assert_eq!(byte_size - (data_starting_byte as u64), meta.get_data_byte_size() as u64);
  let root = meta.get_root();
  Ok(root)
}*/

#[cfg(not(target_arch = "wasm32"))]
struct GetProcess<'a> {
  value: String,
  meta: &'a BSTreeMeta,
  mmap: &'a Mmap,
  data_starting_byte: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> Process for GetProcess<'a> {
  type Output = Option<(String, String)>;

  fn exec<I, V, D, IRW, VRW>(
    self,
    _types: IdVal,
    id_rw: IRW,
    val_rw: VRW,
    _dist: D,
  ) -> Result<Self::Output, std::io::Error>
  where
    I: Id,
    V: Val,
    D: Fn(&V, &V) -> V,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let v = self
      .value
      .parse::<V>()
      .map_err(|_e| Error::new(ErrorKind::Other, ""))?; // V::from_str(&self.value).unwrap();
    let root = self.meta.get_root();
    let opt_entry = root.get(v, &self.mmap[self.data_starting_byte..], &id_rw, &val_rw)?;
    Ok(opt_entry.map(|Entry { id, val }| (format!("{:?}", id), format!("{:?}", val))))
  }
}

#[cfg(not(target_arch = "wasm32"))]
struct GetExactProcess<'a> {
  value: String,
  meta: &'a BSTreeMeta,
  mmap: &'a Mmap,
  data_starting_byte: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> Process for GetExactProcess<'a> {
  type Output = Option<(String, String)>;

  fn exec<I, V, D, IRW, VRW>(
    self,
    _types: IdVal,
    id_rw: IRW,
    val_rw: VRW,
    _dist: D,
  ) -> Result<Self::Output, std::io::Error>
  where
    I: Id,
    V: Val,
    D: Fn(&V, &V) -> V,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let v = self
      .value
      .parse::<V>()
      .map_err(|_e| Error::new(ErrorKind::Other, ""))?; // V::from_str(&self.value).unwrap();
    let visitor = VisitorExact::new(v);

    let root = self.meta.get_root();
    let visitor = root.visit(
      visitor,
      &self.mmap[self.data_starting_byte..],
      &id_rw,
      &val_rw,
    )?;
    Ok(
      visitor
        .entry
        .map(|Entry { id, val }| (format!("{:?}", id), format!("{:?}", val))),
    )
  }
}

/*
// Plan a read taking readers!
fn get(value: String, input_file: PathBuf) -> Result<(), Error> {
  let now = Instant::now();
  // Get the size of the file
  let metadata = fs::metadata(&input_file)?;
  let byte_size = metadata.len();
  // Open the file and read the metadata part
  let file = File::open(&input_file)?;
  let mmap = unsafe { MmapOptions::new().map(&file)? };
  let (_version, data_starting_byte, meta) = read_meta(&mmap)?;
  println!("File read in {:?} ms", now.elapsed().as_millis());
  println!("Struct: {:?}", &meta);
  assert_eq!(byte_size - (data_starting_byte as u64), meta.get_data_byte_size() as u64);

  let now = Instant::now();
  let idval = &meta.types;
  let p = GetProcess {
    value,
    meta: &meta,
    mmap: &mmap,
    data_starting_byte,
  };
  if let Some((id, val)) = idval.exec(p)? {
    println!("Value found: id: {}, val: {} in {} ms", id, val, now.elapsed().as_millis());
  } else {
    println!("Not found in {} ms", now.elapsed().as_millis());
  }
  Ok(())
}

// Plan a read taking readers!
fn get_v2(value: String, input_file: PathBuf) -> Result<(), Error> {
  let now = Instant::now();
  // Get the size of the file
  let metadata = fs::metadata(&input_file)?;
  let byte_size = metadata.len();
  // Open the file and read the metadata part
  let file = File::open(&input_file)?;
  let mmap = unsafe { MmapOptions::new().map(&file)? };
  let (_version, data_starting_byte, meta) = read_meta(&mmap)?;
  println!("File read in {:?} ms", now.elapsed().as_millis());
  println!("Struct: {:?}", &meta);
  assert_eq!(byte_size - (data_starting_byte as u64), meta.get_data_byte_size() as u64);

  let now = Instant::now();
  let idval = &meta.types;
  let p = GetExactProcess {
    value,
    meta: &meta,
    mmap: &mmap,
    data_starting_byte,
  };
  if let Some((id, val)) = idval.exec(p)? {
    println!("Value found: id: {}, val: {} in {} ms", id, val, now.elapsed().as_millis());
  } else {
    println!("Not found in {} ms", now.elapsed().as_millis());
  }
  Ok(())
}
*/

/// Returns:
/// * `[u8; 3]`: the version of the code used to build the tree
/// * `usize`: the index of the first data byte
/// * `BSTreeMeta`: the tree structure informations
pub fn read_meta(mut buff: &[u8]) -> Result<([u8; 3], usize, BSTreeMeta), Error> {
  let mut file_type = *FILE_TYPE;
  buff.read_exact(&mut file_type)?;
  assert_eq!((*FILE_TYPE), file_type);
  let mut v_nums: [u8; 3] = Default::default();
  buff.read_exact(&mut v_nums)?;
  // eprintln!("File content: {} v{}.{}.{}", from_utf8(&file_type).unwrap(), v_nums[0], v_nums[1], v_nums[2]);
  let meta_byte_size = buff.read_u16::<LittleEndian>()? as usize;
  let meta: BSTreeMeta = bincode::deserialize_from(&buff[..meta_byte_size])
    .map_err(|_e| Error::new(ErrorKind::Other, String::from("Unable to dezerialize meta")))?;
  Ok((v_nums, file_type.len() + 3 + 2 + meta_byte_size, meta))
}

/*
fn read_id<I, V, IRW, VRW>(&self, val: V, raw_entries: &[u8], id_rw: &IRW, val_rw: &VRW) -> Result<I, Error>
  where I: Id,
        V: Val,
        IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V> {

}
fn read_val<I, V, IRW, VRW>(&self, val: V, raw_entries: &[u8], id_rw: &IRW, val_rw: &VRW) -> Result<V, Error>
  where I: Id,
        V: Val,
        IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V> {

}
fn read_entry<I, V, IRW, VRW>(&self, val: V, raw_entries: &[u8], id_rw: &IRW, val_rw: &VRW) -> Result<Entry<I, V>, Error>
  where I: Id,
  V: Val,
  IRW: ReadWrite<Type=I>,
  VRW: ReadWrite<Type=V> {

}
*/

pub fn parse_version() -> Result<[u8; 3], ParseIntError> {
  //-> [u8; 3] {
  let rv: Result<Vec<u8>, ParseIntError> = VERSION.rsplit('.').map(|i| i.parse::<u8>()).collect();
  let v = rv?;
  assert_eq!(v.len(), 3);
  Ok([v[0], v[1], v[2]])
}

/*
struct BSTreeReader {
  pub fn new() ->
}


// Part that implement Process!!
/// # Args
/// * `data`: slice on the data part of the file
pub fn get<I, V, IRW, VRW, T>(
  meta: BSTreeMeta,
  data: &[u8],
  types: &IdVal,
  id_rw: &IRW,
  val_rw: &VRW,
) -> Result<(), Error> {

}
*/

// impl iterator (that is sorted ;) )

// eq      -> return the Id associated to the given val
// eq_all  -> return It (internally a Range)
// nn      -> Return entry
// knn     -> return It (internally a Range)
// range   -> return It (internally a Range)

// - dicho search

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{rw::U64RW, IdType, ValType};

  #[test]
  fn testok_num_nside() {
    assert_eq!(VERSION, "0.1.1");
    assert_eq!(parse_version().unwrap(), [1_u8, 1_u8, 0_u8]);
  }

  #[test]
  fn testok_build() {
    use std::path::PathBuf;
    let path = PathBuf::from("./test_u64u64_x3.bstree");
    // Write
    {
      let mem_args = MemSizeArgs {
        l1: 32,
        disk: 8192,
        fill_factor: 1.0,
      };
      let n = 3_000_000_u64;
      let mut entries = Vec::with_capacity(n as usize);
      for i in 0..n {
        entries.push(Entry { id: i, val: i });
      }
      let res = build(
        path.clone(),
        &mem_args,
        entries.len(),
        entries.into_iter(),
        &IdVal(IdType::U64, ValType::U64),
        &U64RW,
        &U64RW,
      );
      res.unwrap();
    }
    // Read
    /*
    {
      // let root = read(path.clone());
      // let value = 0_u64;
      // root.get(value, raw_entries: &[u8], &U64RW, &U64RW);
    }
    {
      get(String::from("2999999"), path.clone());
      get(String::from("3000000"), path.clone());
    }
    {
      get_v2(String::from("2999999"), path.clone());
      get_v2(String::from("3000000"), path.clone());
    }
    */
  }
}
