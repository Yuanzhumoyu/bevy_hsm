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
  - **Transition Strategy**: Configurable behavior for parent-child state transitions (`StateTransitionStrategy`: `Nested` / `Parallel`).
  - **Return Behavior**: Configurable behavior for the parent state after a child state returns (`ExitTransitionBehavior`: `Rebirth` / `Resurrection` / `Death`).
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
bevy_hsm = { version = "0.18", features = ["history", "hsm", "fsm"] }
```

## Epilogue

`bevy_hsm` is still under active development, and new features will continue to be added and improved. You are welcome to help improve this library by submitting Issues or Pull Requests.

## License

This project is licensed under either of

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
