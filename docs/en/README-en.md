# Bevy HSM - A Hybrid State Machine System

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[中文](../../README.md)

A powerful, hybrid state machine system designed for the [Bevy Game Engine](https://bevyengine.org/). It seamlessly integrates a **Hierarchical State Machine (HSM)** and a **Finite State Machine (FSM)**, allowing you to choose the best tool for different scenarios.

- Use the **HSM** to manage complex, high-level behavioral states in your application, where states have their own lifecycles (enter, update, exit).
- Use the **FSM** to manage simpler, event-driven sub-states within a specific hierarchical state.

## Features

- **Hybrid Model**: Supports both HSM and FSM within a unified framework.
- **State Lifecycles**: Supports `Enter`, `Update`, and `Exit` lifecycle stages for states, which can be associated with independent Bevy systems.
- **Hierarchical Structure**: Supports state nesting (parent and child states) for logic reuse and composition.
- **Flexible Transition Triggers**:
  - **HSM**: Automatically triggers transitions through composable **condition systems** (`GuardEnter`, `GuardExit`), or precisely controls them by sending **events** (`HsmTrigger`).
  - **FSM**: Precisely controls transitions by sending **events** (`FsmTrigger`).
- **Advanced Transition Control (HSM)**:
  - **Transition Strategy(`StateTransitionStrategy`)**: Configurable behavior for parent-child state transitions.
    - `Nested`: The parent state remains active while the child state executes its lifecycle within the parent.
    - `Parallel` The parent state exits before the child state enters, separating their lifecycles.
  - **Return Behavior(`ExitTransitionBehavior`)**: Configurable behavior for the parent state after a child state returns.
    - `Rebirth`: Triggers the parent state's AfterEnter.
    - `Resurrection`: Returns to the parent state's OnUpdate.
    - `Death`: Causes the parent state to exit as well, propagating the exit behavior up the hierarchy.
- **Bevy-Idiomatic**: The entire architecture follows Bevy's ECS paradigm, driven by components, events, and systems for seamless integration with the engine.
- **State History**: Built-in state transition history for easier debugging.

## Basic Usage

Add the `StateMachinePlugin` to your Bevy app:

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default())
        // ... register your states and systems here
        .run();
}
```

## Core Concepts

### Common Concepts

- `BeforeEnterSystem` / `AfterEnterSystem` / `OnUpdateSystem` / `BeforeExitSystem` / `AfterExitSystem`: Systems that execute before entering, after entering, during update, before exiting, and after exiting a state, respectively.
- `ActionRegistry` / `GuardRegistry` / `TransitionRegistry`: Resources for registering and managing all action, guard, and transition systems, respectively.
- `ActionContext` / `GuardContext` / `TransitionContext`: These are specialized system parameters used to provide contextual information about states and transitions in action, guard, and transition systems. For example, `ActionContext` provides the entity of the current state, while `GuardContext` provides the source and target states of the transition.
- `Paused`: A marker component to temporarily "pause" a state machine, making it unresponsive to any transitions.
- `Terminated`: A marker component indicating that the state machine has finished its execution.

### Hierarchical State Machine (HSM) - State-Driven

The HSM is driven by its internal state, making it ideal for managing complex behaviors with lifecycles. It supports two driving modes:

- **State-Driven (Automatic)**: Via the `StateLifecycle` component. This is a special component whose value (`Enter`, `Update`, `Exit`) determines the current lifecycle stage of the state machine and drives all logic through its `on_insert` hook. This mode is typically used for automatic transitions triggered by the state's own conditions.
- **Event-Driven (Manual)**: By sending an `HsmTrigger` event. This is a Bevy event that, when sent, forces an HSM state transition, providing imperative and precise control.
- `StateTree`: Defines the parent-child hierarchical relationships between states.
- `GuardEnter` / `GuardExit`: Components attached to state entities to specify the conditions for entering or exiting that state.

#### HSM Advanced Features

##### Transition Strategy (StateTransitionStrategy)

By setting the `strategy` in the `#[state]` attribute, you can control the behavior of child states when entering or exiting a parent state.

- **`Nested`** (Default): The parent state remains active, and the entry and exit of the child state occur within the parent state's lifecycle.
- **`Parallel`**: During a transition, the parent state will exit first, then the child state completes its lifecycle, after which the parent state may re-enter according to the `ExitTransitionBehavior`.

##### State Behavior (ExitTransitionBehavior)

With the `behavior` attribute, you can define how a parent state behaves after one of its child states exits.

- **`Rebirth`**: After exiting a child state, the parent state will re-execute its `Enter` phase.
- **`Resurrection`** (Default): After exiting a child state, the parent state will directly resume its `Update` phase.
- **`Death`**: After exiting a child state, the parent state itself will also exit, propagating the exit behavior upwards.

##### History State

HSMs support a history state feature. By setting `history_capacity` in the `init` section of the `hsm!` macro, the state machine can "remember" the most recently visited child state. When a parent state is re-entered, it can directly resume to the last active child state instead of its initial child state, which is very useful for implementing features like "back" navigation.

#### Plugin Configuration

##### Custom Scheduling

By default, the state machine systems run in the `Last` schedule. If you need finer control, you can specify that the state machine systems run in your custom schedule using `StateMachinePlugin::with_schedule(MySchedule)`.

```rust,ignore
use bevy::prelude::*;
use bevy_hsm::prelude::*;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct MyUpdate;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::with_schedule(MyUpdate))
        .run();
}
```

### Finite State Machine (FSM) - Event-Driven

The FSM is driven by external events, making it ideal for responsive, direct state switching.

- `FsmState`: A marker component used to identify an entity as an FSM state.
- `FsmStateMachine`: The core component of the FSM, managing the current state and graph.
- `FsmTrigger`: **The engine of the FSM**. This is a Bevy event used to drive FSM state transitions. You can use it to trigger unconditional transitions or to wrap a custom event (like an enum or struct) to trigger a specific event-driven transition.
- `FsmGraph`: Defines all valid transition paths within an FSM. A transition must be defined in the graph to be executed.

## Macro Syntax (EBNF)

### `hsm!`

The `hsm!` macro is used to build a Hierarchical State Machine. It defines a tree structure with a single root state and optional additional Bevy components attached to the state machine entity.

```ebnf
hsm ::= [ machine_config, ',', ], state_node, { ',', component }, [ ',', config_fn ];
machine_config ::= 'init', '(', [ machine_config_param, { ',', machine_config_param } ], ')';
machine_config_param ::= 'history_capacity', '=', integer_literal
                       | ( 'init_state' | 'curr_state' ), '=', state_ref;
state_node ::= state_attribute, [ ':', state_name ], [ '(', { state_content }, ')' ];
state_content ::= ( state_node | component ), { ',', ( state_node | component ) };
state_attribute ::= '#[state', [ '(', state_attribute_param, { ',', state_attribute_param }, ')' ], ']'
                  | '#[state_data(', component, { ',', component }, ')]';
state_attribute_param ::= ( 'guard_enter' | 'guard_exit' ), '=', guard_expression
                        | ( 'before_enter' | 'after_enter' | 'before_exit' | 'after_exit' ), '=', action_id
                        | 'on_update', '=', lit_str
                        | 'strategy', '=', ( 'Nested' | 'Parallel' )
                        | 'behavior', '=', ( 'Rebirth' | 'Resurrection' | 'Death' )
                        | 'fsm_blueprint', '=', rust_expression
                        | 'minimal';
config_fn ::= ':', ( expr_closure | fn_identifier | expr_call );
component ::= rust_expression; (* Any valid Bevy component *)
state_name ::= identifier; (* The name of the state *)
state_ref ::= identifier | integer_literal;
action_id ::= lit_str
            | fn_identifier
            | action_name, ':', ( expr_closure | expr_call | fn_identifier );
action_name ::= identifier;
identifier ::= (* A Rust identifier, e.g., MyState, StateA *) ;
lit_str ::= (* A Rust string literal, e.g., "my_system" *) ;
rust_expression ::= (* Any valid Rust expression *) ;
expr_closure ::= (* A Rust closure expression *,e.g., |entity_commands: EntityCommands, states: &[Entity]| { ... } *) ;
fn_identifier ::= (* A Rust function identifier, e.g., my_function *, signature must be `fn(EntityCommands, &[Entity])` *) ;
expr_call ::= (* Any valid Rust function call expression, e.g., my_function(a, b) *) ;
```

**Key Points**:

- The core of the `hsm!` macro is a single `state_node`, representing the root of the state tree.
- After the root state, you can append any number of Bevy `component`s, which will be added to the same entity as the state machine.
- The `state_node` can be configured with the `#[state(...)]` attribute. In addition to common lifecycle hooks (like `on_update`, `after_enter`), it supports HSM-exclusive attributes, including guards for automatic transitions (`guard_enter`, `guard_exit`) and properties for controlling hierarchical behavior like `strategy` and `behavior`.
- The `#[state_data(...)]` attribute is used to attach components that exist only when that state is active.
- States can be nested. Child states and components are defined within the `()` of the parent state.

### `fsm!`

The `fsm!` macro is used to build a flat Finite State Machine. It defines a set of states, a set of transition rules, and optional additional components.

```ebnf
fsm ::= [ machine_config, ',', ], fsm_graph, [ ',', 'components', ':', '{', [ component, { ',', component } ], '}' ], [ ',', config_fn ];
fsm_graph ::= 'states', ':', '{', state_definition, { ',', state_definition }, '}', ',',
              'transitions', ':', '{', transition, { ',', transition }, '}';
state_definition ::= state_attribute, [ ':', state_name ], [ '(', { component }, ')' ];
transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref, [ ':', transition_condition ];
transition_condition ::= 'event', '(', rust_expression ')' (* Event *)
                       | 'guard', '(', guard_expression ')'; (* Conditional Guard *)
state_ref ::= identifier | integer_literal; (* State name or index *)
(* Definitions for `state_attribute`, `component`, `state_name`, `identifier`, `lit_str`, `rust_expression`, `config_fn`, `action_id`, `machine_config`, `state_ref`, `fsm_graph` are the same as in the hsm! macro. *)
```

**Key Points**:

- The `fsm!` macro consists of three parts: the `fsm_graph`, an optional `components` block, and an optional `config_fn`.
- The `fsm_graph` is required and contains both a `states` and a `transitions` block.
- The syntax for `state_definition` is similar to `state_node` in `hsm!`, but it cannot contain nested states.
- `state_definition` also supports `#[state(...)]` and `#[state_data(...)]` attributes. However, please note that because FSMs have a flat, event-driven structure, parameters in `#[state(...)]` related to HSM's automatic transitions and hierarchy (like `guard_enter`, `guard_exit`, `strategy`, `behavior`) are invalid here.
- A `transition` defines the rules for moving between states. It can be unconditional or conditional (via an event or a guard).
  - The arrows define the direction of the transition. There are three valid patterns:
    - A => B: A unidirectional transition from A to B.
    - A <= B: A unidirectional transition from B to A.
    - A <=> B: A bidirectional transition between A and B.
  - Note that the arrows on both sides of the transition condition must match.
  
### `hsm_tree!`

`hsm_tree!` is a utility macro for building a standalone state tree (`StateTree`). Its syntax is a subset of the `hsm!` macro, accepting only a single root `state_node`.

```ebnf
hsm_tree ::= state_node, [ ',', config_fn ];
 
(* The definitions for `state_node` and `config_fn` are identical to those in the `hsm!` macro. *)
```

### `fsm_graph!`

`fsm_graph!` is a utility macro for building a standalone state graph (`FsmGraph`). Its syntax is a subset of the `fsm!` macro.

```ebnf
fsm_graph! ::= fsm_graph, [ ',', config_fn ];
 
(* The definitions for `fsm_graph` and `config_fn` are identical to those in the `fsm!` macro. *)
```

### `system_registry!`

`system_registry!` is a helper macro for dynamically registering multiple Bevy systems into a `SystemRegistry` resource. This is useful when you need to pass a collection of related systems (e.g., as state actions) to a state machine.

```ebnf
system_registry ::= '< ', source, ',', system_registry, '>', '[', [ system_definition, { ',', system_definition } ], ']';
system_definition ::= ( lit_str | fn_identifier ), '=>', rust_expression;

source ::= identifier; (* A variable of type `Commands` or `World` *)
system_registry ::= identifier; (* A variable that implements `Extend<(String, SystemId)>` *)
lit_str ::= (* A unique name within the system_registry *)
fn_identifier ::= (* A unique name within the system_registry *)
rust_expression ::= (* A Bevy system (function or closure) *)
```

**Example**:

```rust
let mut system_registry = SystemRegistry::new();
system_registry!(<commands, system_registry>[
    "on_enter_a" => on_enter_a,
    "on_update_a" => || info!("Updating A"),
]);
```

### `combination_condition!`

`combination_condition!` is used to construct complex, combinable guard conditions within the `#[state]` attribute.

```ebnf
combination_condition ::= guard_expression;
 
guard_expression ::= ( 'and' | 'or' ), '(', guard_expression, ',', guard_expression, { ',', guard_expression }, ')'
                   | 'not', '(', guard_expression, ')'
                   | guard_id;
guard_id ::= lit_str | ( '#', identifier );
```

## Cargo Features

This crate provides the following Cargo features:

- **`hsm`** (enabled by default): Enables Hierarchical State Machine (HSM) functionality.
- **`fsm`** (enabled by default): Enables Finite State Machine (FSM) functionality.
- **`hybrid`**: A convenience feature that enables both `hsm` and `fsm`.
- **`history`**: Enables history tracking for state machines, allowing you to trace the sequence of state transitions.
- **`state_data`**: Enables the `StateData` feature, allowing you to attach components as "state-local data" to a state.

By default, `hybrid`, `history`, and `state_data` are all enabled. If you want to configure them yourself, you can do so like this:

```toml
[dependencies]
bevy_hsm = { version = "0.18", default-features = false, features = ["history", "hybrid"] }
```

## Epilogue

`bevy_hsm` is still under active development, and new features will continue to be added and improved. You are welcome to help improve this library by submitting Issues or Pull Requests.

## License

This project is licensed under either of

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
