use acl_model::tuple::{ObjectRef, ParseError, SubjectRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tuple {
    pub namespace: String,
    pub object_id: String,
    pub relation: String,
    pub subject: Subject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subject {
    User {
        namespace: String,
        id: String,
    },
    UserSet {
        namespace: String,
        object_id: String,
        relation: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRequest {
    pub writes: Vec<Tuple>,
    pub deletes: Vec<Tuple>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResponse {
    pub written: usize,
    pub deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRequest {
    pub namespace: String,
    pub object_id: String,
    pub relation: String,
    pub subject: Subject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResponse {
    pub allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclErrorBody {
    pub error: String,
    pub code: Option<String>,
}

impl From<&SubjectRef> for Subject {
    fn from(s: &SubjectRef) -> Self {
        match s {
            SubjectRef::User {
                object,
                relation: None,
            } => Subject::User {
                namespace: object.namespace().to_string(),
                id: object.id().to_string(),
            },
            SubjectRef::User {
                object,
                relation: Some(rel),
            } => Subject::UserSet {
                namespace: object.namespace().to_string(),
                object_id: object.id().to_string(),
                relation: rel.clone(),
            },
        }
    }
}

impl TryFrom<Subject> for SubjectRef {
    type Error = ParseError;

    fn try_from(s: Subject) -> Result<Self, Self::Error> {
        match s {
            Subject::User { namespace, id } => {
                let obj = ObjectRef::new(namespace, id)?;
                SubjectRef::user(obj, None)
            }
            Subject::UserSet {
                namespace,
                object_id,
                relation,
            } => {
                let obj = ObjectRef::new(namespace, object_id)?;
                SubjectRef::user(obj, Some(relation))
            }
        }
    }
}

impl From<&acl_model::tuple::Tuple> for Tuple {
    fn from(t: &acl_model::tuple::Tuple) -> Self {
        Tuple {
            namespace: t.object().namespace().to_string(),
            object_id: t.object().id().to_string(),
            relation: t.relation().to_string(),
            subject: Subject::from(t.subject()),
        }
    }
}

impl TryFrom<Tuple> for acl_model::tuple::Tuple {
    type Error = ParseError;

    fn try_from(t: Tuple) -> Result<Self, Self::Error> {
        let object = ObjectRef::new(t.namespace, t.object_id)?;
        let subject = SubjectRef::try_from(t.subject)?;
        acl_model::tuple::Tuple::new(object, t.relation, subject)
    }
}
