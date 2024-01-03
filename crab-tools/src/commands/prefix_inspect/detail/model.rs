use itertools::Itertools;
use std::io::BufWriter;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct PrintedPrefix {
    left_start_index: usize,
    right_start_index: usize,
    pub lines: Vec<String>,
}

impl PrintedPrefix {
    fn new(sections: Vec<String>) -> Self {
        let header_len = sections[0].lines().count();
        let left_len = sections[1].lines().count();
        let lines = sections
            .into_iter()
            .flat_map(|it| it.lines().map(|it| it.to_owned()).collect_vec())
            .collect_vec();
        Self {
            lines,
            left_start_index: header_len,
            right_start_index: header_len + left_len,
        }
    }

    pub fn find_subnet_from_line_index(&self, index: usize) -> Option<u8> {
        if index < self.left_start_index {
            None
        } else if index < self.right_start_index {
            Some(0)
        } else {
            Some(1)
        }
    }
}

pub(super) struct PrintedPrefixBuilder {
    pub buf: BufWriter<Vec<u8>>,
    pub sections: Vec<String>,
}

impl From<PrintedPrefixBuilder> for PrintedPrefix {
    fn from(value: PrintedPrefixBuilder) -> Self {
        Self::new(value.sections)
    }
}

impl Default for PrintedPrefixBuilder {
    fn default() -> Self {
        Self {
            buf: BufWriter::new(vec![]),
            sections: vec![],
        }
    }
}

impl PrintedPrefixBuilder {
    pub(super) fn flush_section(mut self) -> StdResult<Self, Error> {
        let utf8 = self
            .buf
            .into_inner()
            .expect("writing to Vec<u8> to always succeed");
        let section = String::from_utf8(utf8).map_err(pfxerr!(Format))?;
        self.sections.push(section);
        self.buf = BufWriter::new(vec![]);
        Ok(self)
    }
}

// Separate error struct needed to implement Clone (and this is also the reason for the weird desc thing)
#[derive(Debug, Error, Clone)]
pub enum Error {
    #[error("Connecting to DB: {desc}")]
    DbConnect { desc: String },
    #[error("Formatting output: {desc}")]
    Format { desc: String },
    #[error("Loading tree: {desc}")]
    LoadTree { desc: String },
    #[error("Loading measurements: {desc}")]
    LoadMeasurements { desc: String },
    #[error("Splitting into subnets: {desc}")]
    SubnetSplit { desc: String },
}

pub(super) type StdResult<T, E> = std::result::Result<T, E>;
pub type Result = StdResult<PrintedPrefix, Error>;
