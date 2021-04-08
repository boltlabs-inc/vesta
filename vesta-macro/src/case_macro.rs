#![allow(clippy::type_complexity)]
use itertools::Itertools;
use proc_macro2::Span;
use quote::{quote, quote_spanned, ToTokens};
use std::collections::{BTreeMap, BTreeSet};
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    spanned::Spanned,
    token::{Brace, Paren, Underscore},
    Arm, Error, Expr, Ident, LitInt, Pat, PatWild, Token,
};

pub(crate) struct CaseInput {
    scrutinee: Expr,
    brace_token: Brace,
    arms: Vec<CaseArm>,
}

impl Parse for CaseInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let scrutinee = Expr::parse_without_eager_brace(input)?;
        let content;
        let brace_token = braced!(content in input);
        let mut arms = Vec::new();
        while !content.is_empty() {
            arms.push(content.call(CaseArm::parse)?);
        }
        Ok(CaseInput {
            scrutinee,
            arms,
            brace_token,
        })
    }
}

struct CaseArm {
    tag: Option<usize>,
    tag_span: Span,
    arm: Arm,
}

impl Parse for CaseArm {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let tag;
        let tag_span;
        let mut arm;
        if input.peek(Token![_]) {
            // If wildcard pattern, the tag is `None`, parse an arm also with a wildcard pattern
            tag = None;
            tag_span = input.fork().parse::<Token![_]>()?.span();
            arm = input.parse()?;
        } else if input.peek2(Paren) {
            // If of the form `N(...) => ...`, we *consume* the `N` token, then parse an `Arm` with
            // the given pattern (after verifying that the thing *inside* the parentheses is
            // non-empty, so as to make sure you can't write `N()`: you have to do either `N(())` or
            // `N` alone)
            let lit = input.parse::<LitInt>()?;
            tag = Some(lit.base10_parse::<usize>()?);
            tag_span = lit.span();
            let pat;
            parenthesized!(pat in input.fork());
            if pat.is_empty() {
                return Err(pat.error("expected pattern"));
            }
            arm = input.parse::<Arm>()?;
        } else {
            // If of the form `N => ...`, we parse the `N` token but do *not* consume it, then parse
            // an `Arm` which will use that `N` token as its pattern, allowing us to re-use the
            // `Arm`-parsing built into `syn`, then replace the pattern in the `Arm` itself with
            // `_`, which is what we wanted in the first place
            let lit = input.fork().parse::<LitInt>()?;
            tag = Some(lit.base10_parse::<usize>()?);
            tag_span = lit.span();
            arm = input.parse::<Arm>()?;
            // Explicitly construct a `_` pattern with the right span, so unreachable pattern
            // warnings get displayed nicely
            arm.pat = Pat::Wild(PatWild {
                attrs: vec![],
                underscore_token: Underscore { spans: [tag_span] },
            });
        };
        Ok(CaseArm { tag, tag_span, arm })
    }
}

impl CaseInput {
    pub fn compile(self) -> Result<CaseOutput, Error> {
        let CaseInput {
            scrutinee,
            arms,
            brace_token,
        } = self;

        let mut cases: BTreeMap<usize, Vec<(Span, Arm)>> = BTreeMap::new();
        let mut default: Option<(Span, Arm)> = None;
        let mut unreachable: Vec<CaseArm> = Vec::new();
        let mut all_tags = BTreeSet::new();

        for case_arm in arms {
            if default.is_none() {
                if let Some(tag) = case_arm.tag {
                    all_tags.insert(tag);
                    cases
                        .entry(tag)
                        .or_insert_with(Vec::new)
                        .push((case_arm.tag_span, case_arm.arm));
                } else {
                    default = Some((case_arm.tag_span, case_arm.arm));
                }
            } else {
                unreachable.push(case_arm);
            }
        }

        // Compute the missing cases, if any were skipped when there was not a default
        let max_tag: Option<usize> = all_tags.iter().rev().next().cloned();
        let missing_cases = if let Some(max_tag) = max_tag {
            if default.is_none() {
                (0..=max_tag)
                    .filter(|tag| !all_tags.contains(tag))
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        if missing_cases.is_empty() {
            Ok(CaseOutput {
                scrutinee,
                brace_token,
                cases,
                default,
                unreachable,
            })
        } else {
            // Construct the list of missing cases as a nice string
            let mut patterns = String::new();
            let max = missing_cases.len().saturating_sub(1);
            let mut previous = false;
            for (n, tag) in missing_cases.iter().enumerate() {
                if previous {
                    if n == max {
                        if max > 1 {
                            patterns.push(',');
                        }
                        patterns.push_str(" and ");
                    } else {
                        patterns.push_str(", ");
                    }
                }
                patterns.push_str(&format!("`{}(_)`", tag));
                previous = true;
            }
            let message = format!("non-exhaustive patterns: {} not covered", patterns);
            Err(Error::new(scrutinee.span(), message))
        }
    }
}

pub(crate) struct CaseOutput {
    scrutinee: Expr,
    brace_token: Brace,
    cases: BTreeMap<usize, Vec<(Span, Arm)>>,
    default: Option<(Span, Arm)>,
    unreachable: Vec<CaseArm>,
}

impl ToTokens for CaseOutput {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let vesta_path = crate::vesta_path();

        // Generate hygienic idents named "value" and "tag"
        let value_ident = Ident::new("value", Span::mixed_site());
        let tag_ident = Ident::new("tag", Span::mixed_site());

        let CaseOutput {
            scrutinee,
            brace_token,
            cases,
            default,
            unreachable,
        } = self;

        // Get the span for all the cases
        let cases_span = brace_token.span;

        // Compute the max tag ever mentioned
        let mut max_tag = None;
        cases
            .keys()
            .chain(
                unreachable
                    .iter()
                    .filter_map(|case_arm| case_arm.tag.as_ref()),
            )
            .for_each(|tag| {
                max_tag = match max_tag {
                    None => Some(tag),
                    Some(max_tag) => Some(max_tag.max(tag)),
                }
            });

        // Determine whether all the combined cases should have been exhaustive, and if so, what
        // their bound should be
        let exhaustive_cases = if default.is_some() {
            None
        } else {
            Some(max_tag.map(|t| t + 1).unwrap_or(0))
        };

        // Generate the default arm, if one exists
        let default_arm: Vec<_> = default
            .iter()
            .map(|(_, arm)| {
                quote! {
                    #[allow(unreachable_patterns)]
                    #arm
                }
            })
            .collect();

        // Generate all the outer arms
        let active_arms = cases.iter().map(|(tag, inner_cases)| {
            let inner_arms = inner_cases.iter().map(|(_, arm)| arm);
            let tag_span: Span = inner_cases
                .iter()
                .map(|(span, _)| span)
                .cloned()
                .fold1(|s, t| s.join(t).unwrap_or(s))
                .unwrap_or_else(Span::call_site);
            let pat = quote_spanned!(tag_span=> ::std::option::Option::Some(#tag));
            quote! {
                #pat => match unsafe {
                    #vesta_path::Case::<#tag>::case(#value_ident)
                } {
                    #(#inner_arms)*
                    #(#default_arm)*
                }
            }
        });

        // Generate the exhaustive fall-through case, if one is necessary
        let exhaustive_arm = exhaustive_cases.iter().map(|num_cases| {
            quote! {
                _ => {
                    #vesta_path::assert_exhaustive::<_, #num_cases>(&#value_ident);
                    unsafe { #vesta_path::unreachable() }
                }
            }
        });

        // Generate all the unreachable arms, for maximum warning reporting
        let unreachable_arms = unreachable
            .iter()
            .map(|CaseArm { tag, arm, tag_span }| match tag {
                Some(tag) => quote_spanned! { *tag_span=>
                    ::std::option::Option::Some(#tag) => match unsafe {
                        #vesta_path::Case::<#tag>::case(#value_ident)
                    } {
                        #arm
                        _ => unsafe { #vesta_path::unreachable() }
                    }
                },
                None => quote!(#arm),
            });

        // Glue all the arms together
        let arms = active_arms
            .chain(exhaustive_arm.chain(default_arm.iter().cloned().chain(unreachable_arms)));

        stream.extend(quote_spanned!(cases_span=> {
            let #value_ident = #scrutinee;
            let #tag_ident = #vesta_path::Match::tag(&#value_ident);
            #[allow(unused_parens)]
            match #tag_ident {
                #(#arms)*
            }
        }))
    }
}
