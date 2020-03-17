//! Size of the different memory caches used to build and query the tree.
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct MemSizeArgs {
  # [structopt(long, default_value = "32")] // 32 kB
  /// Size of the L1 cache memory, in kilobytes (kB). It correspond to the page size of a DBMS.
  pub l1: usize,
  # [structopt(long, default_value = "8192")] // 8192 kB = 8 MB
  /// Size of the HDD cache size, in kilobytes (kB)
  pub disk: usize,
  # [structopt(short = "r", long, default_value = "1.0")] // 80%
  /// Fill factor: to prevent occupying the full l1 cache memory
  pub fill_factor: f32,
}

impl MemSizeArgs {
  
  /// Returns the size of the l1 cache, in bytes.
  pub fn l1_byte_size(&self) -> usize {
    ((self.l1 * 1024) as f32 * self.fill_factor) as usize
  }
  
  /// Returns the size of the disk cache, in bytes.
  pub fn disk_byte_size(&self) -> usize {
    self.disk * 1024
  }
}
