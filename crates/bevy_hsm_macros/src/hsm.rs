use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, Token, parse::Parse, punctuated::Punctuated};

use crate::{
    hsm_tree::{HsmTree, StateNode},
    kw,
    machine_config::{StateMachineConfig, StateMachineConfigImpl},
};

pub fn hsm_impl(item: TokenStream) -> TokenStream {
    let hsm_impl: HsmImpl = syn::parse_macro_input!(item as HsmImpl);
    quote! {#hsm_impl}.into()
}

#[derive(Debug)]
struct HsmImpl {
    state_tree: HsmTree,
    config_fn: Option<ConfigFn>,
    machine_config: StateMachineConfigImpl,
    components: Punctuated<Expr, Token![,]>,
}

impl quote::ToTokens for HsmImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            state_tree,
            components,
            config_fn,
            machine_config,
        } = self;
        let hsm_state_machine = machine_config.hsm_config();

        tokens.extend(quote::quote! {
            bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_commands: EntityCommands|{
                use bevy_hsm::prelude::*;
                let mut commands = entity_commands.commands();
                #state_tree
                let structure_id = entity_commands.id();
                entity_commands.insert((#hsm_state_machine,state_tree,#components));
                #config_fn
            })
        });
    }
}

impl Parse for HsmImpl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut components = Punctuated::<Expr, Token![,]>::new();
        let mut root_state: Option<StateNode> = None;
        let mut config_fn: Option<ConfigFn> = None;

        let machine_config = if input.peek(kw::init) {
            input.parse::<kw::init>()?;
            Some(input.parse::<StateMachineConfig>()?)
        } else {
            None
        };

        while !input.is_empty() {
            let fork = input.fork();
            let is_state = if let Ok(attrs) = fork.call(syn::Attribute::parse_outer) {
                attrs.iter().any(|a| a.path().is_ident("state"))
            } else {
                false
            };
            if is_state {
                if root_state.is_some() {
                    return Err(syn::Error::new(
                        input.span(),
                        "Only one root state is allowed",
                    ));
                }
                root_state = Some(input.parse()?);
            } else if fork.peek(Token![:]) {
                if config_fn.is_some() {
                    return Err(syn::Error::new(
                        input.span(),
                        "Only one config function is allowed",
                    ));
                }
                config_fn = Some(input.parse::<ConfigFn>()?);
            } else {
                components.push(input.parse::<Expr>()?);
            }
            input.parse::<Option<Token![,]>>()?;
        }
        let state_tree: HsmTree = match root_state {
            Some(state_node) => state_node.into(),
            None => return Err(input.error("Root state is required")),
        };

        let machine_config = match machine_config {
            Some(sm) => {
                let name_to_index = state_tree
                    .states
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| s.name.as_ref().map(|n| (n.clone(), i)))
                    .collect::<HashMap<_, _>>();
                sm.to_impl(&name_to_index, state_tree.states.len())?
            }
            None => Default::default(),
        };

        Ok(HsmImpl {
            state_tree,
            components,
            config_fn,
            machine_config,
        })
    }
}
#[derive(Debug)]
pub enum ConfigFn {
    Closure(syn::ExprClosure),
    Call(syn::ExprCall),
    FnName(syn::Ident),
}

impl Parse for ConfigFn {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<Token![:]>()?;
        if input.peek(syn::Ident) {
            Ok(ConfigFn::FnName(input.parse()?))
        } else {
            Ok(ConfigFn::Closure(input.parse()?))
        }
    }
}

impl quote::ToTokens for ConfigFn {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self {
            ConfigFn::FnName(fn_call) => {
                quote::quote! {#fn_call(entity_commands, &ids);}
            }
            ConfigFn::Closure(closure) => {
                quote::quote! {(#closure)(entity_commands, &ids);}
            }
            ConfigFn::Call(call) => {
                quote::quote! {#call;}
            }
        })
    }
}
