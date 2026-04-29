use acl_engine::{StoreError, TupleStore};
use acl_model::tuple::{ObjectRef, SubjectRef, Tuple};
use async_trait::async_trait;
use sqlx::PgPool;

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
        _object: &ObjectRef,
        _relation: &str,
    ) -> Result<Vec<SubjectRef>, StoreError> {
        todo!()
    }

    async fn read_reverse(&self, _subject: &SubjectRef) -> Result<Vec<Tuple>, StoreError> {
        todo!()
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
}
