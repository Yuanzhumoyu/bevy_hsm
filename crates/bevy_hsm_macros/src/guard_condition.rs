use proc_macro::TokenStream;
use syn::{Expr, Ident, Token, parse::Parse, parse_macro_input};

pub fn guard_condition_impl(item: TokenStream) -> TokenStream {
    let constant_value = parse_macro_input!(item as GuardCondition);
    quote::quote! {
        #constant_value
    }
    .into()
}

#[derive(Clone)]
enum GuardCondition {
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
        let Ok(ident) = input.parse::<Ident>() else {
            return if let Ok(lit) = input.parse::<syn::ExprLit>() {
                Ok(GuardCondition::Id(Expr::Lit(lit)))
            } else {
                Ok(GuardCondition::Id(input.parse::<Expr>()?))
            };
        };
        let cc = match ident.to_string().as_str() {
            "and" => {
                let conditions = Self::parse_tuple(input)?;
                if conditions.len() < 2 {
                    return Err(syn::Error::new(
                        input.span(),
                        "not condition must have exactly two argument",
                    ));
                }
                GuardCondition::And(conditions)
            }
            "or" => {
                let conditions = Self::parse_tuple(input)?;
                if conditions.len() < 2 {
                    return Err(syn::Error::new(
                        input.span(),
                        "not condition must have exactly two argument",
                    ));
                }

                GuardCondition::Or(conditions)
            }
            "not" => {
                let conditions = Self::parse_tuple(input)?;
                if conditions.len() != 1 {
                    return Err(syn::Error::new(
                        input.span(),
                        "not condition must have exactly one argument",
                    ));
                }
                GuardCondition::Not(Box::new(conditions.into_iter().next().unwrap()))
            }
            _ => GuardCondition::Id(Expr::Path(syn::ExprPath {
                attrs: Vec::new(),
                qself: None,
                path: syn::Path::from(ident),
            })),
        };
        Ok(cc)
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
