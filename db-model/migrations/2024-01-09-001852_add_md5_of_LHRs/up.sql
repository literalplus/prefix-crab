-- a uuid is just a u128 if you squint a little bit
-- ref: https://dba.stackexchange.com/a/115316
ALTER TABLE prefix_tree
    ADD COLUMN lhr_set_hash uuid NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000'::uuid;
