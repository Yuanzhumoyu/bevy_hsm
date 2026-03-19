use proc_macro::TokenStream;
use syn::{Expr, Token, parse::Parse, parse_macro_input};

use crate::kw;

pub fn guard_condition_impl(item: TokenStream) -> TokenStream {
    let constant_value = parse_macro_input!(item as GuardCondition);
    quote::quote! {
        #constant_value
    }
    .into()
}

#[derive(Clone, Debug)]
pub(super) enum GuardCondition {
    And(Vec<GuardCondition>),
    Or(Vec<GuardCondition>),
    Not(Box<GuardCondition>),
    Id(Expr),
}

impl quote::ToTokens for GuardCondition {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            GuardCondition::And(conditions) => {
                tokens.extend(quote::quote! {
                    GuardCondition::And(
                        ::smallvec::SmallVec::from_vec(vec![#(Box::new(#conditions)),*])
                    )
                });
            }
            GuardCondition::Or(conditions) => {
                tokens.extend(quote::quote! {
                    GuardCondition::Or(
                        ::smallvec::SmallVec::from_vec(vec![#(Box::new(#conditions)),*])
                    )
                });
            }
            GuardCondition::Not(condition) => {
                tokens.extend(quote::quote! {
                    GuardCondition::Not(Box::new(#condition))
                });
            }
            GuardCondition::Id(id) => {
                tokens.extend(quote::quote! {
                    GuardCondition::from(#id)
                });
            }
        }
    }
}

impl Parse for GuardCondition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        let gc = if lookahead.peek(kw::and) {
            input.parse::<kw::and>()?;
            let conditions = Self::parse_tuple(input)?;
            if conditions.len() < 2 {
                return Err(syn::Error::new(
                    input.span(),
                    "and condition must have at least two arguments",
                ));
            }
            GuardCondition::And(conditions)
        } else if lookahead.peek(kw::or) {
            input.parse::<kw::or>()?;
            let conditions = Self::parse_tuple(input)?;
            if conditions.len() < 2 {
                return Err(syn::Error::new(
                    input.span(),
                    "or condition must have at least two arguments",
                ));
            }
            GuardCondition::Or(conditions)
        } else if lookahead.peek(kw::not) {
            input.parse::<kw::not>()?;
            let conditions = Self::parse_tuple(input)?;
            if conditions.len() != 1 {
                return Err(syn::Error::new(
                    input.span(),
                    "not condition must have exactly one argument",
                ));
            }
            GuardCondition::Not(Box::new(conditions.into_iter().next().unwrap()))
        } else if lookahead.peek(syn::LitStr) {
            GuardCondition::Id(input.parse()?)
        } else if lookahead.peek(Token![#]) && input.peek2(syn::Ident) {
            input.parse::<Token![#]>()?;
            GuardCondition::Id(input.parse()?)
        } else {
            return Err(lookahead.error());
        };
        Ok(gc)
    }
}

impl GuardCondition {
    fn parse_tuple(input: syn::parse::ParseStream) -> syn::Result<Vec<Self>> {
        let content;
        syn::parenthesized!(content in input);
        let conditions = content.parse_terminated(GuardCondition::parse, Token![,])?;

        let result = conditions.into_iter().collect();
        Ok(result)
    }
}
