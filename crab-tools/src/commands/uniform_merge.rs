use std::{fs::File, net::Ipv6Addr, path::PathBuf};

use anyhow::*;
use clap::Args;
use db_model::prefix_tree::PrefixTree;
use ipnet::Ipv6Net;
use itertools::Itertools;
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Args, Clone)]
pub struct Params {
    in_file: PathBuf,
    out_file: PathBuf,
}

pub fn handle(params: Params) -> Result<()> {
    let out_file = File::create_new(&params.out_file)?;
    let in_file = File::open(&params.in_file)?;

    info!("Reading input...");
    let mut reader = csv::Reader::from_reader(in_file);
    let input: Vec<SubnetRow> = reader.deserialize().map(|it| it.unwrap()).collect_vec();

    info!("Processing {} subnets...", input.len());
    let result = run(input)?;

    info!("Writing {} nodes...", result.len());
    write(out_file, result)
}

#[derive(Deserialize, Debug)]
pub struct SubnetRow {
    pub subnet: Ipv6Net,
    pub received_count: u64,
    pub last_hop_routers: String,
}

impl SubnetRow {
    fn super_row(self, consume: SubnetRow) -> SubnetRow {
        if consume.last_hop_routers != self.last_hop_routers {
            panic!("LHRs should be checked before consuming");
        }
        Self {
            subnet: self.subnet.supernet().expect("to have supernet"),
            received_count: self.received_count + consume.received_count,
            last_hop_routers: self.last_hop_routers,
        }
    }
}

#[derive(Serialize)]
pub struct OutputNode {
    pub net: Ipv6Net,
    pub net_len: u8,
    pub is_leaf: bool,

    pub last_hop_routers: Option<String>,
    pub lhr_set_hash: Option<String>,
}

fn write(out_file: File, result: Vec<OutputNode>) -> Result<()> {
    let mut writer = csv::Writer::from_writer(out_file);

    for item in result {
        writer.serialize(item)?;
    }

    Ok(())
}

#[derive(Debug)]
enum Node {
    SameLeaf {
        row: SubnetRow,
    },
    Distinct {
        net: Ipv6Net,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    fn net(&self) -> &Ipv6Net {
        match self {
            Node::SameLeaf { row } => &row.subnet,
            Node::Distinct {
                net,
                left: _,
                right: _,
            } => net,
        }
    }

    fn last_hop_routers(&self) -> Option<&String> {
        match self {
            Node::SameLeaf { row } => Some(&row.last_hop_routers),
            _ => None,
        }
    }

    fn row(self) -> Option<SubnetRow> {
        match self {
            Node::SameLeaf { row } => Some(row),
            _ => None,
        }
    }

    fn merge_with(self, right: Self) -> Self {
        if self.last_hop_routers() == right.last_hop_routers() {
            if let Node::SameLeaf { row } = self {
                return Node::SameLeaf {
                    row: row.super_row(right.row().expect("same LHRs = leaf")),
                };
            }
        }

        Node::Distinct {
            net: self
                .net()
                .supernet()
                .expect("to have supernet when merging"),
            left: self.into(),
            right: right.into(),
        }
    }
}

fn run(input: Vec<SubnetRow>) -> Result<Vec<OutputNode>> {
    let orig_prefix_size = input.first().expect("input not empty").subnet.prefix_len();
    for check in input.iter() {
        if check.subnet.prefix_len() != orig_prefix_size {
            bail!(
                "item {:?} did not match prefix length of first record {}",
                check,
                orig_prefix_size
            );
        }
    }

    info!("Merging...");
    let input = input.into_iter().map_into().collect_vec();
    let root = merge_to_root(input)?;

    info!("Collecting from root {}...", root.net());
    let mut nodes: Vec<OutputNode> = vec![];
    collect_recursive_into(&mut nodes, root);

    Ok(nodes)
}

fn merge_to_root(input: Vec<Node>) -> Result<Node> {
    let mut current_level_nodes = input;
    let mut next_level_nodes: Vec<Node> = vec![];

    loop {
        current_level_nodes.sort_by_key(|it| *it.net());

        {
            let mut current_iter = current_level_nodes.drain(..);

            while let Some(left_node) = current_iter.next() {
                let right_node = match current_iter.next() {
                    Some(it) => it,
                    None => {
                        if next_level_nodes.is_empty() {
                            return Ok(left_node);
                        } else {
                            bail!("Expected even number of nodes, found {:?}", left_node);
                        }
                    }
                };
                if left_node.net().supernet() != right_node.net().supernet() {
                    bail!(
                        "Adjacent nodes should have same supernet; {:?} vs {:?}",
                        left_node,
                        right_node
                    );
                }
                next_level_nodes.push(left_node.merge_with(right_node));
            }
        }

        std::mem::swap(&mut current_level_nodes, &mut next_level_nodes);
    }
}

impl From<SubnetRow> for Node {
    fn from(row: SubnetRow) -> Self {
        Self::SameLeaf { row }
    }
}

fn collect_recursive_into(result: &mut Vec<OutputNode>, node: Node) {
    match node {
        Node::SameLeaf { row } => result.push(row.into()),
        Node::Distinct { net, left, right } => {
            result.push(OutputNode {
                net,
                net_len: net.prefix_len(),
                is_leaf: false,
                last_hop_routers: None,
                lhr_set_hash: None,
            });
            collect_recursive_into(result, *left);
            collect_recursive_into(result, *right);
        }
    }
}

impl From<SubnetRow> for OutputNode {
    fn from(row: SubnetRow) -> Self {
        let lhrs = row
            .last_hop_routers
            .split(";")
            .map(|it| it.parse::<Ipv6Addr>().expect("input LHR to parse"));
        let hash = PrefixTree::hash_lhrs(lhrs);
        Self {
            net: row.subnet,
            net_len: row.subnet.prefix_len(),
            is_leaf: true,
            last_hop_routers: Some(row.last_hop_routers),
            lhr_set_hash: Some(hash.to_string()),
        }
    }
}
