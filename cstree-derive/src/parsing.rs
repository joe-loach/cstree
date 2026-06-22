mod attributes;

use syn::{Token, parse::Parse, punctuated::Punctuated};

use crate::{errors::ErrorContext, symbols::*};

use self::attributes::Attr;

/// Convenience for recording errors inside `ErrorContext` instead of the `Err` variant of the `Result`.
pub(crate) type Result<T, E = ()> = core::result::Result<T, E>;

pub(crate) struct SyntaxKindEnum<'i> {
    pub(crate) name: syn::Ident,
    pub(crate) data: Option<syn::Type>,
    pub(crate) repr: Option<syn::Ident>,
    pub(crate) variants: Vec<SyntaxKindVariant<'i>>,
    pub(crate) source: &'i syn::DeriveInput,
}

impl<'i> SyntaxKindEnum<'i> {
    pub(crate) fn parse_from_ast(error_handler: &ErrorContext, item: &'i syn::DeriveInput) -> Result<Self> {
        let syn::Data::Enum(enum_data) = &item.data else {
            error_handler.error_at(item, "`Syntax` can only be derived on enums");
            return Err(());
        };

        let name = item.ident.clone();

        let mut repr = Attr::none(error_handler, REPR);
        for repr_attr in item.attrs.iter().filter(|&attr| attr.path().is_ident(&REPR)) {
            if let syn::Meta::List(nested) = &repr_attr.meta {
                if let Ok(nested) = nested.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated) {
                    for meta in nested {
                        if let syn::Meta::Path(path) = meta {
                            if let Some(ident) = path.get_ident() {
                                repr.set(repr_attr, ident.clone());
                            }
                        }
                    }
                }
            }
        }

        let mut data = Attr::none(error_handler, DATA);
        for attr in item.attrs.iter().filter(|&attr| attr.path() == SYNTAX) {
            match attr.parse_args::<SyntaxAttr>() {
                Ok(SyntaxAttr { data: ty }) => data.set(attr, ty),
                Err(e) => {
                    error_handler.error_at(attr, "expected `#[syntax(data = Type)]`");
                    error_handler.syn_error(e);
                }
            }
        }

        let variants = enum_data
            .variants
            .iter()
            .map(|variant| SyntaxKindVariant::parse_from_ast(error_handler, variant))
            .collect();

        Ok(Self {
            name,
            data: data.get(),
            repr: repr.get(),
            variants,
            source: item,
        })
    }
}

struct SyntaxAttr {
    data: syn::Type,
}

impl Parse for SyntaxAttr {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        if name != DATA {
            return Err(syn::Error::new_spanned(name, "expected `data`"));
        }
        input.parse::<Token![=]>()?;
        Ok(Self { data: input.parse()? })
    }
}

pub(crate) struct SyntaxKindVariant<'i> {
    pub(crate) name: syn::Ident,
    pub(crate) static_text: Option<String>,
    pub(crate) static_data: Option<syn::Expr>,
    pub(crate) source: &'i syn::Variant,
}

impl<'i> SyntaxKindVariant<'i> {
    pub(crate) fn parse_from_ast(error_handler: &ErrorContext, variant: &'i syn::Variant) -> Self {
        let name = variant.ident.clone();

        // Check that `variant` is a unit variant
        match &variant.fields {
            syn::Fields::Unit => (),
            syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
                error_handler.error_at(variant, "syntax kinds with fields are not supported");
            }
        }

        // Check that discriminants are unaltered
        if variant.discriminant.is_some() {
            error_handler.error_at(
                variant,
                "syntax kinds are not allowed to have custom discriminant values",
            );
        }

        let mut static_text = Attr::none(error_handler, STATIC_TEXT);
        for text in variant
            .attrs
            .iter()
            .flat_map(|attr| get_static_text(error_handler, attr))
        {
            static_text.set(&text, text.value());
        }
        let mut static_data = Attr::none(error_handler, STATIC_DATA);
        for data in variant
            .attrs
            .iter()
            .flat_map(|attr| get_static_data(error_handler, attr))
        {
            static_data.set(&data, data.clone());
        }
        Self {
            name,
            static_text: static_text.get(),
            static_data: static_data.get(),
            source: variant,
        }
    }
}

fn get_static_text(error_handler: &ErrorContext, attr: &syn::Attribute) -> Option<syn::LitStr> {
    use syn::Meta::*;

    if attr.path() != STATIC_TEXT {
        return None;
    }

    match &attr.meta {
        List(list) => match list.parse_args() {
            Ok(lit) => Some(lit),
            Err(e) => {
                error_handler.error_at(
                    list,
                    "argument to `static_text` must be a string literal: `#[static_text(\"...\")]`",
                );
                error_handler.syn_error(e);
                None
            }
        },
        Path(_) => {
            error_handler.error_at(attr, "missing text for `static_text`: try `#[static_text(\"...\")]`");
            None
        }
        NameValue(_) => {
            error_handler.error_at(
                attr,
                "`static_text` takes the text as a function argument: `#[static_text(\"...\")]`",
            );
            None
        }
    }
}

fn get_static_data(error_handler: &ErrorContext, attr: &syn::Attribute) -> Option<syn::Expr> {
    use syn::Meta::*;

    if attr.path() != STATIC_DATA {
        return None;
    }

    match &attr.meta {
        List(list) => match list.parse_args() {
            Ok(expr) => Some(expr),
            Err(e) => {
                error_handler.error_at(
                    list,
                    "argument to `static_data` must be an expression: `#[static_data(EXPR)]`",
                );
                error_handler.syn_error(e);
                None
            }
        },
        Path(_) => {
            error_handler.error_at(attr, "missing data for `static_data`: try `#[static_data(EXPR)]`");
            None
        }
        NameValue(_) => {
            error_handler.error_at(
                attr,
                "`static_data` takes data as a function argument: `#[static_data(EXPR)]`",
            );
            None
        }
    }
}
