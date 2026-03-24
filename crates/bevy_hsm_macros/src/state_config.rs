use syn::{Expr, Ident, LitStr, Token, parse::Parse, punctuated::Punctuated, spanned::Spanned};

use crate::{action_id::ActionId, guard_condition::GuardCondition, kw, machine_config::ConfigFn};

#[derive(Debug, Default)]
pub(crate) struct StateConfig {
    pub(crate) guard_enter: Option<GuardCondition>,
    pub(crate) guard_exit: Option<GuardCondition>,
    on_update: Option<LitStr>,
    on_enter: Option<ActionId>,
    on_exit: Option<ActionId>,
    pub(crate) strategy: Option<Ident>,
    pub(crate) behavior: Option<Ident>,
    #[cfg(feature = "hybrid")]
    pub(crate) fsm_blueprint: Option<Expr>,
    pub(crate) minimal: bool,
    #[cfg(feature = "state_data")]
    pub(crate) state_datas: Punctuated<Expr, Token![,]>,
}

impl StateConfig {
    #[cfg(feature = "fsm")]
    pub fn is_fsm_any(&self) -> bool {
        #[cfg(feature = "state_data")]
        if !self.state_datas.is_empty() {
            return true;
        }

        self.on_update.is_some() || self.on_enter.is_some() || self.on_exit.is_some()
    }

    #[cfg(feature = "hsm")]
    pub fn is_hsm_any(&self) -> bool {
        #[cfg(feature = "hybrid")]
        if self.fsm_blueprint.is_some() {
            return true;
        }
        #[cfg(feature = "state_data")]
        if !self.state_datas.is_empty() {
            return true;
        }

        self.guard_enter.is_some()
            || self.guard_exit.is_some()
            || self.on_update.is_some()
            || self.on_enter.is_some()
            || self.on_exit.is_some()
            || self.strategy.is_some()
            || self.behavior.is_some()
    }

    #[cfg(feature = "hsm")]
    pub fn is_hsm_state_any(&self) -> bool {
        #[cfg(feature = "hybrid")]
        if self.fsm_blueprint.is_some() {
            return false;
        }
        self.strategy.is_none() && self.behavior.is_none()
    }

    #[cfg(feature = "hsm")]
    pub(super) fn hsm_state_token_stream(&self) -> proc_macro2::TokenStream {
        if self.is_hsm_state_any() {
            return quote::quote! {HsmState::default(),};
        }

        let hsm_state_strategy_field = match &self.strategy {
            Some(strategy) => quote::quote! { strategy: StateTransitionStrategy::#strategy },
            None => quote::quote! { strategy: StateTransitionStrategy::default() },
        };
        let hsm_state_behavior_field = match &self.behavior {
            Some(behavior) => quote::quote! { behavior: ExitTransitionBehavior::#behavior },
            None => quote::quote! { behavior: ExitTransitionBehavior::default() },
        };

        #[cfg(feature = "hybrid")]
        let hsm_state_fsm_blueprint_field = match &self.fsm_blueprint {
            Some(fsm_blueprint) => quote::quote! { fsm_config: Some(#fsm_blueprint) },
            None => quote::quote! { fsm_config: None },
        };

        #[cfg(feature = "hybrid")]
        {
            quote::quote! {HsmState {#hsm_state_strategy_field, #hsm_state_behavior_field, #hsm_state_fsm_blueprint_field,},}
        }
        #[cfg(not(feature = "hybrid"))]
        {
            quote::quote! {HsmState {#hsm_state_strategy_field, #hsm_state_behavior_field,},}
        }
    }

    pub(crate) fn to_actions(&self, actions: &mut Vec<(LitStr, ConfigFn)>) {
        if let Some(enter) = &self.on_enter
            && let Some(action) = enter.to_action()
        {
            actions.push(action);
        }
        if let Some(exit) = &self.on_exit
            && let Some(action) = exit.to_action()
        {
            actions.push(action);
        }
    }

    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut config: StateConfig = Self::default();
        for attr in attrs {
            if attr.path().is_ident("state") {
                if matches!(attr.meta, syn::Meta::Path(_)) {
                    continue;
                }
                let parsed_attrs =
                    attr.parse_args_with(Punctuated::<StateAttrType, Token![,]>::parse_terminated)?;

                for state_attr in parsed_attrs {
                    match state_attr {
                        StateAttrType::EnterGuard(guard) => {
                            if config.guard_enter.is_some() {
                                return Err(syn::Error::new(
                                    guard.span(),
                                    "guard_enter already exists",
                                ));
                            }
                            config.guard_enter = Some(guard);
                        }
                        StateAttrType::ExitGuard(guard) => {
                            if config.guard_exit.is_some() {
                                return Err(syn::Error::new(
                                    guard.span(),
                                    "guard_exit already exists",
                                ));
                            }
                            config.guard_exit = Some(guard);
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
                        #[cfg(feature = "hybrid")]
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
                #[cfg(not(feature = "state_data"))]
                return Err(syn::Error::new(
                    components.span(),
                    "Looking forward to setting up project 'state_data' feature",
                ));

                #[cfg(feature = "state_data")]
                config.state_datas.extend(components);
            }
        }

        Ok(config)
    }
}

impl quote::ToTokens for StateConfig {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            guard_enter,
            guard_exit,
            on_update,
            on_enter,
            on_exit,
            #[cfg(feature = "state_data")]
            state_datas,
            ..
        } = self;
        if let Some(guard_enter) = guard_enter {
            tokens.extend(quote::quote! {EnterGuard(#guard_enter),});
        }
        if let Some(guard_exit) = guard_exit {
            tokens.extend(quote::quote! {ExitGuard(#guard_exit),});
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
        #[cfg(feature = "state_data")]
        if !state_datas.is_empty() {
            tokens.extend(quote::quote! {StateDataBundle::new((#state_datas)),});
        }
    }
}

enum StateAttrType {
    EnterGuard(GuardCondition),
    ExitGuard(GuardCondition),
    OnUpdate(LitStr),
    OnEnter(ActionId),
    OnExit(ActionId),
    Strategy(Ident),
    Behavior(Ident),
    #[cfg(feature = "hybrid")]
    FsmBlueprint(Expr),
    Minimal,
}

impl Parse for StateAttrType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        fn parse_attr<T: Parse, O: Parse>(input: &syn::parse::ParseStream) -> syn::Result<O> {
            input.parse::<T>()?;
            input.parse::<Token![=]>()?;
            input.parse::<O>()
        }
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::minimal) {
            input.parse::<kw::minimal>()?;
            Ok(StateAttrType::Minimal)
        } else if lookahead.peek(kw::guard_enter) {
            Ok(StateAttrType::EnterGuard(parse_attr::<
                kw::guard_enter,
                GuardCondition,
            >(&input)?))
        } else if lookahead.peek(kw::guard_exit) {
            Ok(StateAttrType::ExitGuard(parse_attr::<
                kw::guard_exit,
                GuardCondition,
            >(&input)?))
        } else if lookahead.peek(kw::on_update) {
            Ok(StateAttrType::OnUpdate(
                parse_attr::<kw::on_update, LitStr>(&input)?,
            ))
        } else if lookahead.peek(kw::on_enter) {
            Ok(StateAttrType::OnEnter(
                parse_attr::<kw::on_enter, ActionId>(&input)?,
            ))
        } else if lookahead.peek(kw::on_exit) {
            Ok(StateAttrType::OnExit(parse_attr::<kw::on_exit, ActionId>(
                &input,
            )?))
        } else if lookahead.peek(kw::strategy) {
            Ok(StateAttrType::Strategy(parse_attr::<kw::strategy, Ident>(
                &input,
            )?))
        } else if lookahead.peek(kw::behavior) {
            Ok(StateAttrType::Behavior(parse_attr::<kw::behavior, Ident>(
                &input,
            )?))
        } else {
            #[cfg(feature = "hybrid")]
            if lookahead.peek(kw::fsm_blueprint) {
                return Ok(StateAttrType::FsmBlueprint(parse_attr::<
                    kw::fsm_blueprint,
                    Expr,
                >(&input)?));
            }
            Err(lookahead.error())
        }
    }
}
