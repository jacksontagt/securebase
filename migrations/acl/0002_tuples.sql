CREATE TABLE acl.tuples (
    object_namespace  TEXT NOT NULL,
    object_id         TEXT NOT NULL,
    relation          TEXT NOT NULL,
    subject_namespace TEXT NOT NULL,
    subject_id        TEXT NOT NULL,
    subject_relation  TEXT NOT NULL DEFAULT '',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (object_namespace, object_id, relation,
                 subject_namespace, subject_id, subject_relation)
);

CREATE INDEX tuples_reverse
    ON acl.tuples (subject_namespace, subject_id, subject_relation);

GRANT SELECT, INSERT, DELETE ON acl.tuples TO acl_service;
