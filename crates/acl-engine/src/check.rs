use acl_model::schema::{Rewrite, Schema};
use acl_model::tuple::{ObjectRef, SubjectRef};

use crate::store::{StoreError, TupleStore};

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("store: {0}")]
    Store(#[from] StoreError),
    #[error("unknown relation {namespace}#{relation}")]
    UnknownRelation { namespace: String, relation: String },
}

// Checker associated with a generic TupleStore
pub struct Checker<'a, S: TupleStore> {
    schema: &'a Schema,
    store: &'a S,
}

impl<'a, S: TupleStore> Checker<'a, S> {
    pub fn new(schema: &'a Schema, store: &'a S) -> Self {
        Self { schema, store }
    }

    // Given parts of a tuple, check if
    pub async fn check(
        &self,
        object: &ObjectRef,
        relation: &str,
        subject: &SubjectRef,
    ) -> Result<bool, CheckError> {
        let rewrite = self
            .schema
            .get_rewrite(object.namespace(), relation)
            .ok_or_else(|| CheckError::UnknownRelation {
                namespace: object.namespace().to_string(),
                relation: relation.to_string(),
            })?;
        self.check_rewrite(rewrite, object, relation, subject).await
    }

    // Dispatch corresponding checker for Rewrite types
    async fn check_rewrite(
        &self,
        rewrite: &Rewrite,
        object: &ObjectRef,
        relation: &str,
        subject: &SubjectRef,
    ) -> Result<bool, CheckError> {
        match rewrite {
            Rewrite::This { allowed } => {
                self.check_direct(object, relation, subject, allowed).await
            }
            Rewrite::ComputedUserset { relation: rel } => {
                self.check_computed_userset(object, rel, subject).await
            }
            Rewrite::TupleToUserset { tupleset, computed } => {
                self.check_ttu(object, tupleset, computed, subject).await
            }
            Rewrite::Union(children) => self.check_union(children, object, relation, subject).await,
            Rewrite::Intersection(children) => {
                self.check_intersection(children, object, relation, subject)
                    .await
            }
            Rewrite::Exclusion(base, sub) => {
                self.check_exclusion(base, sub, object, relation, subject)
                    .await
            }
        }
    }

    /// Run check on store for this relation
    async fn check_direct(
        &self,
        object: &ObjectRef,
        relation: &str,
        subject: &SubjectRef,
        _allowed: &[acl_model::schema::NamespaceRef],
    ) -> Result<bool, CheckError> {
        let stored = self.store.read_direct(object, relation).await?;
        Ok(stored.iter().any(|s| s == subject))
    }

    /// Re-run check on other relation
    async fn check_computed_userset(
        &self,
        object: &ObjectRef,
        relation: &str,
        subject: &SubjectRef,
    ) -> Result<bool, CheckError> {
        Box::pin(self.check(object, relation, subject)).await
    }

    /// Read tupleset on `object`; for each parent, recurse into `computed` on that parent.
    async fn check_ttu(
        &self,
        object: &ObjectRef,
        tupleset: &str,
        computed: &str,
        subject: &SubjectRef,
    ) -> Result<bool, CheckError> {
        let parents = self.store.read_direct(object, tupleset).await?;
        for parent in &parents {
            let parent_obj = match parent {
                SubjectRef::User { object, .. } => object,
            };
            if Box::pin(self.check(parent_obj, computed, subject)).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    // TODO: union, intersection, and exclusion can and should be parellilzed
    async fn check_union(
        &self,
        children: &[Rewrite],
        object: &ObjectRef,
        relation: &str,
        subject: &SubjectRef,
    ) -> Result<bool, CheckError> {
        for child in children {
            // Check if any of the union'd rewrites pass the check; short circuit if so
            if Box::pin(self.check_rewrite(child, object, relation, subject)).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn check_intersection(
        &self,
        children: &[Rewrite],
        object: &ObjectRef,
        relation: &str,
        subject: &SubjectRef,
    ) -> Result<bool, CheckError> {
        for child in children {
            // Check if any of the union'd rewrites don't pass the check; short circuit if not
            if !Box::pin(self.check_rewrite(child, object, relation, subject)).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn check_exclusion(
        &self,
        base: &Rewrite,
        sub: &Rewrite,
        object: &ObjectRef,
        relation: &str,
        subject: &SubjectRef,
    ) -> Result<bool, CheckError> {
        let base_check = Box::pin(self.check_rewrite(base, object, relation, subject)).await?;
        // short circuit if base fails check
        if !base_check {
            return Ok(false);
        }
        let sub_check = Box::pin(self.check_rewrite(sub, object, relation, subject)).await?;
        Ok(!sub_check)
    }
}
