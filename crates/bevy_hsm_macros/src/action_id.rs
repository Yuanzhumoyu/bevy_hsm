//! Types for parsing and code-generating action-system registrations.
//!
//! [`ActionId`] represents an action reference in a `#[state(...)]` attribute.
//! [`ActionRegistrationList`] and [`TransitionRegistrationList`] collect the
//! registrations discovered during macro expansion and emit code that registers
//! them with the runtime [`ActionRegistry`] / [`TransitionRegistry`] resources.

use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{LitStr, Token, parse::Parse, spanned::Spanned};

use crate::machine_config::ConfigFn;

/// An action reference appearing in a `#[state(...)]` attribute value.
///
/// Supports several syntactic forms:
/// | Form | Example | Meaning |
/// |------|---------|---------|
/// | String literal | `"on_enter"` | Look up by name in the registry |
/// | Bare ident | `on_enter` | Same, using the ident as the name |
/// | Name + closure | `tag: \|ctx\| { ... }` | Inline closure with a name |
/// | Name + call | `tag: my_fn(a, b)` | Function call expression |
/// | Name + path | `tag: my_fn` | Named function reference |
#[derive(Debug)]
pub enum ActionId {
    Closure((syn::LitStr, syn::ExprClosure)),
    FnIdent((Option<LitStr>, syn::Expr)),
    Call((syn::LitStr, syn::ExprCall)),
    ActionName(syn::LitStr),
}

impl ActionId {
    pub fn span(&self) -> Span {
        match self {
            ActionId::Closure(expr_closure) => expr_closure.0.span(),
            ActionId::Call(expr_call) => expr_call.0.span(),
            ActionId::ActionName(lit_str) => lit_str.span(),
            ActionId::FnIdent(expr) => match &expr.0 {
                Some(name) => name.span(),
                None => expr.1.span(),
            },
        }
    }

    pub fn to_action(&self) -> Option<(LitStr, ConfigFn)> {
        match self {
            ActionId::Closure((name, closure)) => {
                Some((name.clone(), ConfigFn::Closure(closure.clone())))
            }
            ActionId::Call((name, call)) => Some((name.clone(), ConfigFn::Call(call.clone()))),
            ActionId::FnIdent((name, expr)) => {
                let name = name.clone().unwrap_or_else(|| {
                    LitStr::new(&expr.to_token_stream().to_string(), expr.span())
                });
                Some((name, ConfigFn::FnName(expr.clone())))
            }
            ActionId::ActionName(_) => None,
        }
    }
}

impl Parse for ActionId {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(LitStr) {
            Ok(Self::ActionName(input.parse()?))
        } else if lookahead.peek(syn::Ident) {
            let ident = input.parse::<syn::Ident>()?;
            Ok(match input.peek(Token![:]) {
                true => {
                    input.parse::<Token![:]>()?;
                    let name = LitStr::new(&ident.to_string(), ident.span());
                    let expr = input.parse::<syn::Expr>()?;
                    match expr {
                        syn::Expr::Closure(closure) => Self::Closure((name, closure)),
                        syn::Expr::Call(call) => Self::Call((name, call)),
                        syn::Expr::Path(path) => Self::FnIdent((Some(name), syn::Expr::Path(path))),
                        _ => {
                            return Err(syn::Error::new(
                                expr.span(),
                                "expect closure, call or function name",
                            ));
                        }
                    }
                }
                false => {
                    let path: syn::Path = ident.into();
                    let expr = syn::Expr::Path(syn::ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path,
                    });
                    Self::FnIdent((None, expr))
                }
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
            ActionId::ActionName(name) => quote! {#name},
            ActionId::Call((name, _)) => quote! {#name},
            ActionId::FnIdent((name, ident)) => {
                let name_str = match name {
                    Some(name) => name,
                    None => &LitStr::new(&ident.to_token_stream().to_string(), ident.span()),
                };
                quote! {#name_str}
            }
        })
    }
}

/// Collection of action systems to register into the [`ActionRegistry`] resource.
///
/// Generated during macro expansion, this struct outputs code that registers
/// the collected systems into the runtime [`ActionRegistry`] (defined in `bevy_hsm::state_actions`).
#[derive(Debug)]
pub struct ActionRegistrationList(pub Vec<(LitStr, ConfigFn)>);

impl quote::ToTokens for ActionRegistrationList {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        if self.0.is_empty() {
            return;
        }
        let iter = self.0.iter().map(|(name, c)| match c {
            ConfigFn::Closure(expr_closure) => {
                quote! {(#name, world.register_system(#expr_closure))}
            }
            ConfigFn::FnName(ident) => quote! {(#name, world.register_system(#ident))},
            ConfigFn::Call(expr_call) => quote! {(#name, world.register_system(#expr_call))},
        });
        tokens.extend(quote! {
            entity_mut.world_scope(move|world:&mut World| {
                let action_ids = [#(#iter),*];
                let mut action_registry = world.resource_mut::<ActionRegistry>();
                action_registry.extend(action_ids.into_iter());
            });
        });
    }
}

/// Collection of transition systems to register into the [`TransitionRegistry`] resource.
///
/// Generated during macro expansion, this struct outputs code that registers
/// the collected systems into the runtime [`TransitionRegistry`] (defined in `bevy_hsm::state_actions`).
#[derive(Debug)]
pub struct TransitionRegistrationList(pub Vec<(LitStr, ConfigFn)>);

impl quote::ToTokens for TransitionRegistrationList {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        if self.0.is_empty() {
            return;
        }
        let iter = self.0.iter().map(|(name, c)| match c {
            ConfigFn::Closure(expr_closure) => {
                quote! {(#name, world.register_system(#expr_closure))}
            }
            ConfigFn::FnName(ident) => quote! {(#name, world.register_system(#ident))},
            ConfigFn::Call(expr_call) => quote! {(#name, world.register_system(#expr_call))},
        });
        tokens.extend(quote! {
            entity_mut.world_scope(move|world:&mut World| {
                let transition_ids = [#(#iter),*];
                let mut transition_registry = world.resource_mut::<TransitionRegistry>();
                transition_registry.extend(transition_ids.into_iter());
            });
        });
    }
}
