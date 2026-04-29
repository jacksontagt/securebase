use super::{NamespaceDef, Rewrite, SchemaError};
use std::collections::{HashMap, HashSet};

// Validate that every relation name referenced in a rewrite is defined
// within the same namespace block. Returns all errors accumulated.
pub(super) fn validate(namespaces: &HashMap<String, NamespaceDef>) -> Vec<SchemaError> {
    let mut errors = Vec::new();
    for (namespace, namespace_def) in namespaces {
        let defined: HashSet<&str> = namespace_def.relations.keys().map(String::as_str).collect();
        for (relation_name, rewrite) in &namespace_def.relations {
            check_rewrite(namespace, relation_name, rewrite, &defined, &mut errors);
        }
    }
    errors
}

fn check_rewrite(
    namespace: &str,
    from_relation: &str,
    rewrite: &Rewrite,
    defined: &HashSet<&str>,
    errors: &mut Vec<SchemaError>,
) {
    match rewrite {
        Rewrite::This { .. } => {}

        Rewrite::ComputedUserset { relation } => {
            if !defined.contains(relation.as_str()) {
                errors.push(SchemaError::UndefinedRelation {
                    namespace: namespace.to_string(),
                    relation: relation.clone(),
                    referenced_from: from_relation.to_string(),
                });
            }
        }

        Rewrite::TupleToUserset { tupleset, computed } => {
            for name in [tupleset, computed] {
                if !defined.contains(name.as_str()) {
                    errors.push(SchemaError::UndefinedRelation {
                        namespace: namespace.to_string(),
                        relation: name.clone(),
                        referenced_from: from_relation.to_string(),
                    });
                }
            }
        }

        Rewrite::Union(v) | Rewrite::Intersection(v) => {
            for r in v {
                check_rewrite(namespace, from_relation, r, defined, errors);
            }
        }

        Rewrite::Exclusion(a, b) => {
            check_rewrite(namespace, from_relation, a, defined, errors);
            check_rewrite(namespace, from_relation, b, defined, errors);
        }
    }
}
