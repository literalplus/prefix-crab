DROP INDEX IF EXISTS prefix_tree_gist_idx;
DROP TABLE IF EXISTS prefix_tree;
DROP TYPE IF EXISTS prefix_merge_status;

-- not sure how safe this would be, as we might have had it loaded before
-- DROP EXTENSION "ltree";
