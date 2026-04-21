/// A relation tuple: resource_type:resource_id#relation@subject
///
/// Example: document:readme#viewer@user:alice
///          document:readme#viewer@group:eng#member
pub struct Tuple {
    pub resource_type: String,
    pub resource_id: String,
    pub relation: String,
    pub subject: Subject,
}

/// A subject is either a direct user or a userset (e.g. group:eng#member)
pub enum Subject {
    User(String),
    UserSet {
        resource_type: String,
        resource_id: String,
        relation: String,
    },
}

pub struct WriteRequest {
    pub tuple: Tuple,
    pub op: WriteOp,
}

pub enum WriteOp {
    Insert,
    Delete,
}

pub struct CheckRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub permission: String,
    pub subject: Subject,
}

pub struct CheckResponse {
    pub allowed: bool,
}

pub struct ExpandRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub permission: String,
}

/// Tree of subjects that have the given permission on the resource
pub enum ExpandResponse {
    Leaf(Subject),
    Union(Vec<ExpandResponse>),
    Intersection(Vec<ExpandResponse>),
}
