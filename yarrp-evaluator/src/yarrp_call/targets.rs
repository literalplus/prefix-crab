use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::net::Ipv6Addr;
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
        let path = PathBuf::from("out/yarrp-addr-list.txt");
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
        let file = File::create(path) // <- truncates the file
            .with_context(|| format!("while creating targets file {:?}", path))?;
        Ok(BufWriter::new(file))
    }

    /// Pushes a single address to this collector.
    ///
    /// Note that, if a push() fails, the collector enters an undefined state and it should
    /// no longer be used. Further note that there is no guarantee that writes are immediately
    /// reflected in the target file, i.e. buffered I/O may be used.
    pub fn push(&mut self, addr: &Ipv6Addr) -> Result<()> {
        writeln!(self.borrow_writer(), "{}", addr)
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
    pub fn push_slice(&mut self, addrs: &[Ipv6Addr]) -> Result<()> {
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

    pub fn is_empty(&self) -> bool {
        self.count == 0
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
        (collector, tempdir)
    }

    fn new_with_same_file(existing: &TargetCollector) -> TargetCollector {
        TargetCollector::new(existing.path.clone())
            .expect("collector creation to succeed")
    }

    #[test]
    fn check_write_one() -> Result<()> {
        // given
        let (mut collector, tempdir) = new_with_tempfile();
        let addr = "2001:db8:7777::13:7777".parse::<Ipv6Addr>()?;

        // when
        collector.push(&addr)?;

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
        let addr = "2001:db8:aaaa::1".parse::<Ipv6Addr>()?;
        let another_addr = "2001:db8:bbbb::0".parse::<Ipv6Addr>()?;

        // when
        collector.push_slice(&[addr, another_addr])?;

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
        let (mut first_collector, tempdir) = new_with_tempfile();
        let addr = "2001:db8:abaf::1".parse::<Ipv6Addr>()?;
        let another_addr = "2001:db8:caa7::0".parse::<Ipv6Addr>()?;
        first_collector.push_slice(&[addr, another_addr])?;
        first_collector.flush()?;
        let new_addr = "2001:db8:cafe::beef".parse::<Ipv6Addr>()?;

        // when
        let mut new_collector = new_with_same_file(&first_collector);
        new_collector.push(&new_addr)?;

        // then
        new_collector.flush()?;
        let actual = std::io::read_to_string(File::open(first_collector.path)?)?;
        assert_eq!(actual, format!("{}\n", new_addr));
        drop(tempdir);
        Ok(())
    }
}
