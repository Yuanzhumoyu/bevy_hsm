//! Proc-macro implementation for the [`hsm!`] macro.
//!
//! Parses a root state node, optional `init(...)` config, free components,
//! and an optional `:config_fn` callback.

use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{Expr, Token, parse::Parse, punctuated::Punctuated};

use crate::{
    hsm_tree::{HsmTree, StateNode},
    kw,
    machine_config::{ConfigFn, ResolvedStateMachineConfig, StateMachineConfig},
};

pub fn hsm_impl(item: TokenStream) -> TokenStream {
    let hsm_impl: Hsm = syn::parse_macro_input!(item as Hsm);
    hsm_impl.to_token_stream().into()
}

#[derive(Debug)]
struct Hsm {
    state_tree: HsmTree,
    config_fn: Option<ConfigFn>,
    machine_config: ResolvedStateMachineConfig,
    components: Punctuated<Expr, Token![,]>,
}

impl quote::ToTokens for Hsm {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            state_tree,
            components,
            config_fn,
            machine_config,
        } = self;
        let hsm_state_machine = machine_config.hsm_config();

        tokens.extend(quote::quote! {
            bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_mut:&mut EntityWorldMut|{
                use bevy_hsm::prelude::*;
                #state_tree
                let structure_id = entity_mut.id();
                entity_mut.insert((#hsm_state_machine,state_tree,#components));
                #config_fn
            })
        });
    }
}

impl Parse for Hsm {
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

        // Consume optional trailing comma after init(...) per EBNF
        input.parse::<Option<Token![,]>>()?;

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

        Ok(Hsm {
            state_tree,
            components,
            config_fn,
            machine_config,
        })
    }
}
