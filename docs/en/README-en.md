# Bevy HSM — Hybrid State Machine System

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[中文](../../README.md)

A powerful hybrid state machine system designed for the [Bevy Game Engine](https://bevyengine.org/). It seamlessly integrates **Hierarchical State Machine (HSM)** and **Finite State Machine (FSM)**, letting you choose the right tool for every scenario:

- Use **HSM** for complex, hierarchical behavioral states with full lifecycle management
- Use **FSM** for flat, event-driven, fast state switching
- Freely switch between modes via the **interrupt/resume** mechanism for emergency handling

---

## Features

- **Dual-mode support**: HSM and FSM in a unified framework, composable as needed
- **State lifecycle**: Five lifecycle phases — `BeforeEnter` → `AfterEnter` → `OnUpdate` → `BeforeExit` → `AfterExit` — each associated with independent Bevy systems
- **Hierarchical structure (HSM)**: Arbitrary state nesting for logic reuse and composition
- **Transition strategy (HSM)**:
  - `Nested` — child active while parent stays active
  - `Parallel` — parent exits then re-enters when child changes
- **Exit behavior (HSM)**:
  - `Rebirth` — parent re-executes Enter after child exits
  - `Resurrection` — parent resumes Update after child exits
  - `Death` — parent exits along with child, cascading upward
- **Guard mechanism (HSM)**: `GuardEnter` / `GuardExit` for automatic transition control, with `and` / `or` / `not` combinators
- **Transition systems**: `BeforeEnterSystem` / `AfterExitSystem` for transition-specific logic
- **State data (`state_data`)**: Auto-add/remove components and child entities on state enter/exit
- **Interrupt/resume**: Stack-based nested interrupts for both HSM and FSM, with cross-graph/tree jumps
- **History (`history`)**: State transition history recording with traceback and FSM snapshots
- **Hybrid architecture**: Nest an FSM inside an HSM state — a "state machine within a state machine"
- **Declarative macros**: `hsm!` / `fsm!` / `hsm_tree!` / `fsm_graph!` / `combination_condition!` / `system_registry!`
- **Customizable scheduling**: `StateMachinePlugin::with_schedule()` for specifying the run schedule
- **ServiceTarget**: Delegate state machine events to a separate entity for logic separation

---

## Quick Start

Add the dependency in `Cargo.toml`:

```toml
[dependencies]
bevy_hsm = "0.19"
```

Register the plugin in your Bevy app:

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default())
        .run();
}
```

### Your First HSM

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, handle_input)
        .run();
}

fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    // Register action systems
    let enter_id = commands.register_system(|In(ctx): In<ActionContext>| {
        info!("Entering state: {:?}", ctx.state());
    });
    action_registry.insert("log_enter", enter_id);

    // Create states
    let root = commands.spawn((
        Name::new("Root"),
        HsmState::default(),
        AfterEnterSystem::new("log_enter"),
    )).id();

    let child = commands.spawn((
        Name::new("Child"),
        HsmState::default(),
        AfterEnterSystem::new("log_enter"),
    )).id();

    // Build state tree
    let mut tree = StateTree::new(root);
    let tree_id = tree.with_child(root, child);

    // Spawn state machine
    commands.spawn((
        tree,
        StateLifecycle::default(),
        HsmStateMachine::with(
            tree_id,
            root,
            #[cfg(feature = "history")] 10,
        ),
    ));
}
```

See [examples/](../../examples/) and [examples/README.md](../../examples/README.md) for more complete examples.

---

## Core Concepts

### Common Components

| Component | Purpose |
| ----------- | --------- |
| `BeforeEnterSystem` | Executes before entering a state |
| `AfterEnterSystem` | Executes after entering a state |
| `OnUpdateSystem` | Executes every frame while state is active |
| `BeforeExitSystem` | Executes before exiting a state |
| `AfterExitSystem` | Executes after exiting a state |
| `Paused` | Pauses the state machine, blocking all transitions |
| `Terminated` | Marks the state machine as terminated |
| `ServiceTarget` | Delegates state machine events to a specific entity |
| `SpawnStateMachine` | Deferred state machine creation via closure |

### Context Types

| Type | Purpose |
| ----------- | --------- |
| `ActionContext` | Passed to action systems; contains `service_target`, `state_machine`, `state` |
| `GuardContext` | Passed to guard systems; contains `service_target`, `state_machine`, `from_state`, `to_state` |
| `TransitionContext` | Passed to transition systems; contains `service_target`, `state_machine`, transition relationship |

### Registries

| Resource | Purpose |
| ----------- | --------- |
| `ActionRegistry` | Registers action systems (key-value pairs) |
| `GuardRegistry` | Registers guard systems |
| `TransitionRegistry` | Registers transition systems |
| `ActionDispatch` | Action dispatch mapping (auto-managed) |

### HSM Core

| Type | Description |
| ----------- | --------- |
| `HsmState` | HSM state component; configures `strategy` and `behavior` |
| `HsmStateMachine` | HSM runtime component; manages current state, state tree, and transition queue |
| `StateTree` | State tree component; defines parent-child relationships |
| `StateLifecycle` | Lifecycle detection component (`Enter` / `Update` / `Exit`) |
| `HsmTrigger` | HSM transition trigger; supports `to_sub` / `to_super` / `chain` / `guard_sub` / `guard_super` / `interrupt` / `resume` |
| `GuardEnter` / `GuardExit` | Guard components attached to states |
| `StateTransitionStrategy` | Transition strategy (`Nested` / `Parallel`) |
| `ExitTransitionBehavior` | Exit behavior (`Rebirth` / `Resurrection` / `Death`) |

### FSM Core

| Type | Description |
| ----------- | --------- |
| `FsmState` | FSM state marker component |
| `FsmStateMachine` | FSM runtime component; manages current state and graph reference |
| `FsmGraph` | FSM graph component; defines state topology and transition rules |
| `FsmTrigger` | FSM transition trigger; supports `next` / `guard` / `event` / `interrupt` / `resume` |
| `FsmBlueprint` | FSM blueprint for nesting an FSM inside an HSM state (Hybrid architecture) |

### Interrupt Mechanism

Both HSM and FSM support a stack-based interrupt/resume mechanism. On interrupt, the current state and graph are saved, and execution jumps to a handler state; after handling, `Resume` restores the previous state. Nested interrupts and cross-graph/tree jumps are supported.

```rust
// HSM: save current state and interrupt to handler state
commands.trigger(HsmTrigger::interrupt(sm, target_tree, target_state));
// After handling, resume
commands.trigger(HsmTrigger::resume(sm));

// FSM: save current state and interrupt to another graph
commands.trigger(FsmTrigger::with_interrupt(sm, target_graph, target_state));
// After handling, resume
commands.trigger(FsmTrigger::with_resume(sm));
```

---

## Using `hsm!` / `fsm!` Macros

```rust
use bevy_hsm::prelude::*;

// HSM: declarative state tree definition
hsm!(
    init(init_state = root, history_capacity = 10),
    #[state(on_update = "Update:log_update", behavior = Rebirth)]
    root: Root (
        #[state(guard_enter = "is_open", guard_exit = "is_close")]
        child: Child
    ),
    Paused
);

// FSM: declarative state graph definition
fsm!(
    init(init_state = idle),
    states: {
        #[state(after_enter = "log_enter")]
        idle: Idle,
        #[state(after_enter = "log_enter")]
        walking: Walking
    },
    transitions: {
        idle => walking: event(ToggleEvent),
        walking => idle: event(ToggleEvent)
    }
);
```

---

## Cargo Features

| Feature | Default | Description |
| ----------- | --------- | --------- |
| `hsm` | ✓ (via hybrid) | Enables Hierarchical State Machine |
| `fsm` | ✓ (via hybrid) | Enables Finite State Machine |
| `hybrid` | ✓ | Enables both `hsm` + `fsm`, with nested architecture support |
| `history` | ✓ | State transition history recording |
| `state_data` | ✓ | State-associated scene data (auto component/child management) |

Custom configuration:

```toml
[dependencies]
bevy_hsm = { version = "0.19", default-features = false, features = ["hsm", "history"] }
```

---

## Examples

All examples are in the [examples/](../../examples/) directory. See [examples/README.md](../../examples/README.md) for detailed descriptions.

| Example | Description |
| ----------- | --------- |
| `hello_hsm` | HSM intro: state tree, lifecycle, ToSub/ToSuper |
| `simple_fsm` | FSM intro: event-driven transitions |
| `event_fsm` | FSM event system with pause/resume |
| `guard_hsm` | Basic GuardEnter usage |
| `guarded_counter` | Parallel strategy with guard combinations |
| `flashing_light` | Timer-driven automatic guard transitions |
| `deep_chain` | Deeply nested guard navigation |
| `character_controller` | `hsm!` macro declarative character state machine |
| `interrupt_hsm` | Interrupt/resume with cross-tree transitions |
| `vending_machine` | Compound guards, ServiceTarget, event-driven flow |
| `macros/hsm` | Complete `hsm!` macro DSL usage |
| `macros/fsm` | Complete `fsm!` macro DSL usage |
| `calculator` | HSM+FSM Hybrid architecture GUI application |

Run examples:

```bash
cargo run --example hello_hsm
cargo run --example calculator --features "hybrid, history"
```

---

## License

This project is dual-licensed under either of

- MIT License ([LICENSE-MIT](../../LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.
