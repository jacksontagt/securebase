use chumsky::prelude::*;
use super::{Rewrite, SchemaError, TypeRef, TypeRefKind};

pub(super) struct RawTypeDef {
    pub name: String,
    pub relations: Vec<(String, Rewrite)>,
}

type Err<'a> = extra::Err<Rich<'a, char>>;

pub(super) fn parse(input: &str) -> (Option<Vec<RawTypeDef>>, Vec<SchemaError>) {
    let (output, errors) = schema_parser().parse(input).into_output_errors();
    let schema_errors = errors
        .into_iter()
        .map(|e| {
            let span = e.span();
            SchemaError::Parse {
                message: e.to_string(),
                span: span.start..span.end,
            }
        })
        .collect();
    (output, schema_errors)
}

// Matches an identifier that equals `word`, fails (with backtrack) otherwise.
fn kw<'a>(word: &'static str) -> impl Parser<'a, &'a str, (), Err<'a>> + Clone {
    text::ascii::ident()
        .try_map(move |s: &str, span| {
            if s == word {
                Ok(())
            } else {
                Err(Rich::custom(span, format!("expected '{word}'")))
            }
        })
        .padded()
}

fn ident_str<'a>() -> impl Parser<'a, &'a str, String, Err<'a>> + Clone {
    text::ascii::ident().map(|s: &str| s.to_string()).padded()
}

// Parses one entry inside a bracket list: `user`, `user:*`, or `group#member`.
fn type_ref_parser<'a>() -> impl Parser<'a, &'a str, TypeRef, Err<'a>> {
    let base = text::ascii::ident().map(|s: &str| s.to_string());

    let suffix = choice((
        just('#')
            .ignore_then(text::ascii::ident().map(|s: &str| s.to_string()))
            .map(TypeRefKind::Userset),
        just(':').ignore_then(just('*')).to(TypeRefKind::Wildcard),
    ))
    .or_not()
    .map(|opt| opt.unwrap_or(TypeRefKind::Direct));

    base.then(suffix)
        .map(|(type_name, subject)| TypeRef { type_name, subject })
        .padded()
}

// Parses `[user]`, `[user, group#member]`, etc. → Rewrite::This.
fn type_restriction_parser<'a>() -> impl Parser<'a, &'a str, Rewrite, Err<'a>> {
    just('[')
        .padded()
        .ignore_then(
            type_ref_parser()
                .separated_by(just(',').padded())
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(']').padded())
        .map(|allowed| Rewrite::This { allowed })
}

// Parses a bare identifier or `ident from ident`.
// Handles both ComputedUserset and TupleToUserset in one pass to avoid
// backtracking after consuming the leading ident.
fn ident_rewrite_parser<'a>() -> impl Parser<'a, &'a str, Rewrite, Err<'a>> {
    let reserved = |s: &str| {
        matches!(
            s,
            "or" | "and" | "but" | "not" | "from" | "type" | "relations" | "define"
        )
    };

    text::ascii::ident()
        .try_map(move |s: &str, span| {
            if reserved(s) {
                Err(Rich::custom(span, format!("'{s}' is a reserved keyword")))
            } else {
                Ok(s.to_string())
            }
        })
        .padded()
        .then(kw("from").ignore_then(ident_str()).or_not())
        .map(|(name, from_part)| match from_part {
            Some(tupleset) => Rewrite::TupleToUserset { tupleset, computed: name },
            None => Rewrite::ComputedUserset { relation: name },
        })
}

// Step 2: atoms only. Extended with binary operators in Step 3.
fn rewrite_term_parser<'a>() -> impl Parser<'a, &'a str, Rewrite, Err<'a>> {
    choice((type_restriction_parser(), ident_rewrite_parser()))
}

fn relation_def_parser<'a>() -> impl Parser<'a, &'a str, (String, Rewrite), Err<'a>> {
    kw("define")
        .ignore_then(ident_str())
        .then_ignore(just(':').padded())
        .then(rewrite_term_parser())
}

fn type_body_parser<'a>() -> impl Parser<'a, &'a str, Vec<(String, Rewrite)>, Err<'a>> {
    kw("relations").ignore_then(
        relation_def_parser()
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>(),
    )
}

fn type_def_parser<'a>() -> impl Parser<'a, &'a str, RawTypeDef, Err<'a>> {
    kw("type")
        .ignore_then(ident_str())
        .then(type_body_parser().or_not())
        .map(|(name, body)| RawTypeDef {
            name,
            relations: body.unwrap_or_default(),
        })
}

fn schema_parser<'a>() -> impl Parser<'a, &'a str, Vec<RawTypeDef>, Err<'a>> {
    type_def_parser()
        .repeated()
        .collect::<Vec<_>>()
        .padded()
        .then_ignore(end())
}
