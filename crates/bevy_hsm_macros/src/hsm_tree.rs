use std::ops::Range;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, Ident, LitStr, Token, parse::Parse, punctuated::Punctuated, token};

use crate::state_config::StateConfig;

pub fn hsm_tree_impl(item: TokenStream) -> TokenStream {
    let hsm_tree: HsmTreeImpl = syn::parse_macro_input!(item as StateNode).into();
    quote! {
        bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_commands: EntityCommands|{
            use bevy_hsm::prelude::*;
            let mut commands = entity_commands.commands();
            #hsm_tree
            entity_commands.insert(state_tree)
        })
    }
    .into()
}

#[derive(Debug)]
pub(crate) struct HsmTreeImpl {
    states: Vec<StateNodeImpl>,
    transitions: TransitionImpl,
}

impl From<StateNode> for HsmTreeImpl {
    fn from(value: StateNode) -> Self {
        let mut states = Vec::new();
        let mut transitions = Vec::new();
        let mut state_buffer = vec![value];
        let mut state_buffer2 = Vec::new();
        while !state_buffer.is_empty() {
            let mut start = states.len() + state_buffer.len();
            for StateNode {
                config,
                name,
                components,
                state_children,
            } in std::mem::take(&mut state_buffer)
            {
                transitions.push(start..start + state_children.len());
                start += state_children.len();
                state_buffer2.extend(state_children);
                states.push(StateNodeImpl {
                    name,
                    config,
                    components,
                });
            }
            state_buffer.extend(std::mem::take(&mut state_buffer2));
        }
        Self {
            states,
            transitions: TransitionImpl(transitions),
        }
    }
}

impl quote::ToTokens for HsmTreeImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            states,
            transitions,
        } = self;
        tokens.extend(quote::quote! {
            let ids = [#(commands.spawn((#states)).id()),*];
            let mut state_tree = StateTree::new(ids[0]);
            #transitions
        });
    }
}

#[derive(Debug)]
struct TransitionImpl(Vec<std::ops::Range<usize>>);

impl quote::ToTokens for TransitionImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        for (id, range) in self.0.iter().enumerate() {
            if range.is_empty() {
                continue;
            }
            tokens.extend(match range.len() == 1 {
                true => {
                    let index = range.start;
                    quote::quote! {state_tree.with_add(ids[#id], ids[#index]);}
                }
                false => {
                    let Range { start, end } = range;
                    quote::quote! {state_tree.with_adds(ids[#id], &ids[#start..#end]);}
                }
            });
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateNode {
    name: Option<Ident>,
    config: StateConfig,
    components: Punctuated<Expr, Token![,]>,
    state_children: Punctuated<StateNode, Token![,]>,
}

impl StateNode {
    fn push_with_content(&mut self, content: HsmStateContent) {
        match content {
            HsmStateContent::State(state) => self.state_children.push(state),
            HsmStateContent::Component(component) => self.components.push(component),
            HsmStateContent::States(states) => self.state_children.extend(states),
            HsmStateContent::Components(components) => self.components.extend(components),
        }
    }
}

impl Parse for StateNode {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let config = StateConfig::from_attrs(&attrs)?;
        let name: Option<Ident> = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        let mut state = Self {
            name,
            config,
            ..Default::default()
        };

        if state.config.minimal {
            return Ok(state);
        }
        if input.peek(token::Paren) {
            let content_stream;
            syn::parenthesized!(content_stream in input);
            for content in content_stream.parse_terminated(HsmStateContent::parse, Token![,])? {
                state.push_with_content(content);
            }
        } else if !input.is_empty() && !input.peek(Token![,]) {
            state.push_with_content(input.parse::<HsmStateContent>()?);
        }
        Ok(state)
    }
}

#[derive(Debug)]
struct StateNodeImpl {
    name: Option<Ident>,
    config: StateConfig,
    components: Punctuated<Expr, Token![,]>,
}

impl quote::ToTokens for StateNodeImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            name,
            config,
            components,
        } = self;
        if let Some(name) = name {
            let str = LitStr::new(name.to_string().as_str(), name.span());
            tokens.extend(quote::quote! {Name::new(#str),});
        }

        tokens.extend(config.hsm_state_token_stream());

        if config.is_hsm_any() {
            tokens.extend(quote::quote! {(#config),});
        }

        if !components.is_empty() {
            tokens.extend(quote::quote! {(#components),});
        }
    }
}

enum HsmStateContent {
    State(StateNode),
    Component(Expr),
    States(Punctuated<StateNode, Token![,]>),
    Components(Punctuated<Expr, Token![,]>),
}

impl Parse for HsmStateContent {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let fork = input.fork();
        if let Ok(attrs) = fork.call(syn::Attribute::parse_outer) {
            if attrs.iter().any(|a| a.path().is_ident("state")) {
                return Ok(match input.peek(token::Paren) {
                    true => {
                        let content_stream;
                        syn::parenthesized!(content_stream in input);
                        HsmStateContent::States(
                            content_stream.parse_terminated(StateNode::parse, Token![,])?,
                        )
                    }
                    false => HsmStateContent::State(input.parse()?),
                });
            }
        }
        match input.peek(token::Paren) {
            true => {
                let content_stream;
                syn::parenthesized!(content_stream in input);
                let contents = content_stream.parse_terminated(Expr::parse, Token![,])?;
                Ok(HsmStateContent::Components(contents))
            }
            false => Ok(HsmStateContent::Component(input.parse()?)),
        }
    }
}
