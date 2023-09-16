use clap::Args;
#[derive(Args, Debug)]
#[group(id = "treeupdate")]
pub struct Params {
    /// Whether to insert freshly seeded prefixes into the tree for analysis.
    /// Note that the default value of 'false' means that NO internet-wide scan
    /// is performed, and only prefixes that are manually added to the tree are
    /// processed.
    #[arg(long, env = "PUSH_FRESH_PREFIXES_TO_TREE", default_value = "false")]
    push_fresh_prefixes_to_tree: bool,
}
