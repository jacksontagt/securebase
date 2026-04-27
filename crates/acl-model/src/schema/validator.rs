use super::{Rewrite, SchemaError, TypeDef};
use std::collections::{HashMap, HashSet};

// Validate that every relation name referenced in a rewrite is defined
// within the same type block. Returns all errors accumulated.
pub(super) fn validate(types: &HashMap<String, TypeDef>) -> Vec<SchemaError> {
    let mut errors = Vec::new();
    for (type_name, type_def) in types {
        let defined: HashSet<&str> = type_def.relations.keys().map(String::as_str).collect();
        for (relation_name, rewrite) in &type_def.relations {
            check_rewrite(type_name, relation_name, rewrite, &defined, &mut errors);
        }
    }
    errors
}

fn check_rewrite(
    type_name: &str,
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
                    type_name: type_name.to_string(),
                    relation: relation.clone(),
                    referenced_from: from_relation.to_string(),
                });
            }
        }

        Rewrite::TupleToUserset { tupleset, computed } => {
            for name in [tupleset, computed] {
                if !defined.contains(name.as_str()) {
                    errors.push(SchemaError::UndefinedRelation {
                        type_name: type_name.to_string(),
                        relation: name.clone(),
                        referenced_from: from_relation.to_string(),
                    });
                }
            }
        }

        Rewrite::Union(v) | Rewrite::Intersection(v) => {
            for r in v {
                check_rewrite(type_name, from_relation, r, defined, errors);
            }
        }

        Rewrite::Exclusion(a, b) => {
            check_rewrite(type_name, from_relation, a, defined, errors);
            check_rewrite(type_name, from_relation, b, defined, errors);
        }
    }
}
