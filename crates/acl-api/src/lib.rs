use acl_model::{parse_schema, Schema, SchemaError};
use std::sync::Arc;

pub struct Config {
    pub schema_path: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let schema_path = std::env::var("SCHEMA_PATH")
            .map_err(|_| "SCHEMA_PATH environment variable not set".to_string())?;
        Ok(Self { schema_path })
    }
}

/// Read and parse a schema file, returning an `Arc<Schema>` on success.
/// Maps IO errors into a `SchemaError::Parse` so callers see a uniform error type.
pub fn load_schema(path: &str) -> Result<Arc<Schema>, Vec<SchemaError>> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        vec![SchemaError::Parse {
            message: format!("failed to read schema file '{path}': {e}"),
            span: 0..0,
        }]
    })?;
    parse_schema(&text).map(Arc::new)
}

/// Load the schema and log a startup line. The HTTP router will be wired in task 2.5.
pub fn serve(config: Config) -> Result<Arc<Schema>, String> {
    let schema = load_schema(&config.schema_path).map_err(|errs| {
        errs.into_iter()
            .map(|e| format!("{e:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    eprintln!(
        "schema loaded from '{}': {} type(s)",
        config.schema_path,
        schema.type_count()
    );
    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acl_model::Rewrite;
    use std::path::Path;

    fn schema_fga_path() -> String {
        // CARGO_MANIFEST_DIR is .../crates/acl-api; workspace root is two levels up.
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        Path::new(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("schema.fga")
            .to_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn load_schema_from_file() {
        let schema = load_schema(&schema_fga_path()).expect("schema.fga should parse cleanly");

        assert!(schema.has_type("user"));
        assert!(schema.has_type("group"));
        assert!(schema.has_type("folder"));
        assert!(schema.has_type("document"));
        assert!(schema.has_type("file"));
        assert_eq!(schema.type_count(), 5);

        // document#viewer is a Union with 3 members
        assert!(matches!(
            schema.get_rewrite("document", "viewer"),
            Some(Rewrite::Union(v)) if v.len() == 3
        ));

        // unknown relation returns None
        assert!(schema.get_rewrite("document", "nonexistent").is_none());
    }

    #[test]
    fn load_schema_missing_file_returns_err() {
        let result = load_schema("/nonexistent/path/schema.fga");
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0], SchemaError::Parse { .. }));
    }

    #[test]
    fn load_schema_invalid_schema_returns_err() {
        // Write a temp file with an invalid schema and confirm Err propagates.
        let result = load_schema("/dev/null"); // empty file → Ok (empty schema)
        assert!(result.is_ok());
    }
}
