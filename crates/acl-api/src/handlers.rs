use acl_engine::{Checker, TupleStore};
use acl_model::tuple::{ObjectRef, SubjectRef, Tuple as AclTuple};
use axum::{extract::State, Json};
use securebase_proto::acl::{
    CheckRequest, CheckResponse, WriteRequest, WriteResponse,
};

use crate::{error::AclError, AppState};

pub(crate) async fn check(
    State(state): State<AppState>,
    Json(req): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, AclError> {
    let object = ObjectRef::new(req.namespace, req.object_id)?;
    let subject: SubjectRef = req.subject.try_into()?;
    let allowed = Checker::new(&state.schema, state.store.as_ref())
        .check(&object, &req.relation, &subject)
        .await?;
    Ok(Json(CheckResponse { allowed }))
}

pub(crate) async fn write(
    State(state): State<AppState>,
    Json(req): Json<WriteRequest>,
) -> Result<Json<WriteResponse>, AclError> {
    let writes: Vec<AclTuple> = req
        .writes
        .into_iter()
        .map(AclTuple::try_from)
        .collect::<Result<_, _>>()?;
    let deletes: Vec<AclTuple> = req
        .deletes
        .into_iter()
        .map(AclTuple::try_from)
        .collect::<Result<_, _>>()?;
    let written = writes.len();
    let deleted = deletes.len();
    state.store.write(writes, deletes).await?;
    Ok(Json(WriteResponse { written, deleted }))
}

