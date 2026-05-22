//! Proc-macro implementation for the [`fsm!`] macro.
//!
//! Composes an [`FsmGraph`](super::fsm_graph::FsmGraph) with an optional
//! `init(...)` config, a `components: { ... }` block, and a `:config_fn`.

use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    Expr, Result, Token, braced,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::{
    fsm_graph::FsmGraph,
    kw,
    machine_config::{ConfigFn, ResolvedStateMachineConfig, StateMachineConfig},
};

// 宏入口
pub fn fsm_impl(item: TokenStream) -> TokenStream {
    let fsm: Fsm = syn::parse_macro_input!(item as Fsm);
    fsm.to_token_stream().into()
}

#[derive(Debug)]
struct Fsm {
    components: Punctuated<Expr, Token![,]>,
    machine_config: ResolvedStateMachineConfig,
    config_fn: Option<ConfigFn>,
    fsm_graph: FsmGraph,
}

impl Parse for Fsm {
    fn parse(input: ParseStream) -> Result<Self> {
        let machine_config = if input.peek(kw::init) {
            input.parse::<kw::init>()?;
            Some(input.parse::<StateMachineConfig>()?)
        } else {
            None
        };

        let fsm_graph = input.parse::<FsmGraph>()?;
        input.parse::<Option<Token![,]>>()?;

        let components = match input.peek(kw::components) {
            true => {
                input.parse::<kw::components>()?;
                input.parse::<Token![:]>()?;
                let content;
                braced!(content in input);
                let components = content.parse_terminated(Expr::parse, Token![,])?;
                input.parse::<Option<Token![,]>>()?;
                components
            }
            false => Punctuated::new(),
        };

        let config_fn = match input.peek(Token![:]) {
            true => {
                let config_fn = input.parse::<ConfigFn>()?;
                input.parse::<Option<Token![,]>>()?;
                Some(config_fn)
            }
            false => None,
        };

        let machine_config = match machine_config {
            Some(sm) => {
                let name_to_index = fsm_graph
                    .states
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| s.name.as_ref().map(|n| (n.clone(), i)))
                    .collect::<HashMap<_, _>>();
                sm.to_impl(&name_to_index, fsm_graph.states.len())?
            }
            None => Default::default(),
        };

        Ok(Fsm {
            machine_config,
            components,
            fsm_graph,
            config_fn,
        })
    }
}

impl quote::ToTokens for Fsm {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Fsm {
            components,
            fsm_graph,
            machine_config,
            config_fn,
        } = self;

        let fsm_state_machine = machine_config.fsm_config();

        tokens.extend(quote! {
            bevy_hsm::markers::SpawnStateMachine::new(move |entity_mut:&mut EntityWorldMut| {
                use bevy_hsm::prelude::*;
                #fsm_graph
                let structure_id = entity_mut.id();
                entity_mut.insert((#fsm_state_machine,graph,(#components)));
                #config_fn
            })
        });
    }
}
