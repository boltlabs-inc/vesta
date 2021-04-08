use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use std::iter::FromIterator;
use syn::{
    parse_macro_input, parse_quote, punctuated::Punctuated, spanned::Spanned, Arm, Data, DataEnum,
    DataStruct, DeriveInput, Error, Field, Fields, FieldsNamed, FieldsUnnamed, Generics, Ident,
    Item, Path, Token, Type, Variant,
};

use vesta_syntax::{vesta_path, CaseInput};

#[proc_macro]
pub fn case(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as CaseInput);
    match input.compile() {
        Ok(output) => output.to_token_stream().into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Derive `Match`, `Case`, and `Exhaustive` for a "foreign" struct or enum, given its declaration.
///
/// This is only useful within the `vesta` crate itself, because otherwise it will generate an
/// orphan impl. We use this as shorthand to declare a large set of instances to cover most of the
/// standard library.
#[proc_macro]
pub fn derive_match(input: TokenStream) -> TokenStream {
    derive_match_impl(input)
}

#[proc_macro_derive(Match)]
pub fn derive_match_derive(input: TokenStream) -> TokenStream {
    derive_match_impl(input)
}

/// Derive `Match`, `Case`, and `Exhaustive` for a struct or enum, given its declaration.
fn derive_match_impl(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        generics,
        data,
        attrs,
        ..
    } = parse_macro_input!(input as DeriveInput);
    // Determine if the enum is exhaustive
    let mut exhaustive = true;
    for attr in attrs {
        if let Some(ident) = attr.path.get_ident() {
            if ident == "non_exhaustive" {
                exhaustive = false;
            }
        }
    }

    match data {
        Data::Struct(s) => derive_match_struct(ident, generics, s),
        Data::Enum(e) => derive_match_enum(exhaustive, ident, generics, e),
        Data::Union(_) => Error::new(
            Span::call_site(),
            "Cannot derive `Match` for a union, since unions lack a tag",
        )
        .to_compile_error()
        .into(),
    }
}

/// Extract an ordered sequence of field types from a list of fields as `()`, a single `T`, or a
/// tuple, or return `None` if there are more than one named field.
fn ordered_fields_types(fields: Fields) -> Option<Punctuated<Type, Token![,]>> {
    let types = match fields {
        Fields::Named(FieldsNamed { named, .. }) if named.len() > 1 => return None,
        Fields::Named(FieldsNamed { named: fields, .. })
        | Fields::Unnamed(FieldsUnnamed {
            unnamed: fields, ..
        }) => fields.into_iter().map(|f| f.ty).collect(),
        Fields::Unit => vec![parse_quote!(())],
    };
    Some(Punctuated::from_iter(types.into_iter()))
}

/// Extract the field names of a named struct, or count them if they are unnamed.
fn field_names(fields: Fields) -> Result<Punctuated<Ident, Token![,]>, usize> {
    let fields = match fields {
        Fields::Named(FieldsNamed { named: fields, .. })
        | Fields::Unnamed(FieldsUnnamed {
            unnamed: fields, ..
        }) => fields,
        Fields::Unit => Punctuated::new(),
    };
    let len = fields.len();
    fields
        .into_iter()
        .map(|Field { ident, .. }| ident)
        .collect::<Option<_>>()
        .ok_or(len)
}

/// Implement `Case<#n>` for the type `ident` with generics `generics`, constructor `constructor`
/// (this is equal to `ident` for structs, and equal to `ident::constructor` for enums), and fields
/// `fields`.
fn case_impl(
    n: usize,
    ident: Ident,
    generics: Generics,
    constructor: Path,
    fields: Fields,
) -> Option<Item> {
    let vesta_path = vesta_path();
    let case_types = ordered_fields_types(fields.clone())?;
    let (case_body, uncase_body, try_case_body) = match field_names(fields) {
        // In the case of unnamed fields...
        Err(params) => {
            let names: Punctuated<Ident, Token![,]> = (0usize..)
                .map(|i| format_ident!("x_{}", i))
                .take(params)
                .collect();
            (
                quote!({
                    if let #constructor(#names) = self {
                        (#names)
                    } else {
                        #vesta_path::unreachable()
                    }
                }),
                quote!({
                    let (#names) = case;
                    #constructor(#names)
                }),
                quote!({
                    if let #constructor(#names) = self {
                        ::std::result::Result::Ok((#names))
                    } else {
                        ::std::result::Result::Err(self)
                    }
                }),
            )
        }
        // In the case of named fields...
        Ok(field_names) => (
            quote!({
                if let #constructor { #field_names } = self {
                    (#field_names)
                } else {
                    #vesta_path::unreachable()
                }
            }),
            quote!({
                let (#field_names) = case;
                #constructor { #field_names }
            }),
            quote!({
                if let #constructor { #field_names } = self {
                    ::std::result::Result::Ok((#field_names))
                } else {
                    ::std::result::Result::Err(self)
                }
            }),
        ),
    };

    let where_clause = &generics.where_clause;
    Some(parse_quote! {
        impl #generics #vesta_path::Case<#n> for #ident #generics #where_clause {
            type Case = ( #case_types );
            unsafe fn case(self) -> Self::Case #case_body
            fn uncase(case: Self::Case) -> Self #uncase_body
            fn try_case(self) -> ::std::result::Result<Self::Case, Self> #try_case_body
        }
    })
}

/// Derive `Match` for a `struct`
fn derive_match_struct(
    ident: Ident,
    generics: Generics,
    DataStruct { fields, .. }: DataStruct,
) -> TokenStream {
    let fields_span = fields.span();
    if let Some(case_impl) = case_impl(
        0,
        ident.clone(),
        generics.clone(),
        ident.clone().into(),
        fields,
    ) {
        let vesta_path = vesta_path();
        let where_clause = &generics.where_clause;
        TokenStream::from(quote! {
            unsafe impl #generics #vesta_path::Match for #ident #generics #where_clause {
                type Range = #vesta_path::Bounded<1>;

                fn tag(&self) -> ::std::option::Option<::std::primitive::usize> {
                    ::std::option::Option::Some(0)
                }
            }

            #case_impl
        })
    } else {
        Error::new(
            fields_span,
            format!(
                "cannot derive `Match` for the struct `{i}` with more than one named field\n\
            consider making `{i}` a tuple struct, or a wrapper for another type with named fields",
                i = ident
            ),
        )
        .to_compile_error()
        .into()
    }
}

/// Derive `Match` for an `enum`
fn derive_match_enum(
    exhaustive: bool,
    ident: Ident,
    generics: Generics,
    DataEnum { variants, .. }: DataEnum,
) -> TokenStream {
    let vesta_path = vesta_path();

    // Count the number of variants
    let num_variants = variants.len();

    // Construct the `Match` impl
    let mut tag_arms: Vec<Arm> = variants
        .iter()
        .enumerate()
        .map(
            |(
                i,
                Variant {
                    ident: constructor, ..
                },
            )| parse_quote!(#ident::#constructor { .. } => ::std::option::Option::Some(#i)),
        )
        .collect();

    // Only if non-exhaustive, push this fall-through arm
    if !exhaustive {
        tag_arms.push(parse_quote! {
            _ => ::std::option::Option::None
        });
    }

    // Range of the instance
    let range = if exhaustive {
        quote!(#vesta_path::Exhaustive<#num_variants>)
    } else {
        quote!(#vesta_path::Nonexhaustive)
    };

    // Output stream starts with the `Match` impl
    let where_clause = &generics.where_clause;
    let mut output = quote! {
        unsafe impl #generics #vesta_path::Match for #ident #generics #where_clause {
            type Range = #range;

            fn tag(&self) -> ::std::option::Option<::std::primitive::usize> {
                match *self {
                    #(#tag_arms),*
                }
            }
        }
    };

    // Construct each `Case` impl
    let case_impls = variants.into_iter().enumerate().map(
        |(
            n,
            Variant {
                ident: constructor,
                fields,
                ..
            },
        )| {
            let fields_span = fields.span();
            if let Some(case_impl) = case_impl(
                n,
                ident.clone(),
                generics.clone(),
                parse_quote!(#ident::#constructor),
                fields,
            ) {
                quote!(#case_impl)
            } else {
                Error::new(
                    fields_span,
                    format!("cannot derive `Match` for the enum variant `{i}::{c}` with more than one named field\n\
                    consider making `{i}::{c}` a tuple variant, or a wrapper for another type with named fields", i = ident, c = constructor),
                )
                .to_compile_error()
            }
        },
    );

    output.extend(case_impls);
    TokenStream::from(output)
}
