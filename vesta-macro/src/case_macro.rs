use quote::{quote, ToTokens};
use std::collections::BTreeMap;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Arm, Error, Expr, LitInt, Token,
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
    tag_eq: Option<(usize, Token![=])>,
    arm: Arm,
}

impl Parse for CaseArm {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let tag_eq;
        let arm;
        if input.peek(Token![_]) {
            tag_eq = None;
            arm = input.parse()?;
        } else {
            let tag = input.parse::<LitInt>()?.base10_parse::<usize>()?;
            let eq_token = input.parse::<Token![=]>()?;
            tag_eq = Some((tag, eq_token));
            arm = input.parse::<Arm>()?;
        };
        Ok(CaseArm { tag_eq, arm })
    }
}

impl CaseInput {
    pub fn compile(self) -> Result<CaseOutput, Error> {
        let CaseInput {
            scrutinee, arms, ..
        } = self;
        let mut cases: BTreeMap<Option<usize>, Vec<Arm>> = BTreeMap::new();

        for arm in arms {
            cases
                .entry(arm.tag_eq.map(|p| p.0))
                .or_insert_with(Vec::new)
                .push(arm.arm);
        }

        let has_default = cases.contains_key(&None);
        let mut defaults = Vec::new();
        let mut missing_cases = Vec::new();
        let mut ordered_cases = Vec::with_capacity(cases.len());
        let mut previous_tag = None;

        for (option_tag, case) in cases {
            if let Some(tag) = option_tag {
                // Determine if any cases were skipped
                if !has_default {
                    if let Some(previous_tag) = previous_tag {
                        for t in (previous_tag + 1)..tag {
                            missing_cases.push(t);
                        }
                    } else {
                        for t in 0..tag {
                            missing_cases.push(t);
                        }
                    }
                }
                previous_tag = Some(tag);
                ordered_cases.push(case);
            } else {
                defaults = case;
            }
        }

        if missing_cases.is_empty() {
            Ok(CaseOutput {
                scrutinee,
                ordered_cases,
                defaults,
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
    ordered_cases: Vec<Vec<Arm>>,
    defaults: Vec<Arm>,
}

impl ToTokens for CaseOutput {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let vesta_path = crate::vesta_path();

        let CaseOutput {
            scrutinee,
            ordered_cases,
            defaults,
        } = self;

        // Generate the non-default arms
        let mut arms: proc_macro2::TokenStream = Default::default();
        for (tag, inner_cases) in ordered_cases.iter().enumerate() {
            let mut inner_arms: proc_macro2::TokenStream = Default::default();
            for case in inner_cases {
                inner_arms.extend(quote!(#case));
            }
            arms.extend(quote! {
                Some(#tag) => match unsafe { #vesta_path::Case::<#tag>::case(value) } {
                    #inner_arms
                }
            })
        }

        // Generate the default arms
        for default_arm in defaults {
            arms.extend(quote!(#default_arm))
        }

        // Generate the fall-through case
        let num_cases = ordered_cases.len();
        if defaults.is_empty() {
            arms.extend(quote! {
                _ => {
                    #vesta_path::AssertExhaustive::<#num_cases>::assert_exhaustive(&value);
                    unsafe { ::std::hint::unreachable_unchecked() }
                }
            })
        }

        stream.extend(quote!({
            let value = #scrutinee;
            let tag = #vesta_path::Match::tag(&value);
            match tag { #arms }
        }))
    }
}
