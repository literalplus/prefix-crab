use itertools::Itertools;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    net::Ipv6Addr,
    path::PathBuf,
};
use thiserror::Error;

use clap::Args;
use ipnet::{AddrParseError, Ipv6Net};

#[derive(Args, Clone)]
#[group(id = "blocklist")]
pub struct Params {
    /// Flatfile to read blocklist of not-allowed-to-scan prefixes from.
    /// One IPv6 CIDR prefix per line
    /// # at start of line to ocmment out the whole line
    /// No headers or similar
    #[arg(
        long,
        default_value = "/etc/scanning/blocklist",
        env = "BLOCKLIST_FILE"
    )]
    pub blocklist_file: PathBuf,

    /// Whether to treat absence of the blocklist file as a fatal error (the default).
    /// Otherwise, an empty blocklist is used.
    #[arg(long, default_value = "true", env = "FAIL_ON_MISSING_BLOCKLIST")]
    pub fail_on_missing_blocklist: bool,
}

#[derive(Debug)]
pub struct PrefixBlocklist {
    entries: Vec<Ipv6Net>,
}

impl PrefixBlocklist {
    pub fn new(entries: Vec<Ipv6Net>) -> Self {
        return Self { entries };
    }

    pub fn is_blocked(&self, query: &Ipv6Addr) -> bool {
        for entry in self.entries.iter() {
            if entry.contains(query) {
                return true;
            }
        }
        false
    }

    pub fn is_whole_net_blocked(&self, query: &Ipv6Net) -> bool {
        for entry in self.entries.iter() {
            if entry.contains(query) {
                return true;
            }
        }
        false
    }

    pub fn is_any_subnet_blocked(&self, query: &Ipv6Net) -> bool {
        for entry in self.entries.iter() {
            if query.contains(entry) {
                return true;
            }
        }
        false
    }
}

#[derive(Error, Debug)]
pub enum BlocklistReadError {
    #[error("blocklist file does not exist: `{0}`")]
    NoSuchFile(PathBuf),

    #[error("failed to open blocklist file `{path}`")]
    FailedOpen {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read a line from blocklist file")]
    FailedRead { source: std::io::Error },

    #[error("invalid IPv6 prefix encountered: `{line}`")]
    PrefixSyntax {
        line: String,
        source: AddrParseError,
    },
}

pub type BlocklistReadResult = Result<PrefixBlocklist, BlocklistReadError>;

pub fn read(params: Params) -> BlocklistReadResult {
    use BlocklistReadError as E;

    match read_from(params.blocklist_file) {
        e @ Err(E::NoSuchFile(_)) => {
            if params.fail_on_missing_blocklist {
                e
            } else {
                Ok(PrefixBlocklist::new(vec![]))
            }
        }
        any => any,
    }
}

fn read_from(path: PathBuf) -> BlocklistReadResult {
    use BlocklistReadError as E;

    if !path.is_file() {
        return Err(BlocklistReadError::NoSuchFile(path));
    }

    let mut entries = vec![];
    let file = File::open(path.clone()).map_err(|source| E::FailedOpen { path, source })?;
    let lines = BufReader::new(file)
        .lines()
        .filter_ok(|line| !line.starts_with("#") && !line.is_empty());

    for line_res in lines {
        match line_res {
            Err(source) => return Err(E::FailedRead { source }),
            Ok(line) => match line.parse() {
                Ok(prefix) => entries.push(prefix),
                Err(source) => return Err(E::PrefixSyntax { line, source }),
            },
        }
    }

    Ok(PrefixBlocklist::new(entries))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use assertor::assert_that;
    use assertor::BooleanAssertion;
    use assertor::ResultAssertion;

    use crate::test_utils::addr;
    use crate::test_utils::net;

    use super::read_from;

    #[test]
    fn example_blocklist_loads() {
        // given
        let path = PathBuf::from("blocklist-example.txt");
        // when
        let res = read_from(path);
        // then
        assert_that!(res).is_ok();
        let res = res.unwrap();

        assert_that!(res.is_blocked(&addr("2001:0000::"))).is_true();
        assert_that!(res.is_blocked(&addr("2001:1000::"))).is_false();

        assert_that!(res.is_whole_net_blocked(&net("2001:0000::/64"))).is_true();
        assert_that!(res.is_whole_net_blocked(&net("2001:0000::/32"))).is_true();
        assert_that!(res.is_whole_net_blocked(&net("2001:0000::/31"))).is_false();

        assert_that!(res.is_any_subnet_blocked(&net("2001:0000::/31"))).is_true();
        assert_that!(res.is_any_subnet_blocked(&net("2001:0000::/32"))).is_true();
        assert_that!(res.is_any_subnet_blocked(&net("2001:0000::/33"))).is_false();
    }
}
