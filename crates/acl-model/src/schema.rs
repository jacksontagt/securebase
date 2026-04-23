mod parser;
mod validator;

use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct TypeRef {
    pub type_name: String,
    pub subject: TypeRefKind,
}

#[derive(Debug, Clone)]
pub enum TypeRefKind {
    Direct,
    Wildcard,
    Userset(String),
}

#[derive(Debug, Clone)]
pub enum Rewrite {
    This { allowed: Vec<TypeRef> },
    ComputedUserset { relation: String },
    TupleToUserset { tupleset: String, computed: String },
    Union(Vec<Rewrite>),
    Intersection(Vec<Rewrite>),
    Exclusion(Box<Rewrite>, Box<Rewrite>),
}

#[derive(Debug)]
pub struct TypeDef {
    pub name: String,
    pub relations: HashMap<String, Rewrite>,
}

#[derive(Debug)]
pub struct Schema {
    types: HashMap<String, TypeDef>,
}

impl Schema {
    pub(crate) fn new(types: HashMap<String, TypeDef>) -> Self {
        Self { types }
    }

    pub fn get_rewrite(&self, type_name: &str, relation: &str) -> Option<&Rewrite> {
        self.types.get(type_name)?.relations.get(relation)
    }

    pub fn has_type(&self, type_name: &str) -> bool {
        self.types.contains_key(type_name)
    }

    pub fn type_def(&self, type_name: &str) -> Option<&TypeDef> {
        self.types.get(type_name)
    }

    pub fn type_count(&self) -> usize {
        self.types.len()
    }
}

#[derive(Debug)]
pub enum SchemaError {
    Parse {
        message: String,
        span: Range<usize>,
    },
    UndefinedRelation {
        type_name: String,
        relation: String,
        referenced_from: String,
    },
}

pub fn parse_schema(input: &str) -> Result<Schema, Vec<SchemaError>> {
    let (output, parse_errors) = parser::parse(input);
    if !parse_errors.is_empty() {
        return Err(parse_errors);
    }
    let mut types = HashMap::new();
    for raw in output.unwrap_or_default() {
        let mut relations = HashMap::new();
        for (name, rewrite) in raw.relations {
            relations.insert(name, rewrite);
        }
        types.insert(raw.name.clone(), TypeDef { name: raw.name, relations });
    }
    let validation_errors = validator::validate(&types);
    if !validation_errors.is_empty() {
        return Err(validation_errors);
    }
    Ok(Schema::new(types))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_variants_construct() {
        let _this = Rewrite::This { allowed: vec![] };
        let _cu = Rewrite::ComputedUserset { relation: "editor".into() };
        let _ttu = Rewrite::TupleToUserset { tupleset: "parent".into(), computed: "viewer".into() };
        let _union = Rewrite::Union(vec![
            Rewrite::This { allowed: vec![] },
            Rewrite::ComputedUserset { relation: "editor".into() },
        ]);
        let _intersection = Rewrite::Intersection(vec![
            Rewrite::This { allowed: vec![] },
            Rewrite::ComputedUserset { relation: "member".into() },
        ]);
        let _exclusion = Rewrite::Exclusion(
            Box::new(Rewrite::This { allowed: vec![] }),
            Box::new(Rewrite::ComputedUserset { relation: "blocked".into() }),
        );
    }

    #[test]
    fn type_ref_kinds_construct() {
        let _direct = TypeRef { type_name: "user".into(), subject: TypeRefKind::Direct };
        let _wildcard = TypeRef { type_name: "user".into(), subject: TypeRefKind::Wildcard };
        let _userset = TypeRef { type_name: "group".into(), subject: TypeRefKind::Userset("member".into()) };
    }

    #[test]
    fn schema_get_rewrite() {
        let mut relations = HashMap::new();
        relations.insert(
            "viewer".into(),
            Rewrite::This { allowed: vec![TypeRef { type_name: "user".into(), subject: TypeRefKind::Direct }] },
        );
        let mut types = HashMap::new();
        types.insert("doc".into(), TypeDef { name: "doc".into(), relations });
        let schema = Schema::new(types);

        assert!(matches!(schema.get_rewrite("doc", "viewer"), Some(Rewrite::This { .. })));
        assert!(schema.get_rewrite("doc", "nonexistent").is_none());
        assert!(schema.get_rewrite("missing_type", "viewer").is_none());
        assert!(schema.has_type("doc"));
        assert!(!schema.has_type("missing_type"));
    }

    #[test]
    fn parse_schema_empty() {
        assert!(parse_schema("").is_ok());
    }

    #[test]
    fn parse_leaf_type() {
        let schema = parse_schema("type user").unwrap();
        assert!(schema.has_type("user"));
        assert!(schema.get_rewrite("user", "x").is_none());
    }

    #[test]
    fn parse_direct_relation() {
        let schema = parse_schema("type doc\n  relations\n    define owner: [user]").unwrap();
        assert!(matches!(schema.get_rewrite("doc", "owner"), Some(Rewrite::This { .. })));
    }

    #[test]
    fn parse_type_restrictions_multiple() {
        let schema = parse_schema(
            "type group\n  relations\n    define member: [user, group#member, user:*]",
        )
        .unwrap();
        let rewrite = schema.get_rewrite("group", "member").unwrap();
        let Rewrite::This { allowed } = rewrite else { panic!("expected This") };
        assert_eq!(allowed.len(), 3);
        assert!(matches!(allowed[0].subject, TypeRefKind::Direct));
        assert!(matches!(&allowed[1].subject, TypeRefKind::Userset(r) if r == "member"));
        assert!(matches!(allowed[2].subject, TypeRefKind::Wildcard));
    }

    #[test]
    fn parse_computed_userset() {
        let schema = parse_schema(
            "type doc\n  relations\n    define editor: [user]\n    define viewer: editor",
        )
        .unwrap();
        assert!(matches!(
            schema.get_rewrite("doc", "viewer"),
            Some(Rewrite::ComputedUserset { relation }) if relation == "editor"
        ));
    }

    #[test]
    fn parse_tuple_to_userset() {
        let schema = parse_schema(
            "type doc\n  relations\n    define parent: [folder]\n    define viewer: viewer from parent",
        )
        .unwrap();
        assert!(matches!(
            schema.get_rewrite("doc", "viewer"),
            Some(Rewrite::TupleToUserset { tupleset, computed })
                if tupleset == "parent" && computed == "viewer"
        ));
    }

    #[test]
    fn parse_multiple_types() {
        let schema = parse_schema("type user\ntype group\n  relations\n    define member: [user]").unwrap();
        assert!(schema.has_type("user"));
        assert!(schema.has_type("group"));
    }

    #[test]
    fn parse_syntax_error_returns_err() {
        let result = parse_schema("define :"); // missing type block
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(!errs.is_empty());
        assert!(matches!(errs[0], SchemaError::Parse { .. }));
    }

    // --- Step 3: composite rewrites ---

    #[test]
    fn parse_union_two() {
        let schema = parse_schema(
            "type doc\n  relations\n    define editor: [user]\n    define viewer: [user] or editor",
        )
        .unwrap();
        let r = schema.get_rewrite("doc", "viewer").unwrap();
        let Rewrite::Union(v) = r else { panic!("expected Union, got {r:?}") };
        assert_eq!(v.len(), 2);
        assert!(matches!(v[0], Rewrite::This { .. }));
        assert!(matches!(&v[1], Rewrite::ComputedUserset { relation } if relation == "editor"));
    }

    #[test]
    fn parse_union_flattened() {
        // A or B or C → Union([A, B, C]), not Union([Union([A, B]), C])
        let schema = parse_schema(
            "type doc\n  relations\n    define parent: [folder]\n    define editor: [user]\n    define viewer: [user] or editor or viewer from parent",
        )
        .unwrap();
        let r = schema.get_rewrite("doc", "viewer").unwrap();
        let Rewrite::Union(v) = r else { panic!("expected Union, got {r:?}") };
        assert_eq!(v.len(), 3);
        assert!(matches!(v[0], Rewrite::This { .. }));
        assert!(matches!(&v[1], Rewrite::ComputedUserset { relation } if relation == "editor"));
        assert!(matches!(&v[2], Rewrite::TupleToUserset { tupleset, computed } if tupleset == "parent" && computed == "viewer"));
    }

    #[test]
    fn parse_intersection() {
        let schema = parse_schema(
            "type doc\n  relations\n    define member: [user]\n    define viewer: [user] and member",
        )
        .unwrap();
        let r = schema.get_rewrite("doc", "viewer").unwrap();
        let Rewrite::Intersection(v) = r else { panic!("expected Intersection, got {r:?}") };
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn parse_exclusion() {
        let schema = parse_schema(
            "type doc\n  relations\n    define blocked: [user]\n    define viewer: [user] but not blocked",
        )
        .unwrap();
        assert!(matches!(
            schema.get_rewrite("doc", "viewer"),
            Some(Rewrite::Exclusion(_, _))
        ));
    }

    #[test]
    fn parse_parentheses_grouping() {
        // ([user] or editor) and member → Intersection([Union([This, CU(editor)]), CU(member)])
        let schema = parse_schema(
            "type doc\n  relations\n    define editor: [user]\n    define member: [user]\n    define viewer: ([user] or editor) and member",
        )
        .unwrap();
        let r = schema.get_rewrite("doc", "viewer").unwrap();
        let Rewrite::Intersection(v) = r else { panic!("expected Intersection, got {r:?}") };
        assert_eq!(v.len(), 2);
        assert!(matches!(&v[0], Rewrite::Union(inner) if inner.len() == 2));
        assert!(matches!(&v[1], Rewrite::ComputedUserset { relation } if relation == "member"));
    }

    #[test]
    fn parse_full_demo_schema() {
        let schema_text = "\
type user

type group
  relations
    define member: [user, group#member]

type folder
  relations
    define owner: [user]
    define editor: [user] or owner
    define viewer: [user] or editor

type document
  relations
    define parent: [folder]
    define owner: [user]
    define editor: [user] or owner or editor from parent
    define viewer: [user] or editor or viewer from parent
";
        let schema = parse_schema(schema_text).unwrap();

        assert!(schema.has_type("user"));
        assert!(schema.has_type("group"));
        assert!(schema.has_type("folder"));
        assert!(schema.has_type("document"));

        // document#viewer is a flat Union with 3 members
        let viewer = schema.get_rewrite("document", "viewer").unwrap();
        let Rewrite::Union(v) = viewer else { panic!("expected Union") };
        assert_eq!(v.len(), 3, "document#viewer should be Union([This, CU(editor), TTU])");

        // document#editor is also Union with 3 members
        let editor = schema.get_rewrite("document", "editor").unwrap();
        let Rewrite::Union(ev) = editor else { panic!("expected Union") };
        assert_eq!(ev.len(), 3);

        // group#member is This (direct with two type restrictions)
        let member = schema.get_rewrite("group", "member").unwrap();
        let Rewrite::This { allowed } = member else { panic!("expected This") };
        assert_eq!(allowed.len(), 2);

        // unknown relation returns None
        assert!(schema.get_rewrite("document", "nonexistent").is_none());
    }

    // --- Step 4: semantic validation ---

    #[test]
    fn validate_undefined_computed_userset() {
        let result = parse_schema(
            "type doc\n  relations\n    define viewer: nonexistent",
        );
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert_eq!(errs.len(), 1);
        let SchemaError::UndefinedRelation { type_name, relation, referenced_from } = &errs[0]
        else { panic!("expected UndefinedRelation") };
        assert_eq!(type_name, "doc");
        assert_eq!(relation, "nonexistent");
        assert_eq!(referenced_from, "viewer");
    }

    #[test]
    fn validate_undefined_tupleset() {
        let result = parse_schema(
            "type doc\n  relations\n    define viewer: viewer from ghost",
        );
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs.iter().any(|e| matches!(e,
            SchemaError::UndefinedRelation { relation, .. } if relation == "ghost"
        )));
    }

    #[test]
    fn validate_undefined_ttu_computed() {
        let result = parse_schema(
            "type doc\n  relations\n    define parent: [folder]\n    define viewer: missing from parent",
        );
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs.iter().any(|e| matches!(e,
            SchemaError::UndefinedRelation { relation, .. } if relation == "missing"
        )));
    }

    #[test]
    fn validate_accumulates_multiple_errors() {
        let result = parse_schema(
            "type doc\n  relations\n    define a: ghost1\n    define b: ghost2",
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().len(), 2);
    }

    #[test]
    fn validate_undefined_inside_union() {
        let result = parse_schema(
            "type doc\n  relations\n    define viewer: [user] or ghost",
        );
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs.iter().any(|e| matches!(e,
            SchemaError::UndefinedRelation { relation, .. } if relation == "ghost"
        )));
    }

    #[test]
    fn validate_valid_schema_passes() {
        let result = parse_schema(
            "type doc\n  relations\n    define owner: [user]\n    define viewer: owner",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn validate_full_demo_schema_passes() {
        let schema_text = "\
type user

type group
  relations
    define member: [user, group#member]

type folder
  relations
    define owner: [user]
    define editor: [user] or owner
    define viewer: [user] or editor

type document
  relations
    define parent: [folder]
    define owner: [user]
    define editor: [user] or owner or editor from parent
    define viewer: [user] or editor or viewer from parent
";
        assert!(parse_schema(schema_text).is_ok());
    }
}
