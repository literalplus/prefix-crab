-- We're currently using this just for rate limiting (and anyways just cleared the DB),
-- so a default is fine. 0 is technically reserved for RPKI unallocated space, but just
-- more intuitive that it's not supposed to be there for debugging. We shouldn't get
-- RPKI unallocated space anyways in our data (:
ALTER TABLE prefix_tree
    ADD COLUMN asn bigint NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000'::uuid;
