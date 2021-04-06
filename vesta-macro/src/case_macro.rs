use itertools::Itertools;
use proc_macro2::Span;
use quote::{quote, quote_spanned, ToTokens};
use std::collections::{BTreeMap, BTreeSet};
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_quote,
    spanned::Spanned,
    token::Paren,
    Arm, Error, Expr, Ident, LitInt, Pat, Token,
};

pub(crate) struct CaseInput {
    scrutinee: Expr,
    arms: Vec<CaseArm>,
}

impl Parse for CaseInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let scrutinee = Expr::parse_without_eager_brace(input)?;
        let content;
        let _brace_token = braced!(content in input);
        let mut arms = Vec::new();
        while !content.is_empty() {
            arms.push(content.call(CaseArm::parse)?);
        }
        Ok(CaseInput { scrutinee, arms })
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
            // the given pattern (after verifying that the thing *inside* the parentheses is a
            // pattern, so as to make sure you can't omit parentheses)
            let lit = input.parse::<LitInt>()?;
            tag = Some(lit.base10_parse::<usize>()?);
            tag_span = lit.span();
            let pat;
            parenthesized!(pat in input.fork());
            pat.parse::<Pat>()?;
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
            arm.pat = parse_quote!(_);
        };
        Ok(CaseArm { tag, tag_span, arm })
    }
}

impl CaseInput {
    pub fn compile(self) -> Result<CaseOutput, Error> {
        let CaseInput {
            scrutinee, arms, ..
        } = self;

        let mut cases: Vec<BTreeMap<Option<usize>, Vec<(Span, Arm)>>> = Vec::new();
        let mut all_tags = BTreeSet::new();
        let mut has_default = false;

        for (_, group) in arms.into_iter().group_by(|c| c.tag.is_some()).into_iter() {
            let mut grouped_arms = BTreeMap::new();
            for case_arm in group {
                match case_arm.tag {
                    Some(tag) => {
                        all_tags.insert(tag);
                    }
                    None => has_default = true,
                }
                grouped_arms
                    .entry(case_arm.tag)
                    .or_insert_with(Vec::new)
                    .push((case_arm.tag_span, case_arm.arm));
            }
            cases.push(grouped_arms);
        }

        // Compute the missing cases, if any were skipped when there was not a default
        let max_tag: Option<usize> = all_tags.iter().rev().next().cloned();
        let missing_cases = if let Some(max_tag) = max_tag {
            if !has_default {
                (0..=max_tag)
                    .filter(|tag| !all_tags.contains(tag))
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // If the match is supposed to be exhaustive, count the bound on the cases
        let exhaustive_cases = if has_default {
            None
        } else {
            Some(max_tag.map(|t| t + 1).unwrap_or(0))
        };

        if missing_cases.is_empty() {
            Ok(CaseOutput {
                scrutinee,
                cases,
                exhaustive_cases,
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
                patterns.push_str(&format!("`{} = _`", tag));
                previous = true;
            }
            let message = format!("non-exhaustive patterns: {} not covered", patterns);
            Err(Error::new(scrutinee.span(), message))
        }
    }
}

pub(crate) struct CaseOutput {
    scrutinee: Expr,
    cases: Vec<BTreeMap<Option<usize>, Vec<(Span, Arm)>>>,
    exhaustive_cases: Option<usize>,
}

impl ToTokens for CaseOutput {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let vesta_path = crate::vesta_path();

        // Generate hygienic idents named "value" and "tag"
        let value_ident = Ident::new("value", Span::mixed_site());
        let tag_ident = Ident::new("tag", Span::mixed_site());

        let CaseOutput {
            scrutinee,
            cases,
            exhaustive_cases,
        } = self;

        // Compute the grouped sets of defaults, one for each case group
        let mut defaults: BTreeMap<usize, Vec<(&Span, &Arm)>> = cases
            .iter()
            .map(|map| {
                map.iter()
                    .filter_map(|(tag, arms)| if tag.is_none() { Some(arms) } else { None })
                    .map(|v| v.iter().map(|(tag_span, arm)| (tag_span, arm)))
                    .flatten()
                    .collect()
            })
            .enumerate()
            .collect();

        // Generate all the outer arms
        let mut arms: proc_macro2::TokenStream = Default::default();
        for (group_number, group) in cases.iter().enumerate() {
            for (tag, inner_cases) in group.iter() {
                let mut inner_arms: proc_macro2::TokenStream = Default::default();
                for (_tag_span, case) in inner_cases {
                    inner_arms.extend(quote!(#case));
                }
                if let Some(tag) = tag {
                    // Make a list of all the default arms that exist *syntactically below* this
                    // point in the input, and emit them here if so. This means that partial matches
                    // within a particular tag won't create an error if there is a global default
                    // arm(s).
                    let defaults_below_here: Vec<(&Span, &Arm)> =
                        defaults.values().flatten().copied().collect();
                    let mut default_arms_below_here: proc_macro2::TokenStream = Default::default();
                    for (_, arm) in defaults_below_here {
                        default_arms_below_here.extend(quote! {
                            #[allow(unreachable_patterns)]
                            #arm
                        });
                    }

                    // Compute a span that's the join of all the spans for each tag
                    let tag_span = inner_cases
                        .iter()
                        .map(|p| p.0)
                        .fold1(|x, y| x.join(y).unwrap_or(x))
                        .unwrap_or_else(Span::call_site);

                    arms.extend(quote_spanned!(tag_span=> Some(#tag)));
                    arms.extend(quote! {
                        => match unsafe {
                            #vesta_path::Case::<#tag>::case(#value_ident)
                        } {
                            #inner_arms
                            #default_arms_below_here
                        }
                    });
                } else {
                    arms.extend(quote!(#inner_arms))
                }
            }
            // Remove the current group number from the defaults
            defaults.remove(&group_number);
        }

        // Generate the fall-through case
        if let Some(num_cases) = exhaustive_cases {
            arms.extend(quote! {
                _ => {
                    #vesta_path::AssertExhaustive::<#num_cases>::assert_exhaustive(&#value_ident);
                    unsafe { ::std::hint::unreachable_unchecked() }
                }
            })
        }

        stream.extend(quote!({
            let #value_ident = #scrutinee;
            let #tag_ident = #vesta_path::Match::tag(&#value_ident);
            #[allow(unused_parens)]
            match #tag_ident { #arms }
        }))
    }
}
