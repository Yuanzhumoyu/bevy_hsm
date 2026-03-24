extern crate proc_macro;

mod fsm;
mod fsm_graph;
mod guard_condition;
mod hsm;
mod hsm_tree;
mod kw;
mod state_config;
mod action_id;
mod machine_config;

use proc_macro::TokenStream;

/// Builds a complex combination guard condition for state transitions.
///
/// This macro allows you to create nested logical conditions using `and`, `or`, and `not` operators.
/// It is used within the `#[state]` attribute to define `guard_enter` or `guard_exit` conditions.
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
///         #[state(guard_enter = enter_condition)]: Initial
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
/// ```ebnf
/// hsm ::= [ machine_config, ',', ], state_node, { ',', component }, [ ',', config_fn ];
/// machine_config ::= 'init', '(', [ machine_config_param, { ',', machine_config_param } ], ')';
/// machine_config_param ::= 'history_capacity', '=', integer_literal
///                        | ( 'init_state' | 'curr_state' ), '=', state_ref;
/// state_node ::= { state_attribute }, [ ':', state_name ], [ '(', { state_content }, ')' ];
/// state_content ::= ( state_node | component ), { ',', ( state_node | component ) };
/// state_attribute ::= '#[state', [ '(', state_attribute_param, { ',', state_attribute_param }, ')' ], ']' 
///                   | '#[state_data(', component, { ',', component }, ')]';
/// state_attribute_param ::= ( 'guard_enter' | 'guard_exit' ), '=', guard_expression
///                         | ( 'on_update' | 'on_enter' | 'on_exit' ), '=', action_id
///                         | 'strategy', '=', ( 'Nested' | 'Parallel' )
///                         | 'behavior', '=', ( 'Rebirth' | 'Resurrection' | 'Death' )
///                         | 'fsm_blueprint', '=', rust_expression
///                         | 'minimal';
/// config_fn ::= ':', ( expr_closure | fn_identifier | expr_call );
/// component ::= rust_expression; (* Any valid Bevy component *)
/// state_name ::= identifier; (* The name of the state *)
/// state_ref ::= identifier | integer_literal;
/// action_id ::= lit_str
///             | fn_identifier
///             | action_name, ':', ( expr_closure | expr_call | fn_identifier );
/// action_name ::= identifier;
/// identifier ::= (* Rust identifier, e.g., MyState, StateA *);
/// lit_str ::= (* Rust string literal, e.g., "my_system" *);
/// rust_expression ::= (* Any valid Rust expression *);
/// expr_closure ::= (* Rust closure, e.g., |entity_commands: EntityCommands, states: &[Entity]| { ... } *);
/// fn_identifier ::= (* Rust function identifier, e.g., my_function *, with signature `fn(EntityCommands, &[Entity]){ ... }` *);
/// expr_call ::= (* Any valid Rust function call expression, e.g., my_function(a, b) *);
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
///         #[state(on_enter=on_enter:on_enter_a, on_exit=on_exit_a)]: A(
///             #[state(on_enter="on_enter_b", on_exit="on_exit_b")]: B
///         ),
///         Name::new("MyHSM")
///     ));
/// }
/// ```
#[proc_macro]
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
/// hsm_tree ::= state_node, [ ',', config_fn ];
///
/// (* The definitions of `state_node` and `config_fn` are identical to those in the `hsm!` macro. *)
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
/// fsm ::= [ machine_config, ',', ], fsm_graph, [ ',', 'components', ':', '{', [ component, { ',', component } ], '}' ], [ ',', config_fn ];
/// fsm_graph ::= 'states', ':', '{', state_definition, { ',', state_definition }, '}', 
///               'transitions', ':', '{', transition, { ',', transition }, '}';
/// state_definition ::= { state_attribute }, [ ':', state_name ], [ '(', { component }, ')' ];
/// transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref, [ ':', transition_condition ];
/// transition_condition ::= 'event', '(', rust_expression ')' (* Event *)
///                        | 'guard', '(', guard_expression ')'; (* Guard condition *)
/// (* The definitions of `machine_config`, `state_ref`, `state_attribute`, `component`, `state_name`, `identifier`, `lit_str`, `rust_expression`, `config_fn`, `action_id` are the same as in the hsm! macro *)
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
pub fn fsm(item: TokenStream) -> TokenStream {
    fsm::fsm_impl(item)
}

/// Builds an `FsmGraph` component for a Finite State Machine.
///
/// This is a utility macro that is a subset of the `fsm!` macro. It is used to construct
/// an `FsmGraph` component, which defines the states and valid transitions for an FSM.
///
/// # EBNF Syntax
///
/// ```ebnf
/// fsm_graph! ::= fsm_graph, [ ',', config_fn ];
/// 
/// (* The definitions for `fsm_graph` and `config_fn` are identical to those in the `fsm!` macro. *)
/// ```
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
///         states<A>: {
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
pub fn fsm_graph(item: TokenStream) -> TokenStream {
    fsm_graph::fsm_graph_impl(item)
}