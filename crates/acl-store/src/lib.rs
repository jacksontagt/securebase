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
        for t in &writes {
            let (sn, si, sr) = subject_to_parts(t.subject());
            sqlx::query(
                "INSERT INTO acl.tuples
                    (object_namespace, object_id, relation,
                     subject_namespace, subject_id, subject_relation)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT DO NOTHING",
            )
            .bind(t.object().namespace())
            .bind(t.object().id())
            .bind(t.relation())
            .bind(&sn)
            .bind(&si)
            .bind(&sr)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::backend)?;
        }
        for t in &deletes {
            let (sn, si, sr) = subject_to_parts(t.subject());
            sqlx::query(
                "DELETE FROM acl.tuples
                 WHERE object_namespace=$1 AND object_id=$2 AND relation=$3
                   AND subject_namespace=$4 AND subject_id=$5 AND subject_relation=$6",
            )
            .bind(t.object().namespace())
            .bind(t.object().id())
            .bind(t.relation())
            .bind(&sn)
            .bind(&si)
            .bind(&sr)
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
