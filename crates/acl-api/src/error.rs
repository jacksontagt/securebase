use acl_engine::{CheckError, StoreError};
use acl_model::ParseError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use securebase_proto::acl::AclErrorBody;

#[derive(Debug, thiserror::Error)]
pub enum AclError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unknown relation {namespace}#{relation}")]
    UnknownRelation { namespace: String, relation: String },
    #[error("store: {0}")]
    Store(#[from] StoreError),
}

impl From<ParseError> for AclError {
    fn from(e: ParseError) -> Self {
        AclError::BadRequest(e.to_string())
    }
}

impl From<CheckError> for AclError {
    fn from(e: CheckError) -> Self {
        match e {
            CheckError::UnknownRelation {
                namespace,
                relation,
            } => AclError::UnknownRelation {
                namespace,
                relation,
            },
            CheckError::Store(s) => AclError::Store(s),
        }
    }
}

impl IntoResponse for AclError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AclError::BadRequest(_) => (StatusCode::BAD_REQUEST, Some("bad_request")),
            AclError::UnknownRelation { .. } => (StatusCode::NOT_FOUND, Some("unknown_relation")),
            AclError::Store(_) => (StatusCode::INTERNAL_SERVER_ERROR, Some("store_error")),
        };
        let body = AclErrorBody {
            error: self.to_string(),
            code: code.map(String::from),
        };
        (status, Json(body)).into_response()
    }
}
