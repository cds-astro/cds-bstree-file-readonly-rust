//! Other arguments needed when building the bs-tree
use std::{
  fs::{self, File},
  io::{BufReader, BufWriter, Error, ErrorKind},
  path::{Path, PathBuf},
};

use itertools::{Itertools, KMerge};
use log::{debug, error};
use structopt::StructOpt;

use crate::rw::ReadWrite;
use crate::{Entry, Id, Val};

#[derive(Debug, StructOpt)]
pub struct MkAlgoArgs {
  #[structopt(short = "k", long, default_value = "50000000")]
  /// Number of rows process at the same time: must be as large as possible but must fit in memory.
  ///
  /// Also equals the number of rows in a temporary file.
  pub chunk_size: usize,
  #[structopt(short = "w", long, default_value = "7")]
  /// 'k' value of the external k-way merge sort, i.e. maximum number of temporary files merge
  /// at the same time.
  pub kway: usize,
  #[structopt(short = "t", long, parse(from_os_str), default_value = ".bstree_tmp")]
  /// Temporary directory containing temporary files
  pub temp: PathBuf,
  #[structopt(parse(from_os_str))]
  /// Output file basename (without the .bstree extension)
  pub output: PathBuf,
}

impl MkAlgoArgs {
  pub fn new<P: AsRef<Path>>(
    chunk_size: Option<usize>,
    kway: Option<usize>,
    temp: Option<P>,
    output: P,
  ) -> Self {
    Self {
      chunk_size: chunk_size.unwrap_or(50_000_000),
      kway: kway.unwrap_or(7),
      temp: temp
        .map(|p| p.as_ref().to_path_buf())
        .unwrap_or(PathBuf::from(".bstree_tmp")),
      output: output.as_ref().to_path_buf(),
    }
  }

  pub fn get_tmp_dir(&self) -> Result<TmpDir, Error> {
    let path = self.temp.clone();
    TmpDir::new(path)
  }

  pub fn get_output(&self) -> PathBuf {
    let mut o = self.output.clone();
    o.set_extension("bstree");
    o
  }
}

const TMP_FILE_PREFIX: &str = ".bstree_chunk";

pub struct TmpDir {
  path: PathBuf,
  level: usize,
  n_files: usize,
}

impl TmpDir {
  pub fn new(root_dir: PathBuf) -> Result<TmpDir, Error> {
    fs::create_dir_all(&root_dir)?;
    Ok(TmpDir {
      path: root_dir,
      level: 0,
      n_files: 0,
    })
  }

  pub fn next_level(&self) -> TmpDir {
    TmpDir {
      path: self.path.clone(),
      level: self.level + 1,
      n_files: 0,
    }
  }

  pub fn n_files(&self) -> usize {
    self.n_files
  }

  // Return the complete path of tmp file of index level `l` and index `i`
  fn get_file_path(&self, index: usize) -> PathBuf {
    let mut file_path = self.path.clone(); // USE JOIN!!
    file_path.push(format!("{}_l{}i{}", TMP_FILE_PREFIX, self.level, index));
    file_path
  }

  // By construction, we can't write a file of lower level when we have already performed at least
  // on reduce.
  pub fn write_tmp_file<I, V, IRW, VRW, T>(
    &mut self,
    id_rw: &IRW,
    val_rw: &VRW,
    entries: T,
  ) -> Result<(), Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
    T: IntoIterator<Item = Entry<I, V>>,
  {
    let mut buff = BufWriter::new(File::create(self.get_file_path(self.n_files))?);
    for entry in entries.into_iter() {
      entry.write(&mut buff, id_rw, val_rw)?;
    }
    self.n_files += 1;
    Ok(())
  }

  // Recursive function working level by level till the remaining number of temporary file is
  // lower or equald to `k`
  pub fn reduce_to_k_files<I, V, IRW, VRW>(
    self,
    id_rw: &IRW,
    val_rw: &VRW,
    k: usize,
  ) -> Result<Self, Error>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    if self.n_files > k {
      let mut next_level_dir = self.next_level();
      // reduce by k-way merge using itertools
      for chunk in &(0..self.n_files).chunks(k) {
        // Merge k tmp files into a new file
        next_level_dir.write_tmp_file(
          id_rw,
          val_rw,
          chunk
            .map(|i| {
              debug!("level: {}; i_chunk: {}", self.level, &i);
              self.to_sorted_entry_iter(id_rw, val_rw, i)
            })
            .kmerge(),
        )?;
      }
      // Merge k files till number of temporary files is larger than k
      next_level_dir.reduce_to_k_files(id_rw, val_rw, k)
    } else {
      Ok(self)
    }
  }

  pub fn to_sorted_iter<'a, I, V, IRW, VRW>(
    &mut self,
    id_rw: &'a IRW,
    val_rw: &'a VRW,
  ) -> KMerge<TmpFileIter<'a, I, V, IRW, VRW>>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    (0..self.n_files)
      .map(|i| self.to_sorted_entry_iter(id_rw, val_rw, i))
      .kmerge()
  }

  fn to_sorted_entry_iter<'a, I, V, IRW, VRW>(
    &self,
    id_rw: &'a IRW,
    val_rw: &'a VRW,
    i: usize,
  ) -> TmpFileIter<'a, I, V, IRW, VRW>
  where
    I: Id,
    V: Val,
    IRW: ReadWrite<Type = I>,
    VRW: ReadWrite<Type = V>,
  {
    let file_path = self.get_file_path(i);
    TmpFile {
      file: file_path,
      id_rw,
      val_rw,
    }
    .into_iter()
  }

  // Remove temporary files (and dir if empty)
  fn clear(&self) -> Result<(), Error> {
    // Remove all temp files
    for entry in fs::read_dir(&self.path)? {
      let file = entry?;
      let file_name = file
        .file_name()
        .into_string()
        .map_err(|_| Error::new(ErrorKind::Other, "Unable to retrieve filename"))?;
      if file_name.starts_with(&format!("{}_l{}", TMP_FILE_PREFIX, self.level)) {
        fs::remove_file(file.path())?;
      }
    }
    // Remove dir if possible, but with no error if it fails (files of a deeper level must be present)
    if let Err(e) = fs::remove_dir(&self.path) {
      error!(
        "Unable to remove directory: {:?}. Directory not empty? Err: {:?}",
        self.path, e
      );
    }
    Ok(())
  }
}

impl Drop for TmpDir {
  fn drop(&mut self) {
    if self.clear().is_err() {
      error!(
        "Unable to clean the temporary dir '{:?}'. Remove files and dir manually!",
        &self.path
      );
    }
  }
}

struct TmpFile<'a, I, V, IRW, VRW>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
{
  file: PathBuf,
  id_rw: &'a IRW,
  val_rw: &'a VRW,
}

impl<'a, I, V, IRW, VRW> IntoIterator for TmpFile<'a, I, V, IRW, VRW>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
{
  type Item = Entry<I, V>;
  type IntoIter = TmpFileIter<'a, I, V, IRW, VRW>;

  fn into_iter(self) -> Self::IntoIter {
    let f =
      File::open(&self.file).unwrap_or_else(|_| panic!("Unable to open file: {:?}", &self.file));
    let metadata = f
      .metadata()
      .unwrap_or_else(|_| panic!("Unable to read file metadata: {:?}", &self.file));
    let file_size = metadata.len() as usize;
    let n_entries = file_size / (self.id_rw.n_bytes() + self.val_rw.n_bytes());
    TmpFileIter {
      reader: BufReader::new(f),
      id_rw: self.id_rw,
      val_rw: self.val_rw,
      n_entries,
      n_read: 0,
    }
  }
}

pub struct TmpFileIter<'a, I, V, IRW, VRW>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
{
  reader: BufReader<File>,
  id_rw: &'a IRW,
  val_rw: &'a VRW,
  n_entries: usize,
  n_read: usize,
}

impl<'a, I, V, IRW, VRW> Iterator for TmpFileIter<'a, I, V, IRW, VRW>
where
  I: Id,
  V: Val,
  IRW: ReadWrite<Type = I>,
  VRW: ReadWrite<Type = V>,
{
  type Item = Entry<I, V>;

  fn size_hint(&self) -> (usize, Option<usize>) {
    let n_remaining = self.n_entries - self.n_read;
    (n_remaining, Some(n_remaining))
  }

  fn next(&mut self) -> Option<Self::Item> {
    if self.n_read < self.n_entries {
      self.n_read += 1;
      let entry = Entry::read(&mut self.reader, self.id_rw, self.val_rw)
        .unwrap_or_else(|e| panic!("Error reading entry: {:?}", &e));
      Some(entry)
    } else {
      None
    }
  }
}
