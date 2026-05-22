// --- Always available ---
mod guard_condition;
mod kw;

// --- Available when either FSM or HSM is enabled ---
#[cfg(any(feature = "fsm", feature = "hsm"))]
mod action_id;
#[cfg(any(feature = "fsm", feature = "hsm"))]
mod machine_config;
#[cfg(any(feature = "fsm", feature = "hsm"))]
mod state_config;

// --- FSM-only modules ---
#[cfg(feature = "fsm")]
mod fsm;
#[cfg(feature = "fsm")]
mod fsm_graph;

// --- HSM-only modules ---
#[cfg(feature = "hsm")]
mod hsm;
#[cfg(feature = "hsm")]
mod hsm_tree;

use proc_macro::TokenStream;

/// Combines multiple guard conditions into a single complex condition for state transitions.
///
/// This macro simplifies the creation of complex guard logic by allowing you to create nested
/// logical conditions using `and`, `or`, and `not` operators. It is used within the `#[state]`
/// attribute to define `guard_enter` or `guard_exit` conditions.
///
/// # EBNF Syntax
///
/// ```ebnf
/// combination_condition ::= guard_expression;
///
/// guard_expression ::= ( 'and' | 'or' ), '(', guard_expression, ',', guard_expression, { ',', guard_expression }, ')'
///                    | 'not', '(', guard_expression, ')'
///                    | guard_id;
/// guard_id ::= lit_str | ( '#', identifier );
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// fn is_a(context: In<GuardContext>) -> bool { true }
/// fn is_b(context: In<GuardContext>) -> bool { false }
///
/// fn setup(mut commands: Commands, mut guard_registry: ResMut<GuardRegistry>) {
///     system_registry!(<commands, guard_registry>[
///         "is_a" => is_a,
///         "is_b" => is_b,
///     ]);
///     
///     let enter_condition = combination_condition!(and("is_a", not("is_b")));
///
///     commands.spawn(hsm!(
///         #[state(guard_enter = #enter_condition)]: Initial
///     ));
/// }
/// ```
#[proc_macro]
pub fn combination_condition(item: TokenStream) -> TokenStream {
    guard_condition::guard_condition_impl(item)
}

/// Builds a Hierarchical State Machine (HSM).
///
/// The `hsm!` macro defines a tree-like structure with a root state and optional
/// additional Bevy components attached to the state machine entity.
///
/// # EBNF Syntax
///
/// The syntax for the state machine macros shares many common definitions.
///
/// ```ebnf
/// (* ---- Common Definitions ---- *)
///
/// machine_config ::= 'init', '(', [ machine_config_param, { ',', machine_config_param } ], ')'
/// machine_config_param ::= ( 'init_state' | 'curr_state' ), '=', state_ref
///                        | 'history_capacity', '=', integer_literal (* "history" feature *)
///
/// config_fn ::= ':', ( fn_identifier | expr_closure | expr_call )
///
/// state_ref ::= identifier | integer_literal
///
/// component ::= (* Any valid Rust expression that resolves to a component *)
///
/// state_attribute ::= '#[state', [ '(', state_attribute_param, { ',', state_attribute_param }, ')' ], ']'
/// state_attribute_param ::= ( 'before_enter' | 'after_enter' | 'before_exit' | 'after_exit' ), '=', action_id
///                         | 'on_update', '=', lit_str
///                         | ( 'guard_enter' | 'guard_exit' ), '=', guard_expression
///                         | 'strategy', '=', ( 'Nested' | 'Parallel' )
///                         | 'behavior', '=', ( 'Rebirth' | 'Resurrection' | 'Death' )
///                         | 'state_scene', '=', expr_bsn
///                         | 'fsm_blueprint', '=', rust_expression (* "hybrid" feature *)
///                         | 'minimal'
///
/// action_id ::= lit_str
///             | fn_identifier
///             | action_name, ':', ( expr_closure | expr_call | fn_identifier )
///
/// guard_expression ::= (* See combination_condition! macro for details *)
///
/// expr_bsn ::= '{' bsn '}' | '[' bsn_list ']'
///
/// (* ---- hsm! Macro ---- *)
///
/// (*
///  * The hsm! macro is flexible. It requires one root state_node, and allows at most one
///  * machine_config and one config_fn. Components can be freely interspersed.
///  * A typical structure is shown below.
///  *)
/// hsm ::= [ machine_config, ',' ], state_node, { ',', component }, [ ',', config_fn ]
///
/// state_node ::= state_attribute, [ ':', identifier ], [ '(', { hsm_state_content }, ')' ]
/// hsm_state_content ::= ( state_node | component ), { ',', ( state_node | component ) }
///
/// (* ---- Shared Primitives ---- *)
///
/// fn_identifier ::= (* A Rust path to a function, e.g., my_function *)
/// expr_closure ::= (* A Rust closure, e.g., |...| { ... } *)
/// expr_call ::= (* A Rust function call, e.g., my_function(a, b) *)
/// action_name ::= identifier
/// identifier ::= (* A Rust identifier, e.g., MyState, StateA *)
/// integer_literal ::= (* A Rust integer literal, e.g., 0, 42 *)
/// lit_str ::= (* A Rust string literal, e.g., "my_system" *)
/// rust_expression ::= (* Any valid Rust expression *)
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// fn on_enter_a(context: In<ActionContext>) {
///     info!("Entering state A");
/// }
///
/// fn on_exit_a(context: In<ActionContext>) {
///     info!("Exiting state A");
/// }
///
/// fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
///     system_registry!(<commands, action_registry>[
///         "on_enter_a" => on_enter_a,
///         "on_exit_a" => on_exit_a,
///     ]);
///
///     commands.spawn(hsm!(
///         #[state(after_enter=on_enter_a, before_exit=on_exit_a)]: A(
///             #[state(after_enter="on_enter_b", before_exit="on_exit_b")]: B
///         ),
///         Name::new("MyHSM")
///     ));
/// }
/// ```
#[proc_macro]
#[cfg(feature = "hsm")]
pub fn hsm(item: TokenStream) -> TokenStream {
    hsm::hsm_impl(item)
}

/// Builds a `StateTree` component for a Hierarchical State Machine.
///
/// This is a utility macro that is a subset of the `hsm!` macro. It only accepts a single
/// root `state_node` and generates a `StateTree` component, which can be used to dynamically
/// build or modify state machines.
///
/// # EBNF Syntax
///
/// ```ebnf
/// hsm_tree ::= state_node, [ ',', config_fn ]
///
/// (* The definitions for `state_node` and `config_fn` are shared with the `hsm!` macro. *)
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// fn setup(mut commands: Commands) {
///     let state_tree = hsm_tree!(
///         #[state(strategy=Parallel)]: A(
///             #[state(strategy=Nested)]: B,
///             #[state(strategy=Nested)]: C,
///         )
///     );
///     commands.spawn(state_tree);
/// }
/// ```
#[proc_macro]
#[cfg(feature = "hsm")]
pub fn hsm_tree(item: TokenStream) -> TokenStream {
    hsm_tree::hsm_tree_impl(item)
}

/// Builds a flat Finite State Machine (FSM).
///
/// The `fsm!` macro defines a set of states, a set of transition rules, and optional
/// additional components.
///
/// # EBNF Syntax
///
/// ```ebnf
/// fsm ::= [ machine_config, ',' ], fsm_graph_content, [ ',', components_block ], [ ',', config_fn ]
///
/// components_block ::= 'components', ':', '{', [ component, { ',', component } ], '}'
///
/// (* `machine_config`, `config_fn`, and `component` are shared with the `hsm!` macro. *)
/// (* `fsm_graph_content` is defined below. *)
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// #[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
/// struct MyEvent;
///
/// fn setup(mut commands: Commands) {
///     commands.spawn(fsm!(
///         states: {
///             #[state]: A,
///             #[state]: B,
///         },
///         transitions: {
///             A <=> B: event(MyEvent)
///         },
///         components: {
///             Name::new("MyFSM")
///         }
///     ));
/// }
/// ```
#[proc_macro]
#[cfg(feature = "fsm")]
pub fn fsm(item: TokenStream) -> TokenStream {
    fsm::fsm_impl(item)
}

/// Builds an [`FsmGraph`] component for a Finite State Machine.
///
/// This is a utility macro that is a subset of the `fsm!` macro. It is used to construct
/// an [`FsmGraph`] component, which defines the states and valid transitions for an FSM.
///
/// # EBNF Syntax
///
/// ```ebnf
/// fsm_graph ::= fsm_graph_content, [ ',', config_fn ]
///
/// fsm_graph_content ::= 'states', ':', '{', [ fsm_state, { ',', fsm_state } ], '}',
///                       [ ',' ],
///                       'transitions', ':', '{', [ transition, { ',', transition } ], '}'
///
/// fsm_state ::= state_attribute, [ ':', identifier ], [ '(', { component }, ')' ]
///
/// transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref, [ ':', transition_condition ]
/// transition_condition ::= 'event', '(', rust_expression, ')'
///                        | 'guard', '(', guard_expression, ')'
///
/// (* Other definitions are shared with the `hsm!` macro. *)
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// #[derive(Event, PartialEq, Eq, Clone, Copy, Debug, Hash)]
/// struct MyEvent;
///
/// fn setup(mut commands: Commands) {
///     let graph = fsm_graph!(
///         states: {
///             #[state]: A,
///             #[state]: B
///         },
///         transitions: {
///             A => B: event(MyEvent)
///         }
///     );
///
///     commands.spawn(graph);
/// }
/// ```
#[proc_macro]
#[cfg(feature = "fsm")]
pub fn fsm_graph(item: TokenStream) -> TokenStream {
    fsm_graph::fsm_graph_impl(item)
}
