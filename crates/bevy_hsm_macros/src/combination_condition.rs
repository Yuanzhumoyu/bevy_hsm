use proc_macro::TokenStream;
use syn::{Expr, Ident, Token, parse::Parse, parse_macro_input};

pub fn combination_condition_impl(item: TokenStream) -> TokenStream {
    let constant_value = parse_macro_input!(item as CombinationCondition);
    quote::quote! {
        #constant_value
    }
    .into()
}

#[derive(Clone)]
enum CombinationCondition {
    And(Vec<CombinationCondition>),
    Or(Vec<CombinationCondition>),
    Not(Box<CombinationCondition>),
    Id(Expr),
}

impl quote::ToTokens for CombinationCondition {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            CombinationCondition::And(conditions) => {
                tokens.extend(quote::quote! {
                    CombinationCondition::And(
                        ::smallvec::SmallVec::from_vec(vec![#(Box::new(#conditions)),*])
                    )
                });
            }
            CombinationCondition::Or(conditions) => {
                tokens.extend(quote::quote! {
                    CombinationCondition::Or(
                        ::smallvec::SmallVec::from_vec(vec![#(Box::new(#conditions)),*])
                    )
                });
            }
            CombinationCondition::Not(condition) => {
                tokens.extend(quote::quote! {
                    CombinationCondition::Not(Box::new(#condition))
                });
            }
            CombinationCondition::Id(id) => {
                tokens.extend(quote::quote! {
                    CombinationCondition::from(#id)
                });
            }
        }
    }
}

impl Parse for CombinationCondition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let Ok(ident) = input.parse::<Ident>() else {
            return if let Ok(lit) = input.parse::<syn::ExprLit>() {
                Ok(CombinationCondition::Id(Expr::Lit(lit)))
            } else {
                Ok(CombinationCondition::Id(input.parse::<Expr>()?))
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
                CombinationCondition::And(conditions)
            }
            "or" => {
                let conditions = Self::parse_tuple(input)?;
                if conditions.len() < 2 {
                    return Err(syn::Error::new(
                        input.span(),
                        "not condition must have exactly two argument",
                    ));
                }

                CombinationCondition::Or(conditions)
            }
            "not" => {
                let conditions = Self::parse_tuple(input)?;
                if conditions.len() != 1 {
                    return Err(syn::Error::new(
                        input.span(),
                        "not condition must have exactly one argument",
                    ));
                }
                CombinationCondition::Not(Box::new(conditions[0].clone()))
            }
            _ => CombinationCondition::Id(Expr::Path(syn::ExprPath {
                attrs: vec![],
                qself: None,
                path: syn::Path::from(ident),
            })),
        };
        Ok(cc)
    }
}

impl CombinationCondition {
    fn parse_tuple(input: syn::parse::ParseStream) -> syn::Result<Vec<Self>> {
        let content;
        syn::parenthesized!(content in input);
        let conditions = content.parse_terminated(CombinationCondition::parse, Token![,])?;

        let mut result = Vec::new();
        for condition in conditions {
            result.push(condition);
        }
        Ok(result)
    }
}
