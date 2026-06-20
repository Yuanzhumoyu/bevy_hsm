//! Types for parsing `#[state(...)]` attributes and emitting the
//! corresponding component / scene / guard token streams.

use syn::{
    Ident, LitStr, Token, braced, bracketed,
    parse::{self, Parse},
    punctuated::Punctuated,
    spanned::Spanned,
    token::{self},
};

#[cfg(feature = "hybrid")]
use syn::Expr;

use crate::{
    action_id::ActionId,
    guard_condition::GuardCondition,
    kw::{self},
    machine_config::ConfigFn,
};

/// Parsed content of a `#[state(...)]` attribute.
///
/// Each field corresponds to one named parameter inside the attribute.
/// After parsing, call [`StateConfig::from_attrs`] to collect one or more
/// `#[state]` attributes into a single config.
#[derive(Debug, Default)]
pub(crate) struct StateConfig {
    pub(crate) guard_enter: Option<GuardCondition>,
    pub(crate) guard_exit: Option<GuardCondition>,
    before_enter: Option<ActionId>,
    after_exit: Option<ActionId>,
    on_update: Option<LitStr>,
    after_enter: Option<ActionId>,
    before_exit: Option<ActionId>,
    pub(crate) strategy: Option<Ident>,
    pub(crate) behavior: Option<Ident>,
    pub(crate) scene: Option<StateScene>,
    #[cfg(feature = "hybrid")]
    pub(crate) fsm_blueprint: Option<Expr>,
    pub(crate) minimal: bool,
}

impl StateConfig {
    /// Whether any FSM-relevant action fields are set.
    #[cfg(feature = "fsm")]
    pub fn is_fsm_any(&self) -> bool {
        self.before_enter.is_some()
            || self.after_exit.is_some()
            || self.on_update.is_some()
            || self.after_enter.is_some()
            || self.before_exit.is_some()
    }

    /// Whether any HSM-relevant action/guard/strategy fields are set.
    #[cfg(feature = "hsm")]
    pub fn is_hsm_any(&self) -> bool {
        #[cfg(feature = "hybrid")]
        if self.fsm_blueprint.is_some() {
            return true;
        }

        self.guard_enter.is_some()
            || self.guard_exit.is_some()
            || self.before_enter.is_some()
            || self.after_exit.is_some()
            || self.on_update.is_some()
            || self.after_enter.is_some()
            || self.before_exit.is_some()
            || self.strategy.is_some()
            || self.behavior.is_some()
    }

    #[cfg(feature = "hsm")]
    pub fn is_default_hsm_state(&self) -> bool {
        #[cfg(feature = "hybrid")]
        if self.fsm_blueprint.is_some() {
            return false;
        }
        self.strategy.is_none() && self.behavior.is_none()
    }

    #[cfg(feature = "hsm")]
    pub(super) fn hsm_state_token_stream(&self) -> proc_macro2::TokenStream {
        if self.is_default_hsm_state() {
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

    #[cfg(feature = "fsm")]
    pub(crate) fn fsm_state_token_stream(&self) -> proc_macro2::TokenStream {
        quote::quote! {FsmState::default(),}
    }

    pub(crate) fn to_actions(&self, actions: &mut Vec<(LitStr, ConfigFn)>) {
        if let Some(enter) = &self.after_enter
            && let Some(action) = enter.to_action()
        {
            actions.push(action);
        }
        if let Some(exit) = &self.before_exit
            && let Some(action) = exit.to_action()
        {
            actions.push(action);
        }
    }

    pub(crate) fn to_transitions(&self, actions: &mut Vec<(LitStr, ConfigFn)>) {
        if let Some(enter) = &self.before_enter
            && let Some(action) = enter.to_action()
        {
            actions.push(action);
        }
        if let Some(exit) = &self.after_exit
            && let Some(action) = exit.to_action()
        {
            actions.push(action);
        }
    }

    /// Builds a [`StateConfig`] by parsing all `#[state(...)]` attributes
    /// from a set of outer attributes.
    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut config: StateConfig = Self::default();
        for attr in attrs {
            if !attr.path().is_ident("state") {
                continue;
            }
            if matches!(attr.meta, syn::Meta::Path(_)) {
                continue;
            }
            let parsed_attrs =
                attr.parse_args_with(Punctuated::<StateAttrType, Token![,]>::parse_terminated)?;

            for state_attr in parsed_attrs {
                match state_attr {
                    StateAttrType::GuardEnter(guard) => {
                        if config.guard_enter.is_some() {
                            return Err(syn::Error::new(
                                guard.span(),
                                "duplicate `guard_enter` attribute",
                            ));
                        }
                        config.guard_enter = Some(guard);
                    }
                    StateAttrType::GuardExit(guard) => {
                        if config.guard_exit.is_some() {
                            return Err(syn::Error::new(
                                guard.span(),
                                "duplicate `guard_exit` attribute",
                            ));
                        }
                        config.guard_exit = Some(guard);
                    }
                    StateAttrType::BeforeEnter(enter) => {
                        if config.before_enter.is_some() {
                            return Err(syn::Error::new(
                                enter.span(),
                                "duplicate `before_enter` attribute",
                            ));
                        }
                        config.before_enter = Some(enter);
                    }
                    StateAttrType::AfterExit(exit) => {
                        if config.after_exit.is_some() {
                            return Err(syn::Error::new(
                                exit.span(),
                                "duplicate `after_exit` attribute",
                            ));
                        }
                        config.after_exit = Some(exit);
                    }
                    StateAttrType::OnUpdate(update) => {
                        if config.on_update.is_some() {
                            return Err(syn::Error::new(
                                update.span(),
                                "duplicate `on_update` attribute",
                            ));
                        }
                        config.on_update = Some(update);
                    }
                    StateAttrType::AfterEnter(enter) => {
                        if config.after_enter.is_some() {
                            return Err(syn::Error::new(
                                enter.span(),
                                "duplicate `after_enter` attribute",
                            ));
                        }
                        config.after_enter = Some(enter);
                    }
                    StateAttrType::BeforeExit(exit) => {
                        if config.before_exit.is_some() {
                            return Err(syn::Error::new(
                                exit.span(),
                                "duplicate `before_exit` attribute",
                            ));
                        }
                        config.before_exit = Some(exit);
                    }
                    StateAttrType::Strategy(strategy) => {
                        if config.strategy.is_some() {
                            return Err(syn::Error::new(
                                strategy.span(),
                                "duplicate `strategy` attribute",
                            ));
                        }
                        config.strategy = Some(strategy);
                    }
                    StateAttrType::Behavior(behavior) => {
                        if config.behavior.is_some() {
                            return Err(syn::Error::new(
                                behavior.span(),
                                "duplicate `behavior` attribute",
                            ));
                        }
                        config.behavior = Some(behavior);
                    }
                    StateAttrType::StateScene(scene) => {
                        if config.scene.is_some() {
                            return Err(syn::Error::new(
                                scene.span(),
                                "duplicate `scene` attribute",
                            ));
                        }
                        config.scene = Some(scene);
                    }
                    #[cfg(feature = "hybrid")]
                    StateAttrType::FsmBlueprint(fsm_blueprint) => {
                        if config.fsm_blueprint.is_some() {
                            return Err(syn::Error::new(
                                fsm_blueprint.span(),
                                "duplicate `fsm_blueprint` attribute",
                            ));
                        }
                        config.fsm_blueprint = Some(fsm_blueprint);
                    }
                    StateAttrType::Minimal(span) => {
                        if config.minimal {
                            return Err(syn::Error::new(span, "duplicate `minimal` attribute"));
                        }
                        config.minimal = true;
                    }
                }
            }
        }

        // Validate strategy / behavior enum values per EBNF
        if let Some(ref strategy) = config.strategy {
            let s = strategy.to_string();
            if s != "Nested" && s != "Parallel" {
                return Err(syn::Error::new_spanned(
                    strategy,
                    format!("unknown transition strategy `{s}`; expected `Nested` or `Parallel`"),
                ));
            }
        }
        if let Some(ref behavior) = config.behavior {
            let b = behavior.to_string();
            if b != "Rebirth" && b != "Resurrection" && b != "Death" {
                return Err(syn::Error::new_spanned(
                    behavior,
                    format!(
                        "unknown exit behavior `{b}`; expected `Rebirth`, `Resurrection`, or `Death`"
                    ),
                ));
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
            before_enter,
            after_exit,
            on_update,
            after_enter,
            before_exit,
            ..
        } = self;
        if let Some(guard_enter) = guard_enter {
            tokens.extend(quote::quote! {GuardEnter(#guard_enter),});
        }
        if let Some(guard_exit) = guard_exit {
            tokens.extend(quote::quote! {GuardExit(#guard_exit),});
        }
        if let Some(before_enter) = before_enter {
            tokens.extend(quote::quote! {BeforeEnterSystem::new(#before_enter),});
        }
        if let Some(after_exit) = after_exit {
            tokens.extend(quote::quote! {AfterExitSystem::new(#after_exit),});
        }
        if let Some(on_update) = on_update {
            tokens.extend(quote::quote! {OnUpdateSystem::new(#on_update),});
        }
        if let Some(after_enter) = after_enter {
            tokens.extend(quote::quote! {AfterEnterSystem::new(#after_enter),});
        }
        if let Some(before_exit) = before_exit {
            tokens.extend(quote::quote! {BeforeExitSystem::new(#before_exit),});
        }
    }
}

/// A `state_scene = bsn!{ ... }` expression parsed from `#[state(...)]`.
///
/// Generates a call to `world.create_state_scene_patch(...)` when the
/// `state_data` feature is active; emits a compile error otherwise.
#[derive(Debug)]
pub struct StateScene {
    #[allow(dead_code)]
    bsn: ExprBsn,
    #[allow(dead_code)]
    span: proc_macro2::Span,
}

impl Parse for StateScene {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let span = input.span();
        let bsn = input.parse::<ExprBsn>()?;
        Ok(Self { bsn, span })
    }
}

impl quote::ToTokens for StateScene {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        #[cfg(not(feature = "state_data"))]
        {
            tokens.extend(
                syn::Error::new(
                    self.span,
                    "`state_scene` requires the `state_data` feature to be enabled",
                )
                .into_compile_error(),
            );
        }
        #[cfg(feature = "state_data")]
        {
            let bsn = &self.bsn;
            tokens.extend(quote::quote! {world.create_state_scene_patch(#bsn).unwrap()});
        }
    }
}

/// Wraps either a `bsn!{ ... }` or `bsn_list![ ... ]` scene expression.
#[derive(Debug)]
pub struct ExprBsn {
    is_list: bool,
    scene: proc_macro2::TokenStream,
}

impl Parse for ExprBsn {
    fn parse(input: parse::ParseStream) -> syn::Result<Self> {
        let content;
        let lookahead = input.lookahead1();
        let is_list = if lookahead.peek(token::Bracket) {
            bracketed!(content in input);
            true
        } else if lookahead.peek(token::Brace) {
            braced!(content in input);
            false
        } else {
            return Err(lookahead.error());
        };
        let scene = proc_macro2::TokenStream::parse(&content)?;
        Ok(Self { is_list, scene })
    }
}

impl quote::ToTokens for ExprBsn {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ExprBsn { is_list, scene } = self;
        if *is_list {
            tokens.extend(quote::quote! {bsn_list!(#scene)});
        } else {
            tokens.extend(quote::quote! {bsn!(#scene)});
        }
    }
}

enum StateAttrType {
    GuardEnter(GuardCondition),
    GuardExit(GuardCondition),
    BeforeEnter(ActionId),
    AfterExit(ActionId),
    OnUpdate(LitStr),
    AfterEnter(ActionId),
    BeforeExit(ActionId),
    Strategy(Ident),
    Behavior(Ident),
    StateScene(StateScene),
    #[cfg(feature = "hybrid")]
    FsmBlueprint(Expr),
    Minimal(proc_macro2::Span),
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
            let minimal = input.parse::<kw::minimal>()?;
            Ok(StateAttrType::Minimal(minimal.span()))
        } else if lookahead.peek(kw::guard_enter) {
            Ok(StateAttrType::GuardEnter(parse_attr::<
                kw::guard_enter,
                GuardCondition,
            >(&input)?))
        } else if lookahead.peek(kw::guard_exit) {
            Ok(StateAttrType::GuardExit(parse_attr::<
                kw::guard_exit,
                GuardCondition,
            >(&input)?))
        } else if lookahead.peek(kw::before_enter) {
            Ok(StateAttrType::BeforeEnter(parse_attr::<
                kw::before_enter,
                ActionId,
            >(&input)?))
        } else if lookahead.peek(kw::after_enter) {
            Ok(StateAttrType::AfterEnter(parse_attr::<
                kw::after_enter,
                ActionId,
            >(&input)?))
        } else if lookahead.peek(kw::on_update) {
            Ok(StateAttrType::OnUpdate(
                parse_attr::<kw::on_update, LitStr>(&input)?,
            ))
        } else if lookahead.peek(kw::before_exit) {
            Ok(StateAttrType::BeforeExit(parse_attr::<
                kw::before_exit,
                ActionId,
            >(&input)?))
        } else if lookahead.peek(kw::after_exit) {
            Ok(StateAttrType::AfterExit(parse_attr::<
                kw::after_exit,
                ActionId,
            >(&input)?))
        } else if lookahead.peek(kw::strategy) {
            Ok(StateAttrType::Strategy(parse_attr::<kw::strategy, Ident>(
                &input,
            )?))
        } else if lookahead.peek(kw::behavior) {
            Ok(StateAttrType::Behavior(parse_attr::<kw::behavior, Ident>(
                &input,
            )?))
        } else if lookahead.peek(kw::state_scene) {
            Ok(StateAttrType::StateScene(parse_attr::<
                kw::state_scene,
                StateScene,
            >(&input)?))
        } else {
            #[cfg(feature = "hybrid")]
            {
                if lookahead.peek(kw::fsm_blueprint) {
                    return Ok(StateAttrType::FsmBlueprint(parse_attr::<
                        kw::fsm_blueprint,
                        Expr,
                    >(&input)?));
                }
            }

            Err(lookahead.error())
        }
    }
}
