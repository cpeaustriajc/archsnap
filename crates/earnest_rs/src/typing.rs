use crate::reader::{Reader, name_of, unwrapped};
use syn::punctuated::Punctuated;
use syn::{Expr, ExprCall, ExprField, ExprMethodCall, Member, Type, TypeReference};

pub(crate) struct Local {
    pub(crate) name: String,
    pub(crate) ty: Option<Type>,
}

pub(crate) fn plain(name: String, ty: Option<Type>) -> Local {
    Local { name, ty }
}

pub(crate) fn strip(ty: &Type) -> &Type {
    match ty {
        Type::Reference(borrowed) => strip(&borrowed.elem),
        Type::Paren(inner) => strip(&inner.elem),
        Type::Group(inner) => strip(&inner.elem),
        other => other,
    }
}

pub(crate) fn is_reference(ty: &Type) -> bool {
    match ty {
        Type::Reference(_) => true,
        Type::Paren(inner) => is_reference(&inner.elem),
        Type::Group(inner) => is_reference(&inner.elem),
        _ => false,
    }
}

pub(crate) fn path_name(ty: &Type) -> Option<String> {
    match strip(ty) {
        Type::Path(named) if named.qself.is_none() => named.path.get_ident().map(name_of),
        _ => None,
    }
}

pub(crate) fn inside_option(ty: &Type) -> Option<&Type> {
    let Type::Path(named) = strip(ty) else {
        return None;
    };
    let last = named.path.segments.last()?;
    if name_of(&last.ident) != "Option" {
        return None;
    }
    match &last.arguments {
        syn::PathArguments::AngleBracketed(listed) => match listed.args.first()? {
            syn::GenericArgument::Type(inner) => Some(inner),
            _ => None,
        },
        _ => None,
    }
}

fn borrowed(ty: Type) -> Type {
    Type::Reference(TypeReference {
        and_token: syn::token::And::default(),
        lifetime: None,
        mutability: None,
        elem: Box::new(ty),
    })
}

fn infer() -> Type {
    Type::Infer(syn::TypeInfer {
        underscore_token: syn::token::Underscore::default(),
    })
}

impl Reader<'_> {
    pub(crate) fn local_type(&self, name: &str) -> Option<&Type> {
        self.bound
            .iter()
            .rev()
            .find(|local| local.name == name)
            .and_then(|local| local.ty.as_ref())
    }

    fn model_of(&self, name: &str) -> Option<String> {
        if name == "Self" {
            self.model_name.clone()
        } else {
            Some(name.to_owned())
        }
    }

    pub(crate) fn static_type(&self, expr: &Expr) -> Option<Type> {
        match unwrapped(expr) {
            Expr::Reference(reference) if reference.mutability.is_none() => Some(borrowed(
                self.static_type(&reference.expr).unwrap_or_else(infer),
            )),
            Expr::Path(name) if name.qself.is_none() => self.path_type(&name.path),
            Expr::Field(read) => self.field_type(read),
            Expr::MethodCall(call) => self.method_type(call),
            Expr::Call(call) => self.call_type(call),
            _ => None,
        }
    }

    fn path_type(&self, path: &syn::Path) -> Option<Type> {
        let names: Vec<String> = path
            .segments
            .iter()
            .map(|segment| name_of(&segment.ident))
            .collect();
        match names.as_slice() {
            [single] if single == "self" => {
                self.model_name.as_ref().map(|model| borrowed(named(model)))
            }
            [single] => self.local_type(single).cloned(),
            [owner, variant]
                if self
                    .enums
                    .get(owner)
                    .is_some_and(|variants| variants.contains(variant)) =>
            {
                Some(named(owner))
            }
            _ => None,
        }
    }

    fn field_type(&self, read: &ExprField) -> Option<Type> {
        let Member::Named(field) = &read.member else {
            return None;
        };
        let owner = self.static_type(&read.base)?;
        let name = self.model_of(&path_name(&owner)?)?;
        let fields = self.structs.get(&name)?;
        let wanted = name_of(field);
        fields
            .iter()
            .find(|(known, _)| *known == wanted)
            .map(|(_, ty)| ty.clone())
    }

    fn method_type(&self, call: &ExprMethodCall) -> Option<Type> {
        let name = name_of(&call.method);
        if name == "clone" && call.args.is_empty() {
            let receiver = self.static_type(&call.receiver)?;
            return Some(strip(&receiver).clone());
        }
        let on_state = matches!(unwrapped(&call.receiver),
            Expr::Path(path) if path.path.is_ident("self"));
        if on_state {
            return self.method_types.get(&name).cloned();
        }
        None
    }

    fn call_type(&self, call: &ExprCall) -> Option<Type> {
        let Expr::Path(function) = unwrapped(&call.func) else {
            return None;
        };
        let name = function.path.get_ident().map(name_of)?;
        self.fn_types.get(&name).cloned()
    }
}

fn named(name: &str) -> Type {
    Type::Path(syn::TypePath {
        qself: None,
        path: syn::Path {
            leading_colon: None,
            segments: {
                let mut segments = Punctuated::new();
                segments.push(syn::PathSegment::from(syn::Ident::new(
                    name,
                    proc_macro2::Span::call_site(),
                )));
                segments
            },
        },
    })
}
