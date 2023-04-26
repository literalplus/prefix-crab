use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};

/// Abstracts target address provision for [Caller]. This has the effect that consumers can
/// provide target addresses "on their own terms" and are not bound to provide them in a specific
/// format or time (e.g. all at once, in a vector, or in a file).
///
/// Please note that no more than one [TargetCollector] can be in use at the same time, because
/// there is no guarantee that different backends (e.g. a file) are used for each instance.
pub struct TargetCollector {
    pub path: PathBuf,
    writer: Option<BufWriter<File>>,
    count: u32,
}

/// NOT thread-safe. thansk
impl TargetCollector {
    pub fn new_default() -> Result<Self> {
        let path = PathBuf::from("out/zmap-addr-list.txt");
        TargetCollector::new(path)
    }

    pub fn new(path: PathBuf) -> Result<Self> {
        let writer = TargetCollector::create_or_truncate_file(&path)?;
        Ok(TargetCollector { path, writer: Some(writer), count: 0u32 })
    }

    fn create_or_truncate_file(path: &Path) -> Result<BufWriter<File>> {
        let parent_dir = path.parent()
            .with_context(|| format!("targets file has no parent {:?}", path))?;
        fs::create_dir_all(parent_dir)
            .with_context(|| format!("while creating targets parent directory ({:?})", path))?;
        let file = File::create(&path) // <- truncates the file
            .with_context(|| format!("while creating targets file {:?}", path))?;
        Ok(BufWriter::new(file))
    }

    /// Utility function that pre-fills a [TargetCollector] with a vector. This is most useful
    /// when the set of addresses is already known and the flexibility provided by
    /// [TargetCollector] is not needed.
    pub fn from_vec(addrs: Vec<String>) -> Result<Self> {
        let mut collector = Self::new_default()?;
        collector.push_vec(addrs)?;
        Ok(collector)
    }

    /// Pushes a single address to this collector.
    ///
    /// Note that, if a push() fails, the collector enters an undefined state and it should
    /// no longer be used. Further note that there is no guarantee that writes are immediately
    /// reflected in the target file, i.e. buffered I/O may be used.
    pub fn push(&mut self, addr_str: String) -> Result<()> {
        write!(self.borrow_writer(), "{}\n", addr_str)
            .with_context(|| format!("while writing a target to {:?}", self.path))?;
        self.count += 1;
        Ok(())
    }

    fn borrow_writer(&mut self) -> &mut BufWriter<File> {
        return self.writer.as_mut()
            .expect("writer to be Some")
    }

    /// Pushes a vector of addresses to this collector.
    ///
    /// See [TargetCollector::push] for usage notes!
    pub fn push_vec(&mut self, addrs: Vec<String>) -> Result<()> {
        for addr in addrs {
            self.push(addr)?;
        }
        Ok(())
    }

    /// Flushes the writer backing this collector, dumping any unfinished output.
    ///
    /// This must be called before attempting to use the resulting target file, otherwise
    /// the state of the file is undefined.
    pub fn flush(&mut self) -> Result<()> {
        self.borrow_writer().flush()
            .with_context(|| format!("while flushing targets to {:?}", self.path))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::distributions::{Alphanumeric, DistString};
    use tempfile::TempDir;
    use super::*;

    fn new_with_tempfile() -> (TargetCollector, TempDir) {
        let tempdir = tempfile::tempdir()
            .expect("tempdir to be created");
        // Tests run in parallel and it seems to happen that the tempdir somehow gets shared
        // between threads... unsure why but using a random file name fixes it.
        let random_chars = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
        let file_name = format!("{}.txt", random_chars);
        let targets_path = tempdir.path().with_file_name(file_name);
        let collector = TargetCollector::new(targets_path)
            .expect("collector creation to succeed");
        return (collector, tempdir);
    }

    #[test]
    fn check_write_one() -> Result<()> {
        // given
        let (mut collector, tempdir) = new_with_tempfile();
        let addr = "hi! it's me i'm an address";

        // when
        collector.push(addr.to_string())?;

        // then
        collector.flush()?;
        let actual = std::io::read_to_string(File::open(collector.path)?)?;
        assert_eq!(actual, format!("{}\n", addr));
        drop(tempdir);
        Ok(())
    }

    #[test]
    fn check_write_multiple() -> Result<()> {
        // given
        let (mut collector, tempdir) = new_with_tempfile();
        let addr = "hi! it's me i'm an address";
        let another_addr = "at tea time";

        // when
        collector.push_vec(vec![addr.to_string(), another_addr.to_string()])?;

        // then
        collector.flush()?;
        let actual = std::io::read_to_string(File::open(collector.path)?)?;
        assert_eq!(actual, format!("{}\n{}\n", addr, another_addr));
        drop(tempdir);
        Ok(())
    }


    #[test]
    fn check_reuse() -> Result<()> {
        // given
        let (mut collector, tempdir) = new_with_tempfile();
        let addr = "hi! it's me i'm an address";
        let another_addr = "at tea time";
        collector.push_vec(vec![addr.to_string(), another_addr.to_string()])?;
        collector.flush()?;
        let new_addr = "everybody agrees"; // shorter to test truncate()

        // when
        collector.truncate_reset()?;
        collector.push(new_addr.to_string())?;

        // then
        collector.flush()?;
        let actual = std::io::read_to_string(File::open(collector.path)?)?;
        assert_eq!(actual, format!("{}\n", new_addr));
        drop(tempdir);
        Ok(())
    }
}
