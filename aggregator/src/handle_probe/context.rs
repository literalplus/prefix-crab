use anyhow::*;
use diesel::dsl::not;
use diesel::insert_into;
use diesel::prelude::*;

use crate::models::path::{PathExpressionMethods, PrefixPath};
use crate::models::tree::*;
use crate::schema::prefix_tree::dsl::*;

#[derive(Debug)]
pub struct ProbeContext {
    node: PrefixTree,
    ancestors: Vec<PrefixTree>,
    unmerged_children: Vec<PrefixTree>,
}

pub fn fetch(
    connection: &mut PgConnection, target_net: &PrefixPath,
) -> Result<ProbeContext> {
    insert_if_new(connection, target_net)?;

    let ancestors_and_self = select_ancestors_and_self(connection, target_net)
        .with_context(|| "while finding ancestors and self")?;
    let (ancestors, node) = match &ancestors_and_self[..] {
        [parents @ .., node] => (parents.to_vec(), *node),
        [] => bail!("Didn't find the prefix_tree node we just inserted :("),
    };
    let unmerged_children = select_unmerged_children(connection, target_net)?;
    Result::Ok(ProbeContext { node, ancestors, unmerged_children })
}

fn select_ancestors_and_self(
    connection: &mut PgConnection, target_net: &PrefixPath,
) -> Result<Vec<PrefixTree>> {
    let parents = prefix_tree
        .filter(path.ancestor_or_same_as(target_net))
        .select(PrefixTree::as_select())
        .order_by(path)
        .load(connection)
        .with_context(|| "while selecting parents")?;
    Ok(parents)
}

fn select_unmerged_children(
    connection: &mut PgConnection, target_net: &PrefixPath,
) -> Result<Vec<PrefixTree>> {
    let parents = prefix_tree
        .filter(path.descendant_or_same_as(target_net))
        .filter(not(path.eq(target_net)))
        .select(PrefixTree::as_select())
        .order_by(path)
        .load(connection)
        .with_context(|| "while selecting unmerged children")?;
    Ok(parents)
}

fn insert_if_new(connection: &mut PgConnection, target_net: &PrefixPath) -> Result<(), Error> {
    let _inserted_id_or_zero = insert_into(prefix_tree)
        .values((
            path.eq(target_net),
            is_routed.eq(true),
            merge_status.eq(MergeStatus::NotMerged),
            data.eq(ExtraData { ever_responded: true }),
        ))
        .on_conflict_do_nothing()
        .returning(id)
        .execute(connection)
        .with_context(|| "while trying to insert into prefix_tree")?;
    Ok(())
}
