use std::collections::HashMap;

use crate::kw::{self};
use proc_macro2::Span;
use quote::quote;
use syn::{
    Ident, LitInt, Result, Token, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

#[derive(Debug, Default)]
pub(crate) struct StateMachineConfigImpl {
    #[cfg(feature = "history")]
    pub history_capacity: Option<LitInt>,
    pub curr_state: usize,
    pub init_state: usize,
}

impl StateMachineConfigImpl {
    #[cfg(feature = "history")]
    fn history_capacity(&self) -> proc_macro2::TokenStream {
        match &self.history_capacity {
            Some(history_capacity) => quote! {#history_capacity},
            None => quote! {10},
        }
    }

    #[cfg(feature = "hsm")]
    pub fn hsm_config(&self) -> proc_macro2::TokenStream {
        let Self {
            curr_state,
            init_state,
            ..
        } = self;
        #[cfg(feature = "history")]
        {
            let history_capacity = self.history_capacity();
            quote! {
                HsmStateMachine::new(HsmStateId::new(structure_id,ids[#init_state]),HsmStateId::new(structure_id,ids[#curr_state]),#history_capacity)
            }
        }
        #[cfg(not(feature = "history"))]
        {
            quote! {
                HsmStateMachine::new(HsmStateId::new(structure_id,ids[#init_state]),HsmStateId::new(structure_id,ids[#curr_state]))
            }
        }
    }

    #[cfg(feature = "fsm")]
    pub fn fsm_config(&self) -> proc_macro2::TokenStream {
        let Self {
            curr_state,
            init_state,
            ..
        } = self;
        #[cfg(feature = "history")]
        {
            let history_capacity = self.history_capacity();
            quote! {
                FsmStateMachine::new(structure_id,ids[#init_state],ids[#curr_state],#history_capacity)
            }
        }
        #[cfg(not(feature = "history"))]
        {
            quote! {
                FsmStateMachine::new(structure_id,ids[#init_state],ids[#curr_state])
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateMachineConfig {
    #[cfg(feature = "history")]
    pub history_capacity: Option<LitInt>,
    pub curr_state: Option<StateRef>,
    pub init_state: Option<StateRef>,
}

impl Parse for StateMachineConfig {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let attrs = Punctuated::<ConfigAttr, Token![,]>::parse_terminated(&content)?;
        #[cfg(feature = "history")]
        let mut history_capacity: Option<LitInt> = None;
        let mut init_state: Option<StateRef> = None;
        let mut curr_state: Option<StateRef> = None;

        for attr in attrs {
            match attr {
                #[cfg(feature = "history")]
                ConfigAttr::HistoryCapacity(val) => {
                    if history_capacity.is_some() {
                        return Err(syn::Error::new(
                            val.span(),
                            "history_capacity already exists",
                        ));
                    }
                    history_capacity = Some(val);
                }
                ConfigAttr::InitState(state_ref) => {
                    if init_state.is_some() {
                        return Err(syn::Error::new(
                            state_ref.span(),
                            "init_state already exists",
                        ));
                    }
                    init_state = Some(state_ref);
                }
                ConfigAttr::CurrState(state_ref) => {
                    if curr_state.is_some() {
                        return Err(syn::Error::new(
                            state_ref.span(),
                            "curr_state already exists",
                        ));
                    }
                    curr_state = Some(state_ref);
                }
            }
        }

        Ok(Self {
            #[cfg(feature = "history")]
            history_capacity,
            init_state,
            curr_state,
        })
    }
}

impl StateMachineConfig {
    pub fn to_impl(
        &self,
        name_to_index: &HashMap<Ident, usize>,
        state_len: usize,
    ) -> syn::Result<StateMachineConfigImpl> {
        let init_state = match &self.init_state {
            Some(StateRef::Named(name)) => {
                if let Some(index) = name_to_index.get(name) {
                    *index
                } else {
                    return Err(syn::Error::new_spanned(
                        name,
                        "Initial state with this name not found.",
                    ));
                }
            }
            Some(StateRef::Index(i)) => {
                let index = i.base10_parse::<usize>()?;
                if index >= state_len {
                    return Err(syn::Error::new_spanned(
                        i,
                        "Initial state index out of bounds.",
                    ));
                }
                index
            }
            None => 0,
        };

        let curr_state = match &self.curr_state {
            Some(StateRef::Named(name)) => {
                if let Some(index) = name_to_index.get(name) {
                    *index
                } else {
                    return Err(syn::Error::new_spanned(
                        name,
                        "Initial state with this name not found.",
                    ));
                }
            }
            Some(StateRef::Index(i)) => {
                let index = i.base10_parse::<usize>()?;
                if index >= state_len {
                    return Err(syn::Error::new_spanned(
                        i,
                        "Initial state index out of bounds.",
                    ));
                }
                index
            }
            None => 0,
        };

        Ok(StateMachineConfigImpl {
            #[cfg(feature = "history")]
            history_capacity: self.history_capacity.clone(),
            curr_state,
            init_state,
        })
    }
}

enum ConfigAttr {
    InitState(StateRef),
    CurrState(StateRef),
    #[cfg(feature = "history")]
    HistoryCapacity(LitInt),
}

impl Parse for ConfigAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        #[cfg(feature = "history")]
        if lookahead.peek(kw::history_capacity) {
            input.parse::<kw::history_capacity>()?;
            input.parse::<Token![=]>()?;
            return Ok(ConfigAttr::HistoryCapacity(input.parse()?));
        }
        if lookahead.peek(kw::init_state) {
            input.parse::<kw::init_state>()?;
            input.parse::<Token![=]>()?;
            Ok(ConfigAttr::InitState(input.parse()?))
        } else if lookahead.peek(kw::curr_state) {
            input.parse::<kw::curr_state>()?;
            input.parse::<Token![=]>()?;
            Ok(ConfigAttr::CurrState(input.parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

#[derive(Debug)]
pub enum StateRef {
    Named(Ident),
    Index(LitInt),
}

impl StateRef {
    pub fn span(&self) -> Span {
        match self {
            StateRef::Named(ident) => ident.span(),
            StateRef::Index(lit_int) => lit_int.span(),
        }
    }
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
