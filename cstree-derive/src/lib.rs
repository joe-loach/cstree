//! This crate provides `cstree`'s derive macro for `Syntax`.
//!
//! ```
//! # use cstree_derive::Syntax;
//! #
//! # #[derive(Debug, Copy, Clone, PartialEq, Eq)]
//! #[derive(Syntax)]
//! # #[repr(u32)]
//! # enum SyntaxKind { Root }
//! ```
//!
//! Please refer to [the `cstree` main crate] for how to set this up.
//!
//! [the `cstree` main crate]: https://docs.rs/cstree/

use errors::ErrorContext;
use parsing::SyntaxKindEnum;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use std::vec::Vec;
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

mod errors;
mod parsing;
mod symbols;

use symbols::*;

#[proc_macro_derive(Syntax, attributes(static_text, static_data))]
pub fn language(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    expand_syntax(ast).unwrap_or_else(to_compile_errors).into()
}

fn expand_syntax(ast: DeriveInput) -> Result<TokenStream, Vec<syn::Error>> {
    let error_handler = ErrorContext::new();
    let Ok(syntax_kind_enum) = SyntaxKindEnum::parse_from_ast(&error_handler, &ast) else {
        return Err(error_handler.check().unwrap_err());
    };

    // Check that the `enum` is `#[repr(u32)]`
    match &syntax_kind_enum.repr {
        Some(repr) if repr == U32 => (),
        Some(_) | None => error_handler.error_at(
            syntax_kind_enum.source,
            "syntax kind definitions must be `#[repr(u32)]` to derive `Syntax`",
        ),
    }

    error_handler.check()?;

    let name = &syntax_kind_enum.name;
    let variant_count = syntax_kind_enum.variants.len() as u32;
    let static_data = syntax_kind_enum.variants.iter().map(|variant| {
        let variant_name = &variant.name;
        let static_data = match (&variant.static_data, &variant.static_text) {
            (Some(data), _) => quote!(::core::option::Option::Some(#data)),
            (None, Some(text)) => {
                let bytes = syn::LitByteStr::new(text.as_bytes(), variant.source.span());
                quote!(::core::option::Option::Some(#bytes))
            }
            (None, None) => quote!(::core::option::Option::None),
        };
        quote_spanned!(variant.source.span()=>
            #name :: #variant_name => #static_data,
        )
    });
    let trait_impl = quote_spanned! { syntax_kind_enum.source.span()=>
        #[automatically_derived]
        impl ::cstree::Syntax for #name {
            type Data = [u8];

            fn from_raw(raw: ::cstree::RawSyntaxKind) -> Self {
                assert!(raw.0 < #variant_count, "Invalid raw syntax kind: {}", raw.0);
                // Safety: discriminant is valid by the assert above
                unsafe { ::std::mem::transmute::<u32, #name>(raw.0) }
            }

            fn into_raw(self) -> ::cstree::RawSyntaxKind {
                ::cstree::RawSyntaxKind(self as u32)
            }

            fn data_to_bytes(data: &Self::Data) -> &[u8] {
                data
            }

            fn data_from_bytes(data: &[u8]) -> ::core::option::Option<&Self::Data> {
                ::core::option::Option::Some(data)
            }

            fn static_data(self) -> ::core::option::Option<&'static Self::Data> {
                match self {
                    #( #static_data )*
                }
            }
        }
    };
    Ok(trait_impl)
}

fn to_compile_errors(errors: Vec<syn::Error>) -> proc_macro2::TokenStream {
    let compile_errors = errors.iter().map(syn::Error::to_compile_error);
    quote!(#(#compile_errors)*)
}
