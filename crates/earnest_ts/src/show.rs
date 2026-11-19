use earnest_ast::{Binary, Reach, Shape, ShapeKind, TemplatePiece, Term, TermKind, Unary};

fn listed(terms: &[Term]) -> String {
    let shown: Vec<String> = terms.iter().map(show).collect();
    shown.join(", ")
}

fn sign(binary: Binary) -> &'static str {
    match binary {
        Binary::And => "&&",
        Binary::Or => "||",
        Binary::Equal => "===",
        Binary::Differ => "!==",
        Binary::Less => "<",
        Binary::LessOrEqual => "<=",
        Binary::Greater => ">",
        Binary::GreaterOrEqual => ">=",
        Binary::Add => "+",
        Binary::Subtract => "-",
        Binary::Multiply => "*",
        Binary::Remainder => "%",
        _ => "?",
    }
}

fn record(base: Option<&Term>, fields: &[(String, Term)]) -> String {
    let mut parts: Vec<String> = base
        .iter()
        .map(|spread| format!("...{}", show(spread)))
        .collect();
    parts.extend(
        fields
            .iter()
            .map(|(name, value)| format!("{name}: {}", show(value))),
    );
    format!("{{{}}}", parts.join(", "))
}

fn template(pieces: &[TemplatePiece]) -> String {
    let mut out = String::from("`");
    for piece in pieces {
        match piece {
            TemplatePiece::Text(text) => out.push_str(text),
            TemplatePiece::Value(term) => {
                out.push_str("${");
                out.push_str(&show(term));
                out.push('}');
            }
            _ => out.push('?'),
        }
    }
    out.push('`');
    out
}

pub(crate) fn show(term: &Term) -> String {
    match &term.kind {
        TermKind::Null => "null".to_owned(),
        TermKind::Bool(flag) => flag.to_string(),
        TermKind::Int(number) => number.to_string(),
        TermKind::Text(text) => format!("\"{text}\""),
        TermKind::Name(name) => name.clone(),
        TermKind::Member(target, name) => format!("{}.{name}", show(target)),
        TermKind::Call {
            target,
            method,
            arguments,
        } => format!("{}.{method}({})", show(target), listed(arguments)),
        TermKind::Helper {
            name, arguments, ..
        } => format!("{name}({})", listed(arguments)),
        TermKind::Unary(Unary::Not, inner) => format!("(! {})", show(inner)),
        TermKind::Unary(_, inner) => format!("(- {})", show(inner)),
        TermKind::Binary(binary, left, right) => {
            format!("({} {} {})", sign(*binary), show(left), show(right))
        }
        TermKind::Choose(test, yes, no) => {
            format!("(? {} {} {})", show(test), show(yes), show(no))
        }
        TermKind::Record { base, fields } => record(base.as_deref(), fields),
        TermKind::SetOf(items) => format!("set({})", listed(items)),
        TermKind::SeqOf(items) => format!("seq({})", listed(items)),
        TermKind::MapOf(entries) => {
            let shown: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{} -> {}", show(key), show(value)))
                .collect();
            format!("map({})", shown.join(", "))
        }
        TermKind::Lambda { parameters, body } => {
            format!("({}) => {}", parameters.join(", "), show(body))
        }
        TermKind::Bind { name, value, body } => {
            format!("(let {name} = {} in {})", show(value), show(body))
        }
        TermKind::Wrap(inner) => format!("some({})", show(inner)),
        TermKind::Across {
            reach,
            shape,
            parameter,
            body,
        } => {
            let word = match reach {
                Reach::Some => "exists",
                _ => "forall",
            };
            format!(
                "({word} {parameter} in {}: {})",
                show_shape(shape),
                show(body)
            )
        }
        TermKind::Template(pieces) => template(pieces),
        _ => "?".to_owned(),
    }
}

fn show_kind(kind: &ShapeKind) -> String {
    match kind {
        ShapeKind::Bool => "bool".to_owned(),
        ShapeKind::Int { lo, hi, .. } => format!("int({lo}, {hi})"),
        ShapeKind::OneOf(choices) => format!("enum({})", listed(choices)),
        ShapeKind::Optional(inner) => format!("optional({})", show_shape(inner)),
        ShapeKind::Record(fields) => {
            let shown: Vec<String> = fields
                .iter()
                .map(|(name, shape)| format!("{name}: {}", show_shape(shape)))
                .collect();
            format!("record({})", shown.join(", "))
        }
        ShapeKind::Set(item) => format!("set({})", show_shape(item)),
        ShapeKind::Map(key, value) => format!("map({}, {})", show_shape(key), show_shape(value)),
        ShapeKind::Seq { item, max } => format!("seq({}, {max})", show_shape(item)),
        ShapeKind::Queue { item, capacity } => format!("queue({}, {capacity})", show_shape(item)),
        ShapeKind::Lock(None) => "lock".to_owned(),
        ShapeKind::Lock(Some(owner)) => format!("lock({})", show_shape(owner)),
        ShapeKind::Timer => "timer".to_owned(),
        ShapeKind::Request => "request".to_owned(),
        ShapeKind::Retry { max } => format!("retry({max})"),
        _ => "?".to_owned(),
    }
}

pub(crate) fn show_shape(shape: &Shape) -> String {
    match &shape.initial {
        Some(start) => format!("{} = {}", show_kind(&shape.kind), show(start)),
        None => show_kind(&shape.kind),
    }
}
