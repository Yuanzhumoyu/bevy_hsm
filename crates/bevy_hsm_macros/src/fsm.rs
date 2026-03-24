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
    machine_config::{ConfigFn, StateMachineConfig, StateMachineConfigImpl},
};

// 宏入口
pub fn fsm_impl(item: TokenStream) -> TokenStream {
    let fsm: Fsm = syn::parse_macro_input!(item as Fsm);
    fsm.to_token_stream().into()
}

#[derive(Debug)]
struct Fsm {
    components: Punctuated<Expr, Token![,]>,
    machine_config: StateMachineConfigImpl,
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
                content.parse_terminated(Expr::parse, Token![,])?
            }
            false => Punctuated::new(),
        };
        input.parse::<Option<Token![,]>>()?;

        let config_fn = match input.peek(Token![:]) {
            true => Some(input.parse::<ConfigFn>()?),
            false => None,
        };
        input.parse::<Option<Token![,]>>()?;

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
            bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_commands: EntityCommands| {
                use bevy_hsm::prelude::*;
                let mut commands = entity_commands.commands();
                #fsm_graph
                let structure_id = entity_commands.id();
                entity_commands.insert((#fsm_state_machine,graph,(#components)));
                #config_fn
            })
        });
    }
}
