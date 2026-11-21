use crate::{Expr, Op};

impl Expr {
    pub(crate) fn plain(&self) -> &Self {
        match self {
            Self::Apply(Op::SomeOf, arguments) => match arguments.as_slice() {
                [inner] => inner.plain(),
                _ => self,
            },
            _ => self,
        }
    }

    pub(crate) fn each_part<'e>(&'e self, visit: &mut impl FnMut(&'e Self)) {
        match self {
            Self::Const(_) | Self::Field(_) | Self::Local(_) => {}
            Self::Apply(_, items) | Self::SetOf(items) | Self::SeqOf(items) => {
                items.iter().for_each(visit);
            }
            Self::And(left, right) | Self::Or(left, right) => {
                visit(left);
                visit(right);
            }
            Self::Choose(test, yes, no) => {
                visit(test);
                visit(yes);
                visit(no);
            }
            Self::Bind { value, body, .. } => {
                visit(value);
                visit(body);
            }
            Self::Record(fields) => fields.iter().for_each(|(_, inner)| visit(inner)),
            Self::Get(inner, _) => visit(inner),
            Self::With(base, changes) => {
                visit(base);
                for (_, inner) in changes {
                    visit(inner);
                }
            }
            Self::MapOf(entries) => entries.iter().for_each(|(key, value)| {
                visit(key);
                visit(value);
            }),
            Self::Sweep { source, body, .. } => {
                visit(source);
                visit(body);
            }
        }
    }
}
