use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    Expr, Result, Token, braced,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::{fsm_graph::FsmGraph, hsm::ConfigFn, kw};

// 宏入口
pub fn fsm_impl(item: TokenStream) -> TokenStream {
    let fsm: Fsm = syn::parse_macro_input!(item as Fsm);
    fsm.to_token_stream().into()
}

#[derive(Debug)]
struct Fsm {
    components: Punctuated<Expr, Token![,]>,
    config_fn: Option<ConfigFn>,
    fsm_graph: FsmGraph,
}

impl Parse for Fsm {
    fn parse(input: ParseStream) -> Result<Self> {
        let fsm_graph = input.parse::<FsmGraph>()?;
        if input.peek(Token![,]) && input.peek2(kw::components) {
            input.parse::<Token![,]>()?;
        }
        let components = match input.peek(kw::components) {
            true => {
                input.parse::<kw::components>()?;
                input.parse::<Token![:]>()?;
                let content;
                braced!(content in input);
                let components = content.parse_terminated(Expr::parse, Token![,])?;
                components
            }
            false => Punctuated::new(),
        };
        let config_fn = match input.peek(Token![,])&&input.peek2(Token![:]) {
            true => {
                input.parse::<Token![,]>()?;
                input.parse::<Token![:]>()?;
                ConfigFn::parse(&input)
            }
            false => None,
        };

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }

        Ok(Fsm {
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
            config_fn
        } = self;

        tokens.extend(quote! {
            bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_commands: EntityCommands| {
                use bevy_hsm::prelude::*;
                let mut commands = entity_commands.commands();
                #fsm_graph
                let graph_id = entity_commands.id();
                entity_commands.insert((
                    FsmStateMachine::new(
                        graph_id,
                        init_state_id,
                        #[cfg(feature = "history")]
                        10
                    ),
                    graph,
                    (#components)
                ));
                #config_fn
            })
        });
    }
}
