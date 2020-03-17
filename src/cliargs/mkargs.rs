//! Other arguments needed when building the bs-tree
use structopt::StructOpt;
use itertools::{Itertools, KMerge};
use std::path::PathBuf;
use std::str::FromStr;
use std::io::{ErrorKind, Error, BufReader, BufWriter};
use std::fs::{self, File};

use crate::{FromU64, Entry};
use crate::rw::ReadWrite;

#[derive(Debug, StructOpt)]
pub struct MkAlgoArgs {
  # [structopt(short = "k", long, default_value = "50000000")]
  /// Number of rows process at the same time: must be as large as possible but must fit in memory.
  ///
  /// Also equals the number of rows in a temporary file.
  pub chunk_size: usize,
  #[structopt(short = "w", long, default_value = "7")]
  /// 'k' value of the external k-way merge sort, i.e. maximum number of temporary files merge
  /// at the same time.
  pub kway: usize,
  # [structopt(short = "t", long, parse(from_os_str), default_value = ".bstree_tmp")]
  /// Temporary directory containing temporary files
  pub temp: PathBuf,
  # [structopt(parse(from_os_str))]
  /// Output file basename (without the .bstree.bin extension)
  output: PathBuf
}

impl MkAlgoArgs {
  
  pub fn get_tmp_dir(&self) -> TmpDir {
    let mut path = self.temp.clone();
    TmpDir::new(path)
  }

  pub fn get_output(&self) -> PathBuf {
    let mut o = self.output.clone();
    o.set_extension("bstree.bin");
    o
  }

}

const TMP_FILE_PREFIX: &'static str = ".bstree_chunk"; 

pub struct TmpDir {
  path: PathBuf,
  level: usize,
  n_files: usize,
}

impl TmpDir {
  
  pub fn new(root_dir: PathBuf) -> TmpDir {
    fs::create_dir_all(&root_dir);
    TmpDir {
      path: root_dir,
      level: 0,
      n_files: 0,
    }
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
    let mut file_path = self.path.clone();
    file_path.push(format!("{}_l{}i{}", TMP_FILE_PREFIX, self.level, index));
    file_path
  } 
  
  // By construction, we can't write a file of lower level when we have already performed at least
  // on reduce.
  pub fn write_tmp_file<I, V, IRW, VRW, T>(&mut self, id_rw: &IRW, val_rw: &VRW, entries: T) -> Result<(), Error>
    where I: FromStr + FromU64,
          V: FromStr + Ord,
          IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V>,
          T: IntoIterator<Item=Entry<I, V>> {
    let mut buff = BufWriter::new(File::create(self.get_file_path(self.n_files))?);
    for entry in entries.into_iter() {
      entry.write(&mut buff, id_rw, val_rw)?;
    }
    self.n_files += 1;
    Ok(())
  }
  
  // Recursive function working level by level till the remaining number of temporary file is
  // lower or equald to `k`
  pub fn reduce_to_k_files<I, V, IRW, VRW>(self, id_rw: &IRW, val_rw: &VRW, k: usize) -> Result<Self, Error>
    where I: FromStr + FromU64,
          V: FromStr + Ord,
          IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    if self.n_files > k {
      let mut next_level_dir = self.next_level();
      // reduce by k-way merge using itertools
      for chunk in &(0..self.n_files).into_iter().chunks(k) {
        // Merge k tmp files into a new file
        next_level_dir.write_tmp_file(id_rw, val_rw, chunk.map(|i| self.to_sorted_entry_iter(id_rw, val_rw, i)).kmerge());
      }
      // Merge k files till number of temporary files is larger than k
      next_level_dir.reduce_to_k_files(id_rw, val_rw, k)
    } else {
      Ok(self)
    }
  }

  pub fn to_sorted_iter<'a, I, V, IRW, VRW>(&mut self, id_rw: &'a IRW, val_rw: &'a VRW) -> KMerge<TmpFileIter<'a, I, V, IRW, VRW>>
    where I: FromStr + FromU64,
          V: FromStr + Ord,
          IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    (0..self.n_files).into_iter().map(|i| self.to_sorted_entry_iter(id_rw, val_rw, i)).kmerge()
  }
  
  fn to_sorted_entry_iter<'a, I, V, IRW, VRW>(&self, id_rw: &'a IRW, val_rw: &'a VRW, i: usize) -> TmpFileIter<'a, I, V, IRW, VRW>
    where I: FromStr + FromU64,
          V: FromStr + Ord,
          IRW: ReadWrite<Type=I>,
          VRW: ReadWrite<Type=V> {
    let file_path = self.get_file_path(i);
    TmpFile {
      file: file_path,
      id_rw,
      val_rw,
    }.into_iter()
  }

  // Remove temporary files (and dir if empty)
  fn clear(&self) -> Result<(), Error> {
    // Remove all temp files
    for entry in fs::read_dir(&self.path)? {
      let file = entry?;
      let file_name = file.file_name().into_string().map_err(|_| Error::new(ErrorKind::Other, "Unable to retrieve filename"))?;
      if file_name.starts_with(&format!("{}_l{}", TMP_FILE_PREFIX, self.level)) {
        fs::remove_file(file.path())?;  
      }
    }
    // Remove dir if possible, but with no error if it fails (files of a deeper level must be present)
    Ok(())
  }
}

impl Drop for TmpDir {
  fn drop(&mut self) {
    if self.clear().is_err() {
      eprintln!("Unable to clean the temporary dir '{:?}'. Remove files and dir manually!", &self.path);
    }
  }
}



struct TmpFile<'a, I, V, IRW, VRW> 
  where I: FromStr + FromU64,
        V: FromStr + Ord,
        IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V>  {
  file: PathBuf,
  id_rw: &'a IRW,
  val_rw: &'a VRW,
}

impl <'a, I, V, IRW, VRW> IntoIterator for TmpFile<'a, I, V, IRW, VRW>
  where I: FromStr + FromU64,
        V: FromStr + Ord,
        IRW: ReadWrite<Type=I>,
        VRW: ReadWrite<Type=V> {
  type Item = Entry<I, V>;
  type IntoIter = TmpFileIter<'a, I, V, IRW, VRW>;

  fn into_iter(self) -> Self::IntoIter {
    let f = File::open(&self.file).expect(&format!("Unable to open file: {:?}", &self.file));
    let metadata = f.metadata().expect(&format!("Unable to read file metadata: {:?}", &self.file));
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
  where I: FromStr + FromU64,
        V: FromStr + Ord, 
      IRW: ReadWrite<Type=I>,
      VRW: ReadWrite<Type=V> {
  reader: BufReader<File>,
  id_rw: &'a IRW,
  val_rw: &'a VRW,
  n_entries: usize,
  n_read: usize,
}

impl <'a, I, V, IRW, VRW> Iterator for TmpFileIter<'a, I, V, IRW, VRW>
  where I: FromStr + FromU64,
        V: FromStr + Ord,
      IRW: ReadWrite<Type=I>,
      VRW: ReadWrite<Type=V>  {
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

/*

pub fn get_toml_path(file_dir: &PathBuf) -> Box<Path> {
  let mut output_file_path = (*file_dir).clone();
  let mut file_name = output_file_path.file_name().unwrap().to_string_lossy().to_string();
  file_name.push_str(".bstree.toml");
  output_file_path.push(file_name);
  output_file_path.into_boxed_path()
}

pub fn get_file_path(file_dir: &PathBuf) -> Box<Path> {
  let mut output_file_path =(*file_dir).clone();
  let mut file_name = output_file_path.file_name().unwrap().to_string_lossy().to_string();
  file_name.push_str(".bstree.bin");
  output_file_path.push(file_name);
  output_file_path.into_boxed_path()
}

*/


