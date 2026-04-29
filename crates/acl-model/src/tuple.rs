use std::fmt;
use std::str::FromStr;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("missing '#' relation separator")]
    MissingRelationSeparator,
    #[error("missing '@' subject separator")]
    MissingSubjectSeparator,
    #[error("missing ':' namespace separator in '{0}'")]
    MissingNamespaceSeparator(String),
    #[error("empty {0}")]
    EmptyComponent(&'static str),
    #[error("component contains reserved character: {0:?}")]
    ReservedCharacter(char),
}

fn validate_component(name: &'static str, s: &str) -> Result<(), ParseError> {
    if s.is_empty() {
        return Err(ParseError::EmptyComponent(name));
    }
    if let Some(c) = s.chars().find(|&c| matches!(c, ':' | '#' | '@')) {
        return Err(ParseError::ReservedCharacter(c));
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct ObjectRef {
    namespace: String,
    id: String,
}

impl ObjectRef {
    pub fn new(namespace: impl Into<String>, id: impl Into<String>) -> Result<Self, ParseError> {
        let namespace = namespace.into();
        let id = id.into();
        validate_component("namespace", &namespace)?;
        validate_component("id", &id)?;
        Ok(Self { namespace, id })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.id)
    }
}

#[derive(Debug, PartialEq)]
pub enum SubjectRef {
    User {
        object: ObjectRef,
        relation: Option<String>,
    },
    Wildcard,
}

impl SubjectRef {
    pub fn user(object: ObjectRef, relation: Option<String>) -> Result<Self, ParseError> {
        if let Some(ref r) = relation {
            validate_component("relation", r)?;
        }
        Ok(Self::User { object, relation })
    }

    pub fn wildcard() -> Self {
        Self::Wildcard
    }
}

impl fmt::Display for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wildcard => write!(f, "*"),
            Self::User {
                object,
                relation: None,
            } => write!(f, "{object}"),
            Self::User {
                object,
                relation: Some(r),
            } => write!(f, "{object}#{r}"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Tuple {
    object: ObjectRef,
    relation: String,
    subject: SubjectRef,
}

impl Tuple {
    pub fn new(
        object: ObjectRef,
        relation: impl Into<String>,
        subject: SubjectRef,
    ) -> Result<Self, ParseError> {
        let relation = relation.into();
        validate_component("relation", &relation)?;
        Ok(Self {
            object,
            relation,
            subject,
        })
    }

    pub fn object(&self) -> &ObjectRef {
        &self.object
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub fn subject(&self) -> &SubjectRef {
        &self.subject
    }
}

fn parse_object_ref(s: &str) -> Result<ObjectRef, ParseError> {
    let colon = s
        .find(':')
        .ok_or_else(|| ParseError::MissingNamespaceSeparator(s.to_string()))?;
    ObjectRef::new(&s[..colon], &s[colon + 1..])
}

impl FromStr for Tuple {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Split on first '@' to isolate the subject token.
        let at = s.find('@').ok_or(ParseError::MissingSubjectSeparator)?;
        let left = &s[..at];
        let right = &s[at + 1..];

        // Split left on last '#' to isolate the relation.
        // Using rfind so that object ids containing '#' (not valid per validation,
        // but defensive) don't confuse the split.
        let hash = left
            .rfind('#')
            .ok_or(ParseError::MissingRelationSeparator)?;
        let object_str = &left[..hash];
        let relation = &left[hash + 1..];

        let object = parse_object_ref(object_str)?;
        validate_component("relation", relation)?;

        let subject = if right == "*" {
            SubjectRef::Wildcard
        } else {
            let (subj_obj_str, subj_rel) = match right.find('#') {
                Some(h) => (&right[..h], Some(right[h + 1..].to_string())),
                None => (right, None),
            };
            let subj_obj = parse_object_ref(subj_obj_str)?;
            SubjectRef::user(subj_obj, subj_rel)?
        };

        Tuple::new(object, relation, subject)
    }
}

impl fmt::Display for Tuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}@{}", self.object, self.relation, self.subject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_ref_display() {
        let o = ObjectRef::new("document", "readme").unwrap();
        assert_eq!(o.to_string(), "document:readme");
        assert_eq!(o.namespace(), "document");
        assert_eq!(o.id(), "readme");
    }

    #[test]
    fn object_ref_empty_namespace() {
        assert_eq!(
            ObjectRef::new("", "id"),
            Err(ParseError::EmptyComponent("namespace"))
        );
    }

    #[test]
    fn object_ref_empty_id() {
        assert_eq!(
            ObjectRef::new("ns", ""),
            Err(ParseError::EmptyComponent("id"))
        );
    }

    #[test]
    fn object_ref_reserved_char_in_namespace() {
        assert_eq!(
            ObjectRef::new("doc:ument", "id"),
            Err(ParseError::ReservedCharacter(':'))
        );
    }

    #[test]
    fn object_ref_reserved_char_at_sign() {
        assert_eq!(
            ObjectRef::new("doc", "id@bad"),
            Err(ParseError::ReservedCharacter('@'))
        );
    }

    #[test]
    fn object_ref_hash_eq() {
        let a = ObjectRef::new("document", "readme").unwrap();
        let b = ObjectRef::new("document", "readme").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn subject_ref_direct_user_display() {
        let obj = ObjectRef::new("user", "alice").unwrap();
        let s = SubjectRef::user(obj, None).unwrap();
        assert_eq!(s.to_string(), "user:alice");
    }

    #[test]
    fn subject_ref_userset_display() {
        let obj = ObjectRef::new("group", "eng").unwrap();
        let s = SubjectRef::user(obj, Some("member".into())).unwrap();
        assert_eq!(s.to_string(), "group:eng#member");
    }

    #[test]
    fn subject_ref_wildcard_display() {
        assert_eq!(SubjectRef::wildcard().to_string(), "*");
    }

    #[test]
    fn subject_ref_empty_relation_rejected() {
        let obj = ObjectRef::new("group", "eng").unwrap();
        assert_eq!(
            SubjectRef::user(obj, Some("".into())),
            Err(ParseError::EmptyComponent("relation"))
        );
    }

    #[test]
    fn subject_ref_reserved_char_in_relation() {
        let obj = ObjectRef::new("group", "eng").unwrap();
        assert_eq!(
            SubjectRef::user(obj, Some("mem#ber".into())),
            Err(ParseError::ReservedCharacter('#'))
        );
    }

    #[test]
    fn subject_ref_hash_eq() {
        let obj_a = ObjectRef::new("user", "alice").unwrap();
        let obj_b = ObjectRef::new("user", "alice").unwrap();
        let a = SubjectRef::user(obj_a, None).unwrap();
        let b = SubjectRef::user(obj_b, None).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn roundtrip_direct_user() {
        let s = "document:readme#viewer@user:alice";
        assert_eq!(s.parse::<Tuple>().unwrap().to_string(), s);
    }

    #[test]
    fn roundtrip_userset() {
        let s = "document:readme#viewer@group:eng#member";
        assert_eq!(s.parse::<Tuple>().unwrap().to_string(), s);
    }

    #[test]
    fn roundtrip_wildcard() {
        let s = "document:readme#viewer@*";
        assert_eq!(s.parse::<Tuple>().unwrap().to_string(), s);
    }

    #[test]
    fn roundtrip_uuid_id() {
        let s = "document:550e8400-e29b-41d4-a716-446655440000#owner@user:abc123";
        assert_eq!(s.parse::<Tuple>().unwrap().to_string(), s);
    }

    #[test]
    fn parse_missing_at() {
        assert_eq!(
            "document:readme#viewer".parse::<Tuple>(),
            Err(ParseError::MissingSubjectSeparator)
        );
    }

    #[test]
    fn parse_missing_hash() {
        assert_eq!(
            "document:readme@user:alice".parse::<Tuple>(),
            Err(ParseError::MissingRelationSeparator)
        );
    }

    #[test]
    fn parse_missing_colon_in_object() {
        assert_eq!(
            "documentreadme#viewer@user:alice".parse::<Tuple>(),
            Err(ParseError::MissingNamespaceSeparator(
                "documentreadme".into()
            ))
        );
    }

    #[test]
    fn parse_empty_relation() {
        assert_eq!(
            "document:readme#@user:alice".parse::<Tuple>(),
            Err(ParseError::EmptyComponent("relation"))
        );
    }

    #[test]
    fn parse_empty_namespace() {
        assert_eq!(
            ":readme#viewer@user:alice".parse::<Tuple>(),
            Err(ParseError::EmptyComponent("namespace"))
        );
    }

    #[test]
    fn tuple_new_validates_relation() {
        let obj = ObjectRef::new("document", "readme").unwrap();
        let subj = SubjectRef::wildcard();
        assert_eq!(
            Tuple::new(obj, "", subj),
            Err(ParseError::EmptyComponent("relation"))
        );
    }

    #[test]
    fn tuple_hash_eq() {
        let s = "document:readme#viewer@user:alice";
        let a: Tuple = s.parse().unwrap();
        let b: Tuple = s.parse().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn tuple_accessors() {
        let t: Tuple = "document:readme#viewer@user:alice".parse().unwrap();
        assert_eq!(t.object().namespace(), "document");
        assert_eq!(t.object().id(), "readme");
        assert_eq!(t.relation(), "viewer");
        assert!(matches!(t.subject(), SubjectRef::User { .. }));
    }
}
