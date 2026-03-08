use quote::quote;
use syn::{Expr, Ident, LitStr, Token, parse::Parse, punctuated::Punctuated, spanned::Spanned};

use crate::guard_condition::GuardCondition;

#[derive(Debug, Default)]
pub(crate) struct StateConfig {
    pub(crate) enter_guard: Option<GuardCondition>,
    pub(crate) exit_guard: Option<GuardCondition>,
    on_update: Option<LitStr>,
    on_enter: Option<LitStr>,
    on_exit: Option<LitStr>,
    pub(crate) strategy: Option<Ident>,
    pub(crate) behavior: Option<Ident>,
    pub(crate) fsm_blueprint: Option<Expr>,
    pub(crate) minimal: bool,
    pub(crate) state_datas: Punctuated<Expr, Token![,]>,
}

impl StateConfig {
    pub fn is_fsm_any(&self) -> bool {
        self.on_update.is_some()
            || self.on_enter.is_some()
            || self.on_exit.is_some()
            || !self.state_datas.is_empty()
    }

    pub fn is_hsm_any(&self) -> bool {
        self.enter_guard.is_some()
            || self.exit_guard.is_some()
            || self.on_update.is_some()
            || self.on_enter.is_some()
            || self.on_exit.is_some()
            || !self.state_datas.is_empty()
    }

    pub(super) fn hsm_state_token_stream(&self) -> proc_macro2::TokenStream {
        if self.strategy.is_none() && self.behavior.is_none() && self.fsm_blueprint.is_none() {
            return quote! {HsmState::default(),};
        }

        let hsm_state_strategy_field = match &self.strategy {
            Some(strategy) => quote::quote! { strategy: StateTransitionStrategy::#strategy },
            None => quote::quote! { strategy: StateTransitionStrategy::default() },
        };
        let hsm_state_behavior_field = match &self.behavior {
            Some(behavior) => quote::quote! { behavior: ExitTransitionBehavior::#behavior },
            None => quote::quote! { behavior: ExitTransitionBehavior::default() },
        };
        let hsm_state_fsm_blueprint_field = match &self.fsm_blueprint {
            Some(fsm_blueprint) => quote::quote! { fsm_config: Some(#fsm_blueprint) },
            None => quote::quote! { fsm_config: None },
        };
        quote::quote! {HsmState {#hsm_state_strategy_field, #hsm_state_behavior_field, #[cfg(feature = "fsm")] #hsm_state_fsm_blueprint_field,},}
    }

    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut config: StateConfig = Self::default();
        for attr in attrs {
            if attr.path().is_ident("state") {
                if matches!(attr.meta, syn::Meta::Path(_)) {
                    continue;
                }
                let Ok(parsed_attrs) =
                    attr.parse_args_with(Punctuated::<StateAttrType, Token![,]>::parse_terminated)
                else {
                    return Err(syn::Error::new(
                        attr.span(),
                        "Invalid state attribute format. Expected `#[state]` or `#[state(...)]`",
                    ));
                };

                for state_attr in parsed_attrs {
                    match state_attr {
                        StateAttrType::EnterGuard(guard) => {
                            if config.enter_guard.is_some() {
                                return Err(syn::Error::new(
                                    guard.span(),
                                    "enter_guard already exists",
                                ));
                            }
                            config.enter_guard = Some(guard);
                        }
                        StateAttrType::ExitGuard(guard) => {
                            if config.exit_guard.is_some() {
                                return Err(syn::Error::new(
                                    guard.span(),
                                    "exit_guard already exists",
                                ));
                            }
                            config.exit_guard = Some(guard);
                        }
                        StateAttrType::OnUpdate(update) => {
                            if config.on_update.is_some() {
                                return Err(syn::Error::new(
                                    update.span(),
                                    "on_update already exists",
                                ));
                            }
                            config.on_update = Some(update);
                        }
                        StateAttrType::OnEnter(enter) => {
                            if config.on_enter.is_some() {
                                return Err(syn::Error::new(
                                    enter.span(),
                                    "on_enter already exists",
                                ));
                            }
                            config.on_enter = Some(enter);
                        }
                        StateAttrType::OnExit(exit) => {
                            if config.on_exit.is_some() {
                                return Err(syn::Error::new(exit.span(), "on_exit already exists"));
                            }
                            config.on_exit = Some(exit);
                        }
                        StateAttrType::Strategy(strategy) => {
                            if config.strategy.is_some() {
                                return Err(syn::Error::new(
                                    strategy.span(),
                                    "strategy already exists",
                                ));
                            }
                            config.strategy = Some(strategy);
                        }
                        StateAttrType::Behavior(behavior) => {
                            if config.behavior.is_some() {
                                return Err(syn::Error::new(
                                    behavior.span(),
                                    "behavior already exists",
                                ));
                            }
                            config.behavior = Some(behavior);
                        }
                        StateAttrType::FsmBlueprint(fsm_blueprint) => {
                            if config.fsm_blueprint.is_some() {
                                return Err(syn::Error::new(
                                    fsm_blueprint.span(),
                                    "fsm_config already exists",
                                ));
                            }
                            config.fsm_blueprint = Some(fsm_blueprint);
                        }
                        StateAttrType::Minimal => {
                            config.minimal = true;
                        }
                    }
                }
            } else if attr.path().is_ident("state_data") {
                let syn::Meta::List(list) = &attr.meta else {
                    return Err(syn::Error::new(
                        attr.span(),
                        "Invalid state_data attribute format. Expected `#[state_data(...)]`",
                    ));
                };

                let components =
                    list.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;

                if components.is_empty() {
                    return Err(syn::Error::new(
                        attr.span(),
                        "state_data attribute must have at least one component",
                    ));
                }
                config.state_datas.extend(components);
            }
        }

        Ok(config)
    }
}

impl quote::ToTokens for StateConfig {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            enter_guard,
            exit_guard,
            on_update,
            on_enter,
            on_exit,
            state_datas,
            ..
        } = self;
        if let Some(enter_guard) = enter_guard {
            tokens.extend(quote::quote! {EnterGuard(#enter_guard),});
        }
        if let Some(exit_guard) = exit_guard {
            tokens.extend(quote::quote! {ExitGuard(#exit_guard),});
        }
        if let Some(on_update) = on_update {
            tokens.extend(quote::quote! {OnUpdateSystem::new(#on_update),});
        }
        if let Some(on_enter) = on_enter {
            tokens.extend(quote::quote! {OnEnterSystem::new(#on_enter),});
        }
        if let Some(on_exit) = on_exit {
            tokens.extend(quote::quote! {OnExitSystem::new(#on_exit),});
        }
        if !state_datas.is_empty() {
            tokens
                .extend(quote::quote! {#[cfg(feature = "state_data")]state_data::StateDataBundle::new((#state_datas)),});
        }
    }
}

enum StateAttrType {
    EnterGuard(GuardCondition),
    ExitGuard(GuardCondition),
    OnUpdate(LitStr),
    OnEnter(LitStr),
    OnExit(LitStr),
    Strategy(Ident),
    Behavior(Ident),
    FsmBlueprint(Expr),
    Minimal,
}

impl Parse for StateAttrType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let ident_str = ident.to_string();
        if ident_str == "minimal" {
            return Ok(StateAttrType::Minimal);
        }
        input.parse::<Token![=]>()?;
        match ident_str.as_str() {
            "exit_guard" => Ok(StateAttrType::ExitGuard(input.parse()?)),
            "enter_guard" => Ok(StateAttrType::EnterGuard(input.parse()?)),
            "on_update" => Ok(StateAttrType::OnUpdate(input.parse()?)),
            "on_enter" => Ok(StateAttrType::OnEnter(input.parse()?)),
            "on_exit" => Ok(StateAttrType::OnExit(input.parse()?)),
            "strategy" => Ok(StateAttrType::Strategy(input.parse()?)),
            "behavior" => Ok(StateAttrType::Behavior(input.parse()?)),
            "fsm_blueprint" => Ok(StateAttrType::FsmBlueprint(input.parse()?)),
            _ => Err(syn::Error::new(ident.span(), "Invalid state attribute")),
        }
    }
}
