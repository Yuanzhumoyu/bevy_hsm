use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, Ident, LitInt, LitStr, Result, Token, braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

use crate::{guard_condition::GuardCondition, kw, state_config::StateConfig};

pub fn fsm_graph_impl(item: TokenStream) -> TokenStream {
    let fsm_graph: FsmGraph = syn::parse_macro_input!(item as FsmGraph);
    quote! {
        bevy_hsm::markers::SpawnStateMachine::new(move |mut entity_commands: EntityCommands| {
            use bevy_hsm::prelude::*;
            let mut commands = entity_commands.commands();
            #fsm_graph
            entity_commands.insert(graph);
        })
    }
    .into()
}

#[derive(Debug)]
pub struct FsmGraph {
    init_state: Option<StateRef>,
    states: Punctuated<State, Token![,]>,
    transitions: Punctuated<Transition, Token![,]>,
}

impl Parse for FsmGraph {
    fn parse(input: ParseStream) -> Result<Self> {
        if !input.peek(kw::states) {
            return Err(input.error("expected `states: { ... }` block"));
        }
        input.parse::<kw::states>()?;
        let mut init_state: Option<StateRef> = None;
        if input.peek(Token![<]) {
            input.parse::<Token![<]>()?;
            init_state = Some(input.parse()?);
            input.parse::<Token![>]>()?;
        }
        input.parse::<Token![:]>()?;
        let content;
        braced!(content in input);
        let states = content.parse_terminated(State::parse, Token![,])?;
        input.parse::<Option<Token![,]>>()?;
        if !input.peek(kw::transitions) {
            return Err(input.error("expected `transitions: { ... }` block"));
        }
        input.parse::<kw::transitions>()?;
        input.parse::<Token![:]>()?;
        let content;
        braced!(content in input);
        let transitions = content.parse_terminated(Transition::parse, Token![,])?;

        Ok(Self {
            init_state,
            states,
            transitions,
        })
    }
}

impl quote::ToTokens for FsmGraph {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let FsmGraph {
            init_state,
            states,
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
            if let Some(name) = &state.name {
                if name_to_index.insert(name.clone(), i).is_some() {
                    tokens.extend(
                        syn::Error::new_spanned(name, "Duplicate state name found.")
                            .to_compile_error(),
                    );
                    return;
                }
            }
        }

        let init_state_index = match init_state {
            Some(StateRef::Named(name)) => {
                if let Some(index) = name_to_index.get(name) {
                    *index
                } else {
                    tokens.extend(
                        syn::Error::new_spanned(name, "Initial state with this name not found.")
                            .to_compile_error(),
                    );
                    return;
                }
            }
            Some(StateRef::Index(i)) => {
                let index = match i.base10_parse::<usize>() {
                    Ok(index) => index,
                    Err(e) => {
                        tokens.extend(e.to_compile_error());
                        return;
                    }
                };
                if index >= states.len() {
                    tokens.extend(
                        syn::Error::new_spanned(i, "Initial state index out of bounds.")
                            .to_compile_error(),
                    );
                    return;
                }
                index
            }
            None => 0,
        };

        let mut config_errors = Vec::new();
        let spawn_states = states
            .iter()
            .map(|state| {
                config_errors.extend(state.config_error());
                quote! {commands.spawn((FsmState,#state)).id()}
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
                        quote! { graph.add(#from, #to); }
                    }
                    TransitionCondition::OnGuard(guard_expr) => {
                        quote! { graph.add_condition(#from, #guard_expr, #to); }
                    }
                    TransitionCondition::OnEvent(event_expr) => {
                        quote! { graph.add_event(#from, #event_expr, #to); }
                    }
                };

                let add_reverse_transition_code = match &transition.condition {
                    TransitionCondition::Unconditional => {
                        quote! { graph.add(#to, #from); }
                    }
                    TransitionCondition::OnGuard(guard_expr) => {
                        quote! { graph.add_condition(#to, #guard_expr, #from); }
                    }
                    TransitionCondition::OnEvent(event_expr) => {
                        quote! { graph.add_event(#to, #event_expr, #from); }
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

        tokens.extend(quote! {
            #(#resolution_errors);*
            #(#config_errors);*
            let ids = [#(#spawn_states),*];
            let init_state_id = ids[#init_state_index];
            let mut graph = FsmGraph::new(init_state_id);
            #(#build_transitions)*
        });
    }
}

#[derive(Debug)]
struct State {
    name: Option<Ident>,
    config: StateConfig,
    components: Punctuated<Expr, Token![,]>,
    span: proc_macro2::Span,
}

impl State {
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
        if self.config.enter_guard.is_some() {
            errs.push(
                syn::Error::new(self.span, "Enter guard is not supported for FSM states.")
                    .to_compile_error(),
            );
        }

        if self.config.exit_guard.is_some() {
            errs.push(
                syn::Error::new(self.span, "Exit guard is not supported for FSM states.")
                    .to_compile_error(),
            );
        }
        if self.config.fsm_blueprint.is_some() {
            errs.push(
                syn::Error::new(self.span, "fsm_blueprint is not supported for FSM states.")
                    .to_compile_error(),
            );
        }
        errs
    }
}

impl Parse for State {
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

impl quote::ToTokens for State {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let State {
            name,
            config,
            components,
            ..
        } = self;
        if let Some(name) = name {
            let str = LitStr::new(name.to_string().as_str(), name.span());
            tokens.extend(quote::quote! {Name::new(#str),});
        }
        if config.is_fsm_any() {
            tokens.extend(quote::quote! {(#config),});
        }
        if !components.is_empty() {
            tokens.extend(quote::quote! {(#components),});
        }
    }
}

impl Default for State {
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
enum StateRef {
    Named(Ident),
    Index(LitInt),
}

impl Parse for StateRef {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(LitInt) {
            Ok(StateRef::Index(input.parse()?))
        } else if input.peek(Ident) {
            Ok(StateRef::Named(input.parse()?))
        } else {
            Err(input.error("Expected a state name (identifier) or index (integer literal)"))
        }
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
                return Err(lookahead.error());
            }
        } else {
            Ok(TransitionCondition::Unconditional)
        }
    }
}
