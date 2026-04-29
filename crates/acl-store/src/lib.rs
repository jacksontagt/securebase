use acl_engine::{StoreError, TupleStore};
use acl_model::tuple::{ObjectRef, SubjectRef, Tuple};
use async_trait::async_trait;
use sqlx::{PgPool, Row};

pub struct PostgresTupleStore {
    pool: PgPool,
}

impl PostgresTupleStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// Maps a SubjectRef to the three DB columns
// Empty string sentinel for subject_relation means "no relation" (direct user)
fn subject_to_parts(s: &SubjectRef) -> (String, String, String) {
    match s {
        SubjectRef::User { object, relation } => (
            object.namespace().to_string(),
            object.id().to_string(),
            relation.clone().unwrap_or_default(),
        ),
    }
}

type Cols = (
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
);

fn tuples_to_cols(tuples: &[Tuple]) -> Cols {
    let mut on = Vec::with_capacity(tuples.len());
    let mut oi = Vec::with_capacity(tuples.len());
    let mut r = Vec::with_capacity(tuples.len());
    let mut sn = Vec::with_capacity(tuples.len());
    let mut si = Vec::with_capacity(tuples.len());
    let mut sr = Vec::with_capacity(tuples.len());
    for t in tuples {
        let (tsn, tsi, tsr) = subject_to_parts(t.subject());
        on.push(t.object().namespace().to_string());
        oi.push(t.object().id().to_string());
        r.push(t.relation().to_string());
        sn.push(tsn);
        si.push(tsi);
        sr.push(tsr);
    }
    (on, oi, r, sn, si, sr)
}

fn row_to_subject(ns: &str, id: &str, rel: &str) -> Result<SubjectRef, StoreError> {
    let obj = ObjectRef::new(ns, id).map_err(|e| StoreError::CorruptData(e.to_string()))?;
    let relation = if rel.is_empty() {
        None
    } else {
        Some(rel.to_string())
    };
    SubjectRef::user(obj, relation).map_err(|e| StoreError::CorruptData(e.to_string()))
}

fn row_to_tuple(
    obj_ns: &str,
    obj_id: &str,
    rel: &str,
    subj_ns: &str,
    subj_id: &str,
    subj_rel: &str,
) -> Result<Tuple, StoreError> {
    let obj = ObjectRef::new(obj_ns, obj_id).map_err(|e| StoreError::CorruptData(e.to_string()))?;
    let subj = row_to_subject(subj_ns, subj_id, subj_rel)?;
    Tuple::new(obj, rel, subj).map_err(|e| StoreError::CorruptData(e.to_string()))
}

#[async_trait]
impl TupleStore for PostgresTupleStore {
    async fn write(&self, writes: Vec<Tuple>, deletes: Vec<Tuple>) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(StoreError::backend)?;
        if !writes.is_empty() {
            let (on, oi, r, sn, si, sr) = tuples_to_cols(&writes);
            sqlx::query(
                "INSERT INTO acl.tuples
                    (object_namespace, object_id, relation,
                     subject_namespace, subject_id, subject_relation)
                 SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[])
                 ON CONFLICT DO NOTHING",
            )
            .bind(on)
            .bind(oi)
            .bind(r)
            .bind(sn)
            .bind(si)
            .bind(sr)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::backend)?;
        }
        if !deletes.is_empty() {
            let (on, oi, r, sn, si, sr) = tuples_to_cols(&deletes);
            sqlx::query(
                "DELETE FROM acl.tuples
                 WHERE (object_namespace, object_id, relation,
                        subject_namespace, subject_id, subject_relation)
                 IN (SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[]))",
            )
            .bind(on)
            .bind(oi)
            .bind(r)
            .bind(sn)
            .bind(si)
            .bind(sr)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::backend)?;
        }
        tx.commit().await.map_err(StoreError::backend)?;
        Ok(())
    }

    async fn read_direct(
        &self,
        object: &ObjectRef,
        relation: &str,
    ) -> Result<Vec<SubjectRef>, StoreError> {
        let rows = sqlx::query(
            "SELECT subject_namespace, subject_id, subject_relation
             FROM acl.tuples
             WHERE object_namespace=$1 AND object_id=$2 AND relation=$3",
        )
        .bind(object.namespace())
        .bind(object.id())
        .bind(relation)
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::backend)?;

        rows.iter()
            .map(|r| {
                let ns: String = r.get("subject_namespace");
                let id: String = r.get("subject_id");
                let rel: String = r.get("subject_relation");
                row_to_subject(&ns, &id, &rel)
            })
            .collect()
    }

    async fn read_reverse(&self, subject: &SubjectRef) -> Result<Vec<Tuple>, StoreError> {
        let (sn, si, sr) = subject_to_parts(subject);
        let rows = sqlx::query(
            "SELECT object_namespace, object_id, relation,
                    subject_namespace, subject_id, subject_relation
             FROM acl.tuples
             WHERE subject_namespace=$1 AND subject_id=$2 AND subject_relation=$3",
        )
        .bind(&sn)
        .bind(&si)
        .bind(&sr)
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::backend)?;

        rows.iter()
            .map(|r| {
                let obj_ns: String = r.get("object_namespace");
                let obj_id: String = r.get("object_id");
                let rel: String = r.get("relation");
                let subj_ns: String = r.get("subject_namespace");
                let subj_id: String = r.get("subject_id");
                let subj_rel: String = r.get("subject_relation");
                row_to_tuple(&obj_ns, &obj_id, &rel, &subj_ns, &subj_id, &subj_rel)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    fn direct_tuple(obj_ns: &str, obj_id: &str, rel: &str, subj_ns: &str, subj_id: &str) -> Tuple {
        let obj = ObjectRef::new(obj_ns, obj_id).unwrap();
        let subj_obj = ObjectRef::new(subj_ns, subj_id).unwrap();
        let subj = SubjectRef::user(subj_obj, None).unwrap();
        Tuple::new(obj, rel, subj).unwrap()
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn write_inserts_row(pool: PgPool) {
        let store = PostgresTupleStore::new(pool.clone());
        store
            .write(
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
                vec![],
            )
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM acl.tuples")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn write_duplicate_is_idempotent(pool: PgPool) {
        let store = PostgresTupleStore::new(pool.clone());
        store
            .write(
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
                vec![],
            )
            .await
            .unwrap();
        store
            .write(
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
                vec![],
            )
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM acl.tuples")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn delete_removes_row(pool: PgPool) {
        let store = PostgresTupleStore::new(pool.clone());
        store
            .write(
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
                vec![],
            )
            .await
            .unwrap();
        store
            .write(
                vec![],
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
            )
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM acl.tuples")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn delete_nonexistent_is_idempotent(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        store
            .write(
                vec![],
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
            )
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn write_and_delete_in_same_call(pool: PgPool) {
        let store = PostgresTupleStore::new(pool.clone());
        store
            .write(
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
            )
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM acl.tuples")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_direct_returns_subject(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        store
            .write(
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
                vec![],
            )
            .await
            .unwrap();
        let obj = ObjectRef::new("document", "readme").unwrap();
        let subjects = store.read_direct(&obj, "viewer").await.unwrap();
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].to_string(), "user:alice");
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_direct_filters_by_relation(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        store
            .write(
                vec![
                    direct_tuple("document", "readme", "viewer", "user", "alice"),
                    direct_tuple("document", "readme", "editor", "user", "bob"),
                ],
                vec![],
            )
            .await
            .unwrap();
        let obj = ObjectRef::new("document", "readme").unwrap();
        let subjects = store.read_direct(&obj, "viewer").await.unwrap();
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].to_string(), "user:alice");
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_direct_empty_for_unknown_object(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        let obj = ObjectRef::new("document", "no-such-doc").unwrap();
        let subjects = store.read_direct(&obj, "viewer").await.unwrap();
        assert!(subjects.is_empty());
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_direct_returns_userset_subject(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        let obj = ObjectRef::new("document", "readme").unwrap();
        let group_obj = ObjectRef::new("group", "eng").unwrap();
        let group_subj = SubjectRef::user(group_obj, Some("member".into())).unwrap();
        let t = Tuple::new(obj, "viewer", group_subj).unwrap();
        store.write(vec![t], vec![]).await.unwrap();
        let obj2 = ObjectRef::new("document", "readme").unwrap();
        let subjects = store.read_direct(&obj2, "viewer").await.unwrap();
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].to_string(), "group:eng#member");
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_reverse_direct_user(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        store
            .write(
                vec![direct_tuple(
                    "document", "readme", "viewer", "user", "alice",
                )],
                vec![],
            )
            .await
            .unwrap();
        let subj = SubjectRef::user(ObjectRef::new("user", "alice").unwrap(), None).unwrap();
        let tuples = store.read_reverse(&subj).await.unwrap();
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].to_string(), "document:readme#viewer@user:alice");
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_reverse_userset(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        let obj = ObjectRef::new("document", "readme").unwrap();
        let group_subj = SubjectRef::user(
            ObjectRef::new("group", "eng").unwrap(),
            Some("member".into()),
        )
        .unwrap();
        let t = Tuple::new(obj, "viewer", group_subj).unwrap();
        store.write(vec![t], vec![]).await.unwrap();
        let query_subj = SubjectRef::user(
            ObjectRef::new("group", "eng").unwrap(),
            Some("member".into()),
        )
        .unwrap();
        let tuples = store.read_reverse(&query_subj).await.unwrap();
        assert_eq!(tuples.len(), 1);
        assert_eq!(
            tuples[0].to_string(),
            "document:readme#viewer@group:eng#member"
        );
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_reverse_userset_does_not_match_direct(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        // Write a direct-user tuple for group:eng (no relation)
        store
            .write(
                vec![direct_tuple("document", "readme", "viewer", "group", "eng")],
                vec![],
            )
            .await
            .unwrap();
        // Query for userset group:eng#member — must NOT match the direct tuple
        let query_subj = SubjectRef::user(
            ObjectRef::new("group", "eng").unwrap(),
            Some("member".into()),
        )
        .unwrap();
        let tuples = store.read_reverse(&query_subj).await.unwrap();
        assert!(tuples.is_empty());
    }

    #[sqlx::test(migrations = "../../migrations/acl")]
    async fn read_reverse_wildcard(pool: PgPool) {
        let store = PostgresTupleStore::new(pool);
        let obj = ObjectRef::new("document", "readme").unwrap();
        let t = Tuple::new(obj, "viewer", SubjectRef::Wildcard).unwrap();
        store.write(vec![t], vec![]).await.unwrap();
        let tuples = store.read_reverse(&SubjectRef::Wildcard).await.unwrap();
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].to_string(), "document:readme#viewer@*");
    }
}
