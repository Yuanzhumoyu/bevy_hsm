use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, Token, parse::Parse, punctuated::Punctuated};

use crate::hsm_tree::{HsmTreeImpl, StateNode};

pub fn hsm_impl(item: TokenStream) -> TokenStream {
    let hsm = syn::parse_macro_input!(item as Hsm);
    let hsm_impl = HsmImpl {
        state_tree: hsm.init_state.into(),
        components: hsm.components,
    };
    quote! {#hsm_impl}.into()
}

#[derive(Debug)]
struct HsmImpl {
    state_tree: HsmTreeImpl,
    components: Punctuated<Expr, Token![,]>,
}

impl quote::ToTokens for HsmImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            state_tree,
            components,
        } = self;

        tokens.extend(quote::quote! {
            bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_commands: EntityCommands|{
                use bevy_hsm::prelude::*;
                let mut commands = entity_commands.commands();
                #state_tree
                let state_machine_id = entity_commands.id();
                entity_commands.insert((
                    HsmStateMachine::new(
                        HsmStateId::new(state_machine_id,ids[0]),
                        #[cfg(feature = "history")]
                        10,
                    ),
                    state_tree,
                    #components
                ));
            })
        });
    }
}

#[derive(Debug)]
struct Hsm {
    init_state: StateNode,
    components: Punctuated<Expr, Token![,]>,
}

impl Parse for Hsm {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut components = Punctuated::<Expr, Token![,]>::new();
        let mut root_state: Option<StateNode> = None;
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
            } else {
                components.push(input.parse::<Expr>()?);
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Hsm {
            init_state: root_state.ok_or_else(|| input.error("Root state is required"))?,
            components,
        })
    }
}
