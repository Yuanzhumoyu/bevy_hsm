# Bevy HSM - A Hybrid State Machine System

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[中文](../../README.md)

A powerful, hybrid state machine system designed for the [Bevy Game Engine](https://bevyengine.org/). It seamlessly integrates a **Hierarchical State Machine (HSM)** and a **Finite State Machine (FSM)**, allowing you to choose the best tool for different scenarios.

- Use the **HSM** to manage complex, high-level behavioral states in your application, where states have their own lifecycles (enter, update, exit).
- Use the **FSM** to manage simpler, event-driven sub-states within a specific hierarchical state.

## Features

- **Hybrid Model**: Supports both HSM and FSM within a unified framework.
- **State Lifecycles**: Supports `OnEnter`, `OnUpdate`, and `OnExit` lifecycle stages for states, which can be associated with independent Bevy systems.
- **Hierarchical Structure**: Supports state nesting (parent and child states) for logic reuse and composition.
- **Flexible Transition Triggers**:
  - **HSM**: Automatically triggers transitions through composable **condition systems** (`EnterGuard`, `ExitGuard`), or precisely controls them by sending **events** (`HsmTrigger`).
  - **FSM**: Precisely controls transitions by sending **events** (`FsmTrigger`).
- **Advanced Transition Control (HSM)**:
  - **Transition Strategy(`StateTransitionStrategy`)**: Configurable behavior for parent-child state transitions.
    - `Nested`: The parent state remains active while the child state executes its lifecycle within the parent.
    - `Parallel` The parent state exits before the child state enters, separating their lifecycles.
  - **Return Behavior(`ExitTransitionBehavior`)**: Configurable behavior for the parent state after a child state returns.
    - `Rebirth`: Triggers the parent state's OnEnter.
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

- `OnEnterSystem` / `OnUpdateSystem` / `OnExitSystem`: Systems that execute when a state is entered, updated, and exited, respectively.
- `GuardRegistry`: A resource for registering and managing all condition systems.
- `Paused`: A marker component to temporarily "pause" a state machine, making it unresponsive to any transitions.
- `Terminated`: A marker component indicating that the state machine has finished its execution.

### Hierarchical State Machine (HSM) - State-Driven

The HSM is driven by its internal state, making it ideal for managing complex behaviors with lifecycles.

- `HsmStateMachine`: The core component of the HSM, managing the current state, transition queue, and history.
- `StateLifecycle`: **The state-driven engine of the HSM**. This special component's value (`Enter`, `Update`, `Exit`) determines the current lifecycle stage of the state machine and drives all logic through its `on_insert` hook.
- `HsmTrigger`: **The event-driven engine of the HSM**. This is a Bevy event; sending it triggers an HSM state transition, providing an imperative way of control.
- `StateTree`: Defines the parent-child hierarchical relationships between states.
- `EnterGuard` / `ExitGuard`: Components attached to state entities to specify the conditions for entering or exiting that state.

### Finite State Machine (FSM) - Event-Driven

The FSM is driven by external events, making it ideal for responsive, direct state switching.

- `FsmStateMachine`: The core component of the FSM, managing the current state and graph.
- `FsmTrigger`: **The engine of the FSM**. This is a Bevy event; sending it triggers an FSM state transition.
- `FsmGraph`: Defines all valid transition paths within an FSM. A transition must be defined in the graph to be executed.
- `StateEvent`: A trait that allows you to use any custom type (struct, enum, integer, etc.) as a specific event to trigger FSM transitions.

## Macro Syntax (EBNF)

### `hsm!`

The `hsm!` macro is used to build a Hierarchical State Machine. It defines a tree structure with a single root state and optional additional Bevy components attached to the state machine entity.

```ebnf
hsm ::= state_node, { ',', component }, [ ',', ':', config_fn ];
state_node ::= { state_attribute }, [ ':', state_name ], [ '(', { state_content }, ')' ];
state_content ::= ( state_node | component ), { ',', ( state_node | component ) };
state_attribute ::= '#[state', [ '(', state_attribute_param, { ',', state_attribute_param }, ')' ], ']'
                  | '#[state_data(', component, { ',', component }, ')]';
state_attribute_param ::= 'enter_guard' '=' guard_condition
                        | 'exit_guard' '=' guard_condition
                        | 'on_update' '=' string_literal
                        | 'on_enter' '=' string_literal
                        | 'on_exit' '=' string_literal
                        | 'strategy' '=' ( 'Nested' | 'Parallel' )
                        | 'behavior' '=' ( 'Rebirth' | 'Resurrection' | 'Death' )
                        | 'fsm_blueprint' '=' rust_expression
                        | 'minimal';
config_fn ::= expr_closure
            | fn_identifier;
guard_condition ::= rust_expression; (* Any Rust expression that returns a bool *)
component ::= rust_expression; (* Any valid Bevy component *)
state_name ::= identifier; (* The name of the state *)
identifier ::= (* A Rust identifier, e.g., MyState, StateA *) ;
string_literal ::= (* A Rust string literal, e.g., "my_system" *) ;
rust_expression ::= (* Any valid Rust expression *) ;
expr_closure ::= (* A Rust closure expression *,e.g., |entity_commands: EntityCommands, states: &[Entity]| { ... } *) ;
fn_identifier ::= (* A Rust function identifier, e.g., my_function *, parameter type `fn (EntityCommands, &[Entity]){ ... }` *) ;
```

**Key Points**:
  
- The core of the `hsm!` macro is a single `state_node`, representing the root of the state tree.
- After the root state, you can append any number of Bevy `component`s, which will be added to the same entity as the state machine.
- A `state_node` can be configured with `#[state(...)]` attributes to set guards, lifecycle hooks (`on_update`, etc.), and hierarchical behavior (`strategy`, `behavior`).
- The `#[state_data(...)]` attribute is used to attach components that exist only when that state is active.
- States can be nested. Child states and components are defined within the `()` of the parent state.

### `fsm!`

The `fsm!` macro is used to build a flat Finite State Machine. It defines a set of states, a set of transition rules, and optional additional components.

```ebnf
fsm ::= fsm_graph, [ ',', 'components', ':', '{', [ component, { ',', component } ], '}' ],[',', ':', config_fn] ,[','];
fsm_graph ::= 'states', [ '<', state_ref, '>' ], ':', '{', state_definition, { ',', state_definition }, '}', [','],
              'transitions', ':', '{', transition, { ',', transition }, '}';
state_definition ::= { state_attribute }, [ ':', state_name ], [ '(', { component }, ')' ];
transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref [ ':', transition_condition ];
transition_condition ::= 'event', '(', rust_expression ')' (* Event *)
                       | 'guard', '(', guard_expression ')'; (* Conditional Guard *)
state_ref ::= identifier | integer_literal; (* State name or index *)
(* Definitions for `state_attribute`, `component`, `state_name`, `identifier`, `string_literal`, `rust_expression`, `config_fn` are the same as in the hsm! macro. *)
```

**Key Points**:

- The `fsm!` macro consists of two parts: the `fsm_graph` and an optional `components` block.
- The `fsm_graph` is required and contains both a `states` and a `transitions` block.
- The `states<...>` syntax allows you to specify the initial state by name or index (`state_ref`). If omitted, the first state in the list is the initial state.
- The syntax for `state_definition` is similar to `state_node` in `hsm!`, but it cannot contain nested states.
- A `transition` defines the rules for moving between states. It can be unconditional or conditional (via an event or a guard).
  - The arrows define the direction of the transition. There are three valid patterns:
    - A => B: A unidirectional transition from A to B.
    - A <= B: A unidirectional transition from B to A.
    - A <=> B: A bidirectional transition between A and B.
  - Note that the arrows on both sides of the transition condition must match.
  
### `hsm_tree!`

`hsm_tree!` is a utility macro for building a standalone state tree (`StateTree`). Its syntax is a subset of the `hsm!` macro, accepting only a single root `state_node`.

```ebnf
hsm_tree ::= state_node;
 
(* The definition of `state_node` is identical to the one in the `hsm!` macro. *)
```

### `fsm_graph!`

`fsm_graph!` is a utility macro for building a standalone state graph (`FsmGraph`). Its syntax is a subset of the `fsm!` macro.

```ebnf
fsm_graph ::= 'states', [ '<', state_ref, '>' ], ':', '{', state_definition, { ',', state_definition }, '}', ',',
              'transitions', ':', '{', transition, { ',', transition }, '}';
 
(* The definitions for `state_ref`, `state_definition`, and `transition` are identical to those in the `fsm!` macro. *)
```

### `combination_condition!`

`combination_condition!` is used to construct complex, combinable guard conditions within the `#[state]` attribute.

```ebnf
combination_condition ::= guard_expression;
 
guard_expression ::= 'and', '(', guard_expression, ',', guard_expression, { ',', guard_expression }, ')'
                   | 'or', '(', guard_expression, ',', guard_expression, { ',', guard_expression }, ')'
                   | 'not', '(', guard_expression, ')'
                   | guard_id;
guard_id ::= LitStr
           | '#' identifier
```

## Cargo Features

This crate provides several conditional compilation features:

- **`history`**: Enables state history tracking for both `FsmStateMachine` and `HsmStateMachine`. This allows you to see the sequence of states that have been active.
- **`state_data`**: Enables the `StateData` feature. This allows you to attach components as "state-local data" to a state. When the state machine enters that state, these components are automatically cloned to the state machine entity and are removed upon exit.
- **`hybrid`**: Enables hybrid state machine functionality, supporting both HSM and FSM.
- **`hsm`**: Enables HSM functionality.
- **`fsm`**: Enables FSM functionality.

To enable features, add them to your `Cargo.toml` file:

```toml
[dependencies]
bevy_hsm = { version = "0.18", features = ["history", "hyybrid"] }
```

## Epilogue

`bevy_hsm` is still under active development, and new features will continue to be added and improved. You are welcome to help improve this library by submitting Issues or Pull Requests.

## License

This project is licensed under either of

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
