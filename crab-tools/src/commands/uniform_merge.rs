use std::{collections::HashSet, fs::File, net::Ipv6Addr, path::PathBuf};

use anyhow::*;
use clap::Args;
use db_model::prefix_tree::PrefixTree;
use ipnet::Ipv6Net;
use itertools::Itertools;
use log::{debug, info};
use petgraph::{dot::{Config, Dot}, graphmap::DiGraphMap};
use serde::{Deserialize, Serialize};

#[derive(Args, Clone)]
pub struct Params {
    in_file: PathBuf,
    out_file: PathBuf,

    #[arg(long, num_args(0..))]
    ignore_lhr: Vec<Ipv6Addr>,
}

pub fn handle(params: Params) -> Result<()> {
    let out_file = File::create_new(&params.out_file)?;
    let in_file = File::open(&params.in_file)?;

    info!("Reading input...");
    let mut reader = csv::Reader::from_reader(in_file);
    let input: Vec<SubnetRow> = reader.deserialize().map(|it| it.unwrap()).collect_vec();

    info!("Processing {} subnets...", input.len());
    let result = run(params, input)?;

    info!("Writing {} nodes...", result.len());
    write(out_file, result)
}

#[derive(Deserialize, Debug)]
pub struct SubnetRow {
    pub subnet: Ipv6Net,
    pub received_count: u64,
    pub last_hop_routers: String,
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
    SameLeaf(Leaf),
    Distinct {
        net: Ipv6Net,
        left: Box<Node>,
        right: Box<Node>,
    },
}

#[derive(Debug)]
struct Leaf {
    pub net: Ipv6Net,
    pub received_count: u64,
    pub last_hop_routers: HashSet<Ipv6Addr>,
}

impl Leaf {
    fn merge_with(self, consume: &Self) -> Self {
        if consume.last_hop_routers != self.last_hop_routers {
            panic!("LHRs should be checked before consuming. oida");
        }
        Self {
            net: self.net.supernet().expect("to have supernet"),
            received_count: self.received_count + consume.received_count,
            last_hop_routers: self.last_hop_routers,
        }
    }
}

impl Node {
    fn leaf(&self) -> Option<&Leaf> {
        match self {
            Node::SameLeaf(leaf) => Some(leaf),
            _ => None,
        }
    }

    fn net(&self) -> &Ipv6Net {
        match self {
            Node::SameLeaf(leaf) => &leaf.net,
            Node::Distinct {
                net,
                left: _,
                right: _,
            } => net,
        }
    }

    fn last_hop_routers(&self) -> Option<&HashSet<Ipv6Addr>> {
        self.leaf().map(|it| &it.last_hop_routers)
    }

    fn merge_with(self, right: Self) -> Self {
        debug!("Merging {:?} with {:?}", self.net(), right.net());
        if self.last_hop_routers() == right.last_hop_routers() {
            if let Node::SameLeaf(leaf) = self {
                debug!("It was a match!");
                return Node::SameLeaf(leaf.merge_with(right.leaf().expect("same LHRs = leaf")));
            }
        }
        debug!("ghosted");

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

fn run(params: Params, input: Vec<SubnetRow>) -> Result<Vec<OutputNode>> {
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
    let input = input
        .into_iter()
        .map(|row| to_node(&params.ignore_lhr, row))
        .collect_vec();
    let root = merge_to_root(input)?;

    info!("Collecting from root {}...", root.net());
    let mut nodes: Vec<OutputNode> = vec![];
    collect_recursive_into(&mut nodes, root);

    const NIL: &str = "";
    let mut graph: DiGraphMap<Ipv6Net, &str> = DiGraphMap::new();
    for node in nodes.iter() {
        graph.add_node(node.net);
    }
    for node in nodes.iter() {
        let supernet = node.net.supernet().expect("a supernet please");
        if graph.contains_node(supernet) {
            graph.add_edge(node.net, supernet, NIL);
        }
    }
    eprintln!("{}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));

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
                let merged = left_node.merge_with(right_node);
                debug!("Adding node: {:?}", merged);
                next_level_nodes.push(merged);
            }
        }

        std::mem::swap(&mut current_level_nodes, &mut next_level_nodes);
        info!("Processing next level...");
    }
}

fn to_node(ignore_lhrs: &[Ipv6Addr], row: SubnetRow) -> Node {
    let last_hop_routers = row
        .last_hop_routers
        .split(";")
        .map(|it| it.parse::<Ipv6Addr>().expect("input LHR to parse"))
        .filter(|lhr| !ignore_lhrs.contains(lhr))
        .collect();
    Node::SameLeaf(Leaf {
        net: row.subnet,
        received_count: row.received_count,
        last_hop_routers,
    })
}

fn collect_recursive_into(result: &mut Vec<OutputNode>, node: Node) {
    match node {
        Node::SameLeaf(leaf) => result.push(leaf.into()),
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

impl From<Leaf> for OutputNode {
    fn from(leaf: Leaf) -> Self {
        let hash = PrefixTree::hash_lhrs(leaf.last_hop_routers.iter());
        Self {
            net: leaf.net,
            net_len: leaf.net.prefix_len(),
            is_leaf: true,
            last_hop_routers: Some(leaf.last_hop_routers.into_iter().sorted().join(";")),
            lhr_set_hash: Some(hash.to_string()),
        }
    }
}
