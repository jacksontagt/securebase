use super::{NamespaceRef, NamespaceRefKind, Rewrite, SchemaError};
use chumsky::prelude::*;

pub(super) struct RawNamespaceDef {
    pub name: String,
    pub relations: Vec<(String, Rewrite)>,
}

type Err<'a> = extra::Err<Rich<'a, char>>;

pub(super) fn parse(input: &str) -> (Option<Vec<RawNamespaceDef>>, Vec<SchemaError>) {
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

#[derive(Clone, Copy)]
enum BinOp {
    Or,
    And,
    ButNot,
}

// Flatten same-operator chains: A or B or C => Union([A, B, C]), not Union([Union([A, B]), C]).
fn merge(lhs: Rewrite, op: BinOp, rhs: Rewrite) -> Rewrite {
    match op {
        BinOp::Or => match lhs {
            Rewrite::Union(mut v) => {
                v.push(rhs);
                Rewrite::Union(v)
            }
            other => Rewrite::Union(vec![other, rhs]),
        },
        BinOp::And => match lhs {
            Rewrite::Intersection(mut v) => {
                v.push(rhs);
                Rewrite::Intersection(v)
            }
            other => Rewrite::Intersection(vec![other, rhs]),
        },
        BinOp::ButNot => Rewrite::Exclusion(Box::new(lhs), Box::new(rhs)),
    }
}

// Matches an identifier that equals `word`; backtracks via try_map if it doesn't
fn match_word<'a>(word: &'static str) -> impl Parser<'a, &'a str, (), Err<'a>> + Clone {
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

// Parses one entry inside a bracket list: "user", "user:*", or "group#member"
fn namespace_ref_parser<'a>() -> impl Parser<'a, &'a str, NamespaceRef, Err<'a>> + Clone {
    let base = text::ascii::ident().map(|s: &str| s.to_string());

    let suffix = choice((
        just('#')
            .ignore_then(text::ascii::ident().map(|s: &str| s.to_string()))
            .map(NamespaceRefKind::Userset),
        just(':')
            .ignore_then(just('*'))
            .to(NamespaceRefKind::Wildcard),
    ))
    .or_not()
    .map(|opt| opt.unwrap_or(NamespaceRefKind::Direct));

    base.then(suffix)
        .map(|(namespace, subject)| NamespaceRef { namespace, subject })
        .padded()
}

// Parses "[user]", "[user, group#member]"
fn namespace_restriction_parser<'a>() -> impl Parser<'a, &'a str, Rewrite, Err<'a>> + Clone {
    just('[')
        .padded()
        .ignore_then(
            namespace_ref_parser()
                .separated_by(just(',').padded())
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(']').padded())
        .map(|allowed| Rewrite::This { allowed })
}

fn ident_rewrite_parser<'a>() -> impl Parser<'a, &'a str, Rewrite, Err<'a>> + Clone {
    let reserved = |s: &str| {
        matches!(
            s,
            "or" | "and" | "but" | "not" | "from" | "namespace" | "relations" | "define"
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
        .then(match_word("from").ignore_then(ident_str()).or_not())
        .map(|(name, from_part)| match from_part {
            Some(tupleset) => Rewrite::TupleToUserset {
                tupleset,
                computed: name,
            },
            None => Rewrite::ComputedUserset { relation: name },
        })
}

// handles or/and/but-not chains and parentheses
fn rewrite_expr_parser<'a>() -> impl Parser<'a, &'a str, Rewrite, Err<'a>> {
    recursive(|expr| {
        let atom = choice((
            namespace_restriction_parser(),
            expr.clone()
                .delimited_by(just('(').padded(), just(')').padded()),
            ident_rewrite_parser(),
        ));

        let op = choice((
            match_word("or").to(BinOp::Or),
            match_word("and").to(BinOp::And),
            match_word("but")
                .then_ignore(match_word("not"))
                .to(BinOp::ButNot),
        ));

        // Collect (operator, rhs) pairs and fold left so that A or B or C
        // becomes Union([A, B, C]) rather than Union([Union([A, B]), C]).
        atom.clone()
            .then(op.then(atom).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                rest.into_iter()
                    .fold(first, |lhs, (op, rhs)| merge(lhs, op, rhs))
            })
    })
}

fn relation_def_parser<'a>() -> impl Parser<'a, &'a str, (String, Rewrite), Err<'a>> {
    match_word("define")
        .ignore_then(ident_str())
        .then_ignore(just(':').padded())
        .then(rewrite_expr_parser())
}

fn namespace_body_parser<'a>() -> impl Parser<'a, &'a str, Vec<(String, Rewrite)>, Err<'a>> {
    match_word("relations").ignore_then(
        relation_def_parser()
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>(),
    )
}

fn namespace_def_parser<'a>() -> impl Parser<'a, &'a str, RawNamespaceDef, Err<'a>> {
    match_word("namespace")
        .ignore_then(ident_str())
        .then(namespace_body_parser().or_not())
        .map(|(name, body)| RawNamespaceDef {
            name,
            relations: body.unwrap_or_default(),
        })
}

fn schema_parser<'a>() -> impl Parser<'a, &'a str, Vec<RawNamespaceDef>, Err<'a>> {
    namespace_def_parser()
        .repeated()
        .collect::<Vec<_>>()
        .padded()
        .then_ignore(end())
}
