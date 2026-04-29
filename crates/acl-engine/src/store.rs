use acl_model::tuple::{ObjectRef, SubjectRef, Tuple};
use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage backend: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("corrupt stored data: {0}")]
    CorruptData(String),
}

impl StoreError {
    pub fn backend(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        StoreError::Backend(Box::new(e))
    }
}

#[async_trait]
pub trait TupleStore: Send + Sync {
    async fn write(&self, writes: Vec<Tuple>, deletes: Vec<Tuple>) -> Result<(), StoreError>;

    /// Get all subjects of some [object] with [relation]
    async fn read_direct(
        &self,
        object: &ObjectRef,
        relation: &str,
    ) -> Result<Vec<SubjectRef>, StoreError>;

    /// Get all relations (and objects) for [subject]
    async fn read_reverse(&self, subject: &SubjectRef) -> Result<Vec<Tuple>, StoreError>;
}
