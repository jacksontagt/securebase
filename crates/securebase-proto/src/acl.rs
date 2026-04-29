/// A relation tuple: namespace:object_id#relation@subject
///
/// Example: document:readme#viewer@user:alice
///          document:readme#viewer@group:eng#member
pub struct Tuple {
    pub namespace: String,
    pub object_id: String,
    pub relation: String,
    pub subject: Subject,
}

/// A subject is either a direct user or a userset (e.g. group:eng#member)
pub enum Subject {
    User(String),
    UserSet {
        namespace: String,
        object_id: String,
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
    pub namespace: String,
    pub object_id: String,
    pub relation: String,
    pub subject: Subject,
}

pub struct CheckResponse {
    pub allowed: bool,
}

pub struct ExpandRequest {
    pub namespace: String,
    pub object_id: String,
    pub relation: String,
}

/// Tree of subjects that have the given relation on the object
pub enum ExpandResponse {
    Leaf(Subject),
    Union(Vec<ExpandResponse>),
    Intersection(Vec<ExpandResponse>),
}
