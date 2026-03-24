use proc_macro2::Span;
use quote::quote;
use syn::{LitStr, Token, parse::Parse, spanned::Spanned};

use crate::hsm::ConfigFn;

#[derive(Debug)]
pub enum ActionId {
    Closure((syn::LitStr, syn::ExprClosure)),
    FnIdent((Option<LitStr>, syn::Ident)),
    Call((syn::LitStr, syn::ExprCall)),
    ActionNmae(syn::LitStr),
}

impl ActionId {
    pub fn span(&self) -> Span {
        match self {
            ActionId::Closure(expr_closure) => expr_closure.0.span(),
            ActionId::Call(expr_call) => expr_call.0.span(),
            ActionId::ActionNmae(lit_str) => lit_str.span(),
            ActionId::FnIdent(ident) => match &ident.0 {
                Some(name) => name.span(),
                None => ident.1.span(),
            },
        }
    }

    pub fn to_action(&self) -> Option<(LitStr, ConfigFn)> {
        match self {
            ActionId::Closure((name, closure)) => {
                Some((name.clone(), ConfigFn::Closure(closure.clone())))
            }
            ActionId::Call((name, call)) => Some((name.clone(), ConfigFn::Call(call.clone()))),
            ActionId::FnIdent((name, ident)) => {
                let name = name
                    .clone()
                    .unwrap_or_else(|| LitStr::new(&ident.to_string(), ident.span()));
                Some((name, ConfigFn::FnName(ident.clone())))
            }
            ActionId::ActionNmae(_) => None,
        }
    }
}

impl Parse for ActionId {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(LitStr) {
            Ok(Self::ActionNmae(input.parse()?))
        } else if lookahead.peek(syn::Ident) {
            let ident = input.parse::<syn::Ident>()?;
            Ok(match input.peek(Token![:]) {
                true => {
                    input.parse::<Token![:]>()?;
                    let name = LitStr::new(&ident.to_string(), ident.span());
                    let expr = input.parse::<syn::Expr>()?;
                    match expr {
                        syn::Expr::Closure(calousre) => Self::Closure((name, calousre)),
                        syn::Expr::Call(call) => Self::Call((name, call)),
                        syn::Expr::Path(path) => {
                            let Some(ident) = path.path.get_ident() else {
                                return Err(syn::Error::new(path.span(), "expect function name"));
                            };
                            Self::FnIdent((Some(name), ident.clone()))
                        }
                        _ => {
                            return Err(syn::Error::new(
                                expr.span(),
                                "expect closure, call or function name",
                            ));
                        }
                    }
                }
                false => Self::FnIdent((None, ident)),
            })
        } else {
            Err(lookahead.error())
        }
    }
}

impl quote::ToTokens for ActionId {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self {
            ActionId::Closure((name, _)) => quote! {#name},
            ActionId::ActionNmae(name) => quote! {#name},
            ActionId::Call((name, _)) => quote! {#name},
            ActionId::FnIdent((name, ident)) => {
                let name_str = match name {
                    Some(name) => name,
                    None => &LitStr::new(&ident.to_string(), ident.span()),
                };
                quote! {#name_str}
            }
        })
    }
}

#[derive(Debug)]
pub struct ActionRegistry(pub Vec<(LitStr, ConfigFn)>);

impl quote::ToTokens for ActionRegistry {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        if self.0.is_empty() {
            return;
        }
        let iter = self.0.iter().map(|(name, c)| match c {
            ConfigFn::Closure(expr_closure) => {
                quote! {(#name, commands.register_system(#expr_closure))}
            }
            ConfigFn::FnName(ident) => quote! {(#name, commands.register_system(#ident))},
            ConfigFn::Call(expr_call) => quote! {(#name, commands.register_system(#expr_call))},
        });
        tokens.extend(quote! {
            let action_ids = [#(#iter),*];
            commands.queue(move|world:&mut World|{
                let mut action_registry = world.resource_mut::<ActionRegistry>();
                action_registry.extend(action_ids.into_iter());
            });
        });
    }
}
