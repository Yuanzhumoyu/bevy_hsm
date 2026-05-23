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
- **Interrupt Mechanism**: Both HSM and FSM support an interrupt/resume mechanism. Save the current state via an interrupt, transition to a handler state to deal with urgent events, then resume back to the previously saved state. Supports nested interrupts via a stack-based design.
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

### Interrupt State Mechanism

Both HSM and FSM support an interrupt/resume mechanism, which allows the state machine to temporarily suspend its current state, jump to a designated handler state (potentially in a different state graph/tree) to process urgent events, and then return to the previously saved state and graph. This is similar to an "interrupt service routine" in hardware or operating systems.

Key features:

- **Stack-based**: Interrupted state graphs and states are stored on a stack, naturally supporting nested interrupts (interrupt within an interrupt).
- **Cross-graph interrupts**: Interrupts can target states in a different state graph/tree, enabling separation of normal behavior and error-handling logic.
- **Self-interrupt protection**: Interrupting to the currently active state and graph is a no-op.
- **Empty resume protection**: Calling resume when no interrupt has occurred is safely ignored.
- **Independent of other features**: The interrupt mechanism is part of the FSM/HSM core and does not depend on optional features like `history` or `state_data`.

**HSM Usage:**

```rust
// Save current state and jump to the interrupt handler state (in a different state tree)
commands.trigger(HsmTrigger::interrupt(hsm_entity, error_tree_entity, error_handler_state));

// After handling the urgent event, return to the previously saved state and tree
commands.trigger(HsmTrigger::resume(hsm_entity));
```

**FSM Usage:**

```rust
// Save current state and jump to the interrupt handler state (in a different graph)
commands.trigger(FsmTrigger::with_interrupt(fsm_entity, error_graph_entity, error_handler_state));

// After handling the urgent event, return to the previously saved state
commands.trigger(FsmTrigger::with_resume(fsm_entity));

// Query interrupt status
fn query_interrupt_status(query: Query<&FsmStateMachine>) {
    for sm in &query {
        if sm.is_interrupted() {
            println!("Interrupt depth: {}", sm.interrupt_depth());
        }
    }
}
```

The interrupt stack can be cleared at any time via `clear_interrupt_stack()`, which abandons all pending resumes and keeps the state machine in its current state.

**Two Design Philosophies: HSM vs. FSM**
At its core, `bevy_hsm` offers two state machines with distinct design philosophies:

- **HSM (Hierarchical State Machine)** uses an **asynchronous, plan-based** lifecycle model. A transition intent (triggered by a guard or event) is placed into a queue, and a system later executes a detailed transition plan asynchronously (typically in the `Last` schedule). This makes it ideal for managing complex, hierarchical behaviors that require robust entry/exit logic.
- **FSM (Finite State Machine)** uses a **synchronous, command-based** lifecycle model. A transition is triggered by an event and **immediately** completes all exit and entry actions synchronously within the event-handling function. This makes it very lightweight and fast, suitable for responsive, direct state switching.

Understanding the difference between these two modes is key to using this library effectively.

### Hierarchical State Machine (HSM) - State-Driven

The HSM is driven by its internal state, making it ideal for managing complex behaviors with lifecycles. Its lifecycle management is **asynchronous and plan-based**. It supports two driving modes:

- **State-Driven (Automatic)**: Via the `StateLifecycle` component. This is a special component whose value (`Enter`, `Update`, `Exit`) determines the current lifecycle stage. When a transition is needed, the system calculates a detailed **transition plan** (a series of enter and exit steps) and then drives the execution of this plan asynchronously by modifying the `StateLifecycle` value.
- **Event-Driven (Manual)**: By sending an `HsmTrigger` event. This is a Bevy event that, when sent, forces an HSM state transition. It also generates a transition plan, which is then driven by the `StateLifecycle`, providing an imperative and precise method of control. Supports `Interrupt` and `Resume` trigger types for saving/restoring state via the interrupt mechanism.
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

The FSM is driven entirely by external events, and its lifecycle management is **synchronous and command-based**. This makes it ideal for responsive, direct state switching.

- `FsmState`: A marker component used to identify an entity as an FSM state.
- `FsmStateMachine`: The core component of the FSM, managing the current state and graph.
- `FsmTrigger`: **The sole event engine of the FSM**. This is a Bevy event used to drive FSM state transitions. When the event is received, the state machine **immediately and synchronously** completes the entire process from exiting the old state to entering the new one. Supports multiple trigger types: `Next` (unconditional), `Guard` (condition-checked), `Event` (event-based), `Interrupt` (save state and jump to handler), and `Resume` (return to saved state).
- `FsmGraph`: Defines all valid transition paths within an FSM. A transition must be defined in the graph to be executed.

## Macro Syntax (EBNF)

To precisely define the structure of the macros, we use the EBNF notation.

### Common Definitions

These are the basic building blocks shared across multiple state machine macros.

```ebnf
(* ---- Common Definitions ---- *)

machine_config ::= 'init', '(', [ machine_config_param, { ',', machine_config_param } ], ')'
machine_config_param ::= ( 'init_state' | 'curr_state' ), '=', state_ref
                       | 'history_capacity', '=', integer_literal (* "history" feature *)

config_fn ::= ':', ( fn_identifier | expr_closure | expr_call )

state_ref ::= identifier | integer_literal

component ::= (* Any valid Rust expression that resolves to a component *)

state_attribute ::= '#[state', [ '(', state_attribute_param, { ',', state_attribute_param }, ')' ], ']'
state_attribute_param ::= ( 'before_enter' | 'after_enter' | 'before_exit' | 'after_exit' ), '=', action_id
                        | 'on_update', '=', lit_str
                        | ( 'guard_enter' | 'guard_exit' ), '=', guard_expression
                        | 'strategy', '=', ( 'Nested' | 'Parallel' )
                        | 'behavior', '=', ( 'Rebirth' | 'Resurrection' | 'Death' )
                        | 'state_scene', '=', expr_bsn
                        | 'fsm_blueprint', '=', rust_expression (* "hybrid" feature *)
                        | 'minimal'

action_id ::= lit_str
            | fn_identifier
            | action_name, ':', ( expr_closure | expr_call | fn_identifier )

guard_expression ::= (* See combination_condition! macro for details *)

expr_bsn ::= '{' bsn '}' | '[' bsn_list ']'

(* ---- Shared Primitives ---- *)

fn_identifier ::= (* A Rust path to a function, e.g., my_function *)
expr_closure ::= (* A Rust closure, e.g., |...| { ... } *)
expr_call ::= (* A Rust function call, e.g., my_function(a, b) *)
action_name ::= identifier
identifier ::= (* A Rust identifier, e.g., MyState, StateA *)
integer_literal ::= (* A Rust integer literal, e.g., 0, 42 *)
lit_str ::= (* A Rust string literal, e.g., "my_system" *)
rust_expression ::= (* Any valid Rust expression *)
```

### `hsm!`

The `hsm!` macro is used to build a Hierarchical State Machine. It defines a tree structure with a single root state and optional additional Bevy components attached to the state machine entity.

```ebnf
(*
 * The hsm! macro is flexible. It requires one root state_node, and allows at most one
 * machine_config and one config_fn. Components can be freely interspersed.
 * A typical structure is shown below.
 *)
hsm ::= [ machine_config, ',' ], state_node, { ',', component }, [ ',', config_fn ]

state_node ::= state_attribute, [ ':', identifier ], [ '(', { hsm_state_content }, ')' ]
hsm_state_content ::= ( state_node | component ), { ',', ( state_node | component ) }

(* Definitions for `machine_config`, `state_attribute`, `component`, `config_fn` are in "Common Definitions". *)
```

**Key Points**:

- The core of the `hsm!` macro is a single `state_node`, representing the root of the state tree.
- After the root state, you can append any number of Bevy `component`s, which will be added to the same entity as the state machine.
- The `state_node` can be configured with the `#[state(...)]` attribute. In addition to common lifecycle hooks (like `on_update`, `after_enter`), it supports HSM-exclusive attributes, including guards for automatic transitions (`guard_enter`, `guard_exit`) and properties for controlling hierarchical behavior like `strategy` and `behavior`.
- States can be nested. Child states and components are defined within the `()` of the parent state.

### `fsm!`

The `fsm!` macro is used to build a flat Finite State Machine. It defines a set of states, a set of transition rules, and optional additional components.

```ebnf
fsm ::= [ machine_config, ',' ], fsm_graph_content, [ ',', components_block ], [ ',', config_fn ]

components_block ::= 'components', ':', '{', [ component, { ',', component } ], '}'

(* Definitions for `machine_config`, `config_fn`, `component` are in "Common Definitions". *)
(* The definition for `fsm_graph_content` is in the `fsm_graph!` macro. *)
```

**Key Points**:

- The `fsm!` macro consists of `fsm_graph_content`, an optional `components` block, and an optional `config_fn`.
- The `fsm_graph_content` is required and contains both a `states` and a `transitions` block.
- The syntax for `fsm_state` is similar to `state_node` in `hsm!`, but it cannot contain nested states.
- `fsm_state` also supports the `#[state(...)]` attribute. However, please note that because FSMs have a flat, event-driven structure, parameters in `#[state(...)]` related to HSM's automatic transitions and hierarchy (like `guard_enter`, `guard_exit`, `strategy`, `behavior`) are invalid here. (*FSM transitions must be explicitly triggered by an `FsmTrigger` event and thus do not support automatic guards.*)
- A `transition` defines the rules for moving between states.
  - The arrows define the direction of the transition: `=>` (unidirectional), `<=` (unidirectional), `<=>` (bidirectional).
  - Transitions can be made conditional with `event` or `guard`.

### `hsm_tree!`

`hsm_tree!` is a utility macro for building a standalone state tree (`StateTree`). Its syntax is a subset of the `hsm!` macro, accepting only a single root `state_node`.

```ebnf
hsm_tree ::= state_node, [ ',', config_fn ]

(* Definitions for `state_node` and `config_fn` are in "Common Definitions". *)
```

### `fsm_graph!`

`fsm_graph!` is a utility macro for building a standalone state graph (`FsmGraph`). Its syntax is a subset of the `fsm!` macro.

```ebnf
fsm_graph ::= fsm_graph_content, [ ',', config_fn ]

fsm_graph_content ::= 'states', ':', '{', [ fsm_state, { ',', fsm_state } ], '}',
                      [ ',' ],
                      'transitions', ':', '{', [ transition, { ',', transition } ], '}'

fsm_state ::= state_attribute, [ ':', identifier ], [ '(', { component }, ')' ]

transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref, [ ':', transition_condition ]
transition_condition ::= 'event', '(', rust_expression, ')'
                       | 'guard', '(', guard_expression, ')'

(* Other definitions are in "Common Definitions". *)
```

### `system_registry!`

`system_registry!` is a helper macro for dynamically registering multiple Bevy systems into a `SystemRegistry` resource. This is useful when you need to pass a collection of related systems (e.g., as state actions) to a state machine.

```ebnf
system_registry ::= '<', source, ',', system_registry, '>', '[', [ system_definition, { ',', system_definition } ], ']';
system_definition ::= ( lit_str | fn_identifier ), '=>', rust_expression;

source ::= identifier; (* A variable of type `Commands` or `World` *)
system_registry ::= identifier; (* A variable that implements `Extend<(SystemLabel, SystemId)>` *)
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
