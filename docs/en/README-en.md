# Bevy HSM (Hierarchical State Machine)

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[中文](../../README.md)

A hierarchical state machine system for the Bevy engine that implements hierarchical state machine functionality.

## Features

- Supports state lifecycle phases: enter, update, and exit
- Supports hierarchical states (parent and child states)
- Supports state transition conditions
- Supports state machine system and condition system registration
- Provides state transition history functionality
- Supports state priority management
- Supports combination condition systems

## Usage

Add the HSM plugin to your Bevy app:

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(HsmPlugin::default())
        .run();
}
```

## Core Concepts

- StateMachine: Manages entity state transitions, including current state, next state, and state mapping table
- HsmOnState: State lifecycle management, used to manage state entering, updating, and exiting
- StationaryStateMachine: Used to pause state machines
- Terminated: Component used to indicate that a state machine has terminated
- StateConditions: State transition condition system for determining if states meet conditions for entering or exiting
- HsmState: Represents a state, associated with the main entity (the entity that owns the StateMachine component)
- StateTree: State tree, used to describe the transition relationships between states
- HsmOnEnterCondition: Condition for entering a state
- HsmOnExitCondition: Condition for exiting a state
- HsmOnEnterSystem: System for entering a state, used to execute logic when a state is entered
- HsmOnUpdateSystem: System for updating a state, used to execute logic when a state is updated
- HsmOnExitSystem: System for exiting a state, used to execute logic when a state is exited

## Conclusion

Currently, bevy_hsm is in the development stage and will continue to be improved and have new features added. Of course, you can submit issues or pull requests to help improve this library.

## License

This project is licensed under either MIT or Apache 2.0, you may choose whichever you prefer to use this project.

- MIT License ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
