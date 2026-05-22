//! Proc-macro implementation for the [`fsm_graph!`] and [`fsm!`] macros.
//!
//! Handles parsing of FSM states, transitions, and the graph structure.

use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    Expr, Ident, LitStr, Result, Token, braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

use crate::{
    action_id::{ActionRegistrationList, TransitionRegistrationList},
    guard_condition::GuardCondition,
    kw,
    machine_config::{ConfigFn, StateRef},
    state_config::StateConfig,
};

pub fn fsm_graph_impl(item: TokenStream) -> TokenStream {
    let graph_impl = syn::parse_macro_input!(item as FsmGraphImpl);
    graph_impl.to_token_stream().into()
}

pub struct FsmGraphImpl {
    graph: FsmGraph,
    config_fn: Option<ConfigFn>,
}

impl Parse for FsmGraphImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        let graph = input.parse()?;

        input.parse::<Option<Token![,]>>()?;

        let config_fn = if input.peek(Token![:]) {
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self { graph, config_fn })
    }
}

impl quote::ToTokens for FsmGraphImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let FsmGraphImpl { graph, config_fn } = self;
        tokens.extend(quote! {
            bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_mut:&mut EntityWorldMut| {
                use bevy_hsm::prelude::*;
                #graph
                entity_mut.insert(graph);
                #config_fn
            })
        });
    }
}

#[derive(Debug)]
pub struct FsmGraph {
    action_registry: ActionRegistrationList,
    transition_registry: TransitionRegistrationList,
    pub(crate) states: Punctuated<FsmState, Token![,]>,
    transitions: Punctuated<Transition, Token![,]>,
}

impl Parse for FsmGraph {
    fn parse(input: ParseStream) -> Result<Self> {
        if !input.peek(kw::states) {
            return Err(input.error("expected `states: { ... }` block"));
        }
        input.parse::<kw::states>()?;
        input.parse::<Token![:]>()?;
        let content;
        braced!(content in input);
        let states = content.parse_terminated(FsmState::parse, Token![,])?;
        input.parse::<Option<Token![,]>>()?;
        if !input.peek(kw::transitions) {
            return Err(input.error("expected `transitions: { ... }` block"));
        }
        input.parse::<kw::transitions>()?;
        input.parse::<Token![:]>()?;
        let content;
        braced!(content in input);
        let transitions = content.parse_terminated(Transition::parse, Token![,])?;

        let mut action_registry = Vec::new();
        let mut transition_registry = Vec::new();
        for state in states.iter() {
            state.config.to_actions(&mut action_registry);
            state.config.to_transitions(&mut transition_registry);
        }

        Ok(Self {
            states,
            transitions,
            action_registry: ActionRegistrationList(action_registry),
            transition_registry: TransitionRegistrationList(transition_registry),
        })
    }
}

impl quote::ToTokens for FsmGraph {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let FsmGraph {
            states,
            action_registry,
            transition_registry,
            transitions,
        } = self;

        if states.is_empty() {
            tokens.extend(quote! {
                compile_error!("An FSM must have at least one state.");
            });
            return;
        }

        let mut name_to_index = HashMap::<Ident, usize>::new();
        for (i, state) in states.iter().enumerate() {
            if let Some(name) = &state.name
                && name_to_index.insert(name.clone(), i).is_some()
            {
                tokens.extend(
                    syn::Error::new_spanned(name, "Duplicate state name found.").to_compile_error(),
                );
                return;
            }
        }

        let mut config_errors = Vec::new();
        let spawn_states = states
            .iter()
            .map(|state| {
                config_errors.extend(state.config_error());
                quote! {#state.id()}
            })
            .collect::<Vec<_>>();

        let mut used_states = vec![false; states.len()];
        if !used_states.is_empty() {
            used_states[0] = true;
        }
        let mut resolve_ref = |state_ref: &StateRef| -> Result<proc_macro2::TokenStream> {
            match state_ref {
                StateRef::Index(i) => {
                    let index = i.base10_parse::<usize>()?;
                    if index >= used_states.len() {
                        return Err(syn::Error::new_spanned(i, "State index out of bounds."));
                    }
                    used_states[index] = true;
                    Ok(quote! { ids[#i] })
                }
                StateRef::Named(n) => name_to_index
                    .get(n)
                    .map(|index| {
                        used_states[*index] = true;
                        quote! { ids[#index] }
                    })
                    .ok_or_else(|| syn::Error::new_spanned(n, "State with this name not found.")),
            }
        };
        let build_transitions = match transitions
            .iter()
            .map(|transition| {
                let from = resolve_ref(&transition.from)?;
                let to = resolve_ref(&transition.to)?;

                let add_transition_code = match &transition.condition {
                    TransitionCondition::Unconditional => {
                        quote! { graph.with_add(#from, #to); }
                    }
                    TransitionCondition::OnGuard(guard_expr) => {
                        quote! { graph.with_condition(#from, #guard_expr, #to); }
                    }
                    TransitionCondition::OnEvent(event_expr) => {
                        quote! { graph.with_event(#from, #event_expr, #to); }
                    }
                };

                let add_reverse_transition_code = match &transition.condition {
                    TransitionCondition::Unconditional => {
                        quote! { graph.with_add(#to, #from); }
                    }
                    TransitionCondition::OnGuard(guard_expr) => {
                        quote! { graph.with_condition(#to, #guard_expr, #from); }
                    }
                    TransitionCondition::OnEvent(event_expr) => {
                        quote! { graph.with_event(#to, #event_expr, #from); }
                    }
                };
                Ok(match transition.direction {
                    TransitionDirection::Left => add_reverse_transition_code,
                    TransitionDirection::Right => add_transition_code,
                    TransitionDirection::Both => quote! {
                        #add_transition_code
                        #add_reverse_transition_code
                    },
                })
            })
            .collect::<Result<Vec<_>>>()
        {
            Ok(v) => v,
            Err(e) => {
                tokens.extend(e.to_compile_error());
                return;
            }
        };

        let mut resolution_errors = Vec::new();
        for (i, is_used) in used_states.iter().enumerate() {
            if *is_used {
                continue;
            }
            let state = &states[i];
            let state_description = if let Some(name) = &state.name {
                format!("State `{}` (at index {})", name, i)
            } else {
                format!("State at index {}", i)
            };
            let err = syn::Error::new(
                state.span,
                format!(
                    "{} is defined but not used in any transition.",
                    state_description
                ),
            )
            .to_compile_error();
            resolution_errors.push(err);
        }

        let ids_len = spawn_states.len();

        tokens.extend(quote! {
            #(#resolution_errors);*
            #(#config_errors);*
            #action_registry
            #transition_registry
            let ids = entity_mut.world_scope(move|world| -> [Entity; #ids_len] {
                [#(#spawn_states),*]
            });
            let init_state_id = ids[0];
            let mut graph = FsmGraph::new(init_state_id);
            #(#build_transitions)*
        });
    }
}

#[derive(Debug)]
pub(crate) struct FsmState {
    pub(crate) name: Option<Ident>,
    config: StateConfig,
    components: Punctuated<Expr, Token![,]>,
    span: proc_macro2::Span,
}

impl FsmState {
    fn push_with_content(&mut self, content: FsmStateContent) {
        match content {
            FsmStateContent::Component(component) => self.components.push(component),
            FsmStateContent::Components(v) => self.components.extend(v),
        }
    }

    fn config_error(&self) -> Vec<proc_macro2::TokenStream> {
        let mut errs = Vec::with_capacity(4);
        if self.config.strategy.is_some() {
            errs.push(
                syn::Error::new(self.span, "Strategy is not supported for FSM states.")
                    .to_compile_error(),
            );
        }
        if self.config.behavior.is_some() {
            errs.push(
                syn::Error::new(self.span, "Behavior is not supported for FSM states.")
                    .to_compile_error(),
            );
        }
        if self.config.guard_enter.is_some() {
            errs.push(
                syn::Error::new(self.span, "Enter guard is not supported for FSM states.")
                    .to_compile_error(),
            );
        }

        if self.config.guard_exit.is_some() {
            errs.push(
                syn::Error::new(self.span, "Exit guard is not supported for FSM states.")
                    .to_compile_error(),
            );
        }
        #[cfg(feature = "hybrid")]
        if self.config.fsm_blueprint.is_some() {
            errs.push(
                syn::Error::new(self.span, "fsm_blueprint is not supported for FSM states.")
                    .to_compile_error(),
            );
        }
        errs
    }
}

impl Parse for FsmState {
    fn parse(input: ParseStream) -> Result<Self> {
        let span = input.span();
        // 解析 `#[state(...)]` 和 `#[state_data(...)]` 属性
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let config = StateConfig::from_attrs(&attrs)?;

        let mut state = Self {
            config,
            span,
            ..Default::default()
        };

        // 解析状态名称
        if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            state.name = Some(input.parse()?);
        }

        if state.config.minimal {
            return Ok(state);
        }
        if input.peek(token::Paren) {
            let content_stream;
            syn::parenthesized!(content_stream in input);
            for content in content_stream.parse_terminated(FsmStateContent::parse, Token![,])? {
                state.push_with_content(content);
            }
        } else if !input.is_empty() && !input.peek(Token![,]) {
            state.push_with_content(input.parse()?);
        }

        Ok(state)
    }
}

impl quote::ToTokens for FsmState {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let FsmState {
            name,
            config,
            components,
            ..
        } = self;
        let mut fsm_state = proc_macro2::TokenStream::default();
        if let Some(name) = name {
            let str = LitStr::new(name.to_string().as_str(), name.span());
            fsm_state.extend(quote::quote! {Name::new(#str),});
        }

        fsm_state.extend(config.fsm_state_token_stream());

        if config.is_fsm_any() {
            fsm_state.extend(quote::quote! {(#config),});
        }
        if !components.is_empty() {
            fsm_state.extend(quote::quote! {(#components),});
        }

        if let Some(scene) = &config.scene {
            fsm_state.extend(quote::quote! {#scene,});
        }

        tokens.extend(quote::quote! {world.spawn((#fsm_state))});
    }
}

impl Default for FsmState {
    fn default() -> Self {
        Self {
            name: Default::default(),
            config: Default::default(),
            components: Default::default(),
            span: proc_macro2::Span::call_site(),
        }
    }
}

enum FsmStateContent {
    Component(Expr),
    Components(Punctuated<Expr, Token![,]>),
}

impl Parse for FsmStateContent {
    fn parse(input: ParseStream) -> Result<Self> {
        match input.peek(token::Paren) {
            true => {
                let content_stream;
                syn::parenthesized!(content_stream in input);
                let contents = content_stream.parse_terminated(Expr::parse, Token![,])?;
                Ok(Self::Components(contents))
            }
            false => Ok(Self::Component(input.parse()?)),
        }
    }
}

// --- 3. 转移定义结构体 ---
#[derive(Debug)]
struct Transition {
    pub from: StateRef,
    pub to: StateRef,
    pub condition: TransitionCondition,
    pub direction: TransitionDirection,
}

impl Parse for Transition {
    fn parse(input: ParseStream) -> Result<Self> {
        let from = input.parse::<StateRef>()?;

        let direction_token = input.lookahead1();
        let direction = if direction_token.peek(kw::Both) {
            input.parse::<kw::Both>()?;
            TransitionDirection::Both
        } else if direction_token.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            TransitionDirection::Right
        } else if direction_token.peek(Token![<=]) {
            input.parse::<Token![<=]>()?;
            TransitionDirection::Left
        } else {
            return Err(direction_token.error());
        };

        let to = input.parse::<StateRef>()?;

        let condition = input.parse::<TransitionCondition>()?;

        Ok(Transition {
            from,
            to,
            condition,
            direction,
        })
    }
}

#[derive(Debug)]
enum TransitionDirection {
    // <=
    Left,
    // =>
    Right,
    // <=>
    Both,
}

#[derive(Debug)]
enum TransitionCondition {
    Unconditional,
    OnGuard(GuardCondition),
    OnEvent(Expr),
}

impl Parse for TransitionCondition {
    fn parse(input: ParseStream) -> Result<Self> {
        fn parse_condition<T: Parse>(input: &ParseStream) -> Result<T> {
            let content;
            parenthesized!(content in input);
            if content.is_empty() {
                return Err(content.error("The content of the parentheses is empty."));
            }
            content.parse::<T>()
        }
        if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::guard) {
                input.parse::<kw::guard>()?;
                Ok(TransitionCondition::OnGuard(parse_condition::<
                    GuardCondition,
                >(&input)?))
            } else if lookahead.peek(kw::event) {
                input.parse::<kw::event>()?;
                Ok(TransitionCondition::OnEvent(parse_condition::<Expr>(
                    &input,
                )?))
            } else {
                Err(lookahead.error())
            }
        } else {
            Ok(TransitionCondition::Unconditional)
        }
    }
}
