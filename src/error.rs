use bevy::{ecs::schedule::ScheduleError, prelude::Entity};
use std::fmt;

use crate::labels::SystemLabel;

/// The error type for operations within the state machine crate.
#[derive(Debug)]
pub enum StateMachineError {
    /// A required `StateTree` component was not found on an entity.
    #[cfg(feature = "hsm")]
    StateTreeNotFound(Entity),
    /// A required [`HsmStateMachine`]component was not found on an entity.
    #[cfg(feature = "hsm")]
    HsmStateMachineMissing(Entity),
    /// A required `HsmState` component was not found on a state entity.
    #[cfg(feature = "hsm")]
    HsmStateMissing(Entity),
    /// A required `StateLifecycle` component was not found on a state entity.
    #[cfg(feature = "hsm")]
    StateLifecycleMissing(Entity),
    /// A registered system could not be found by its name.
    SystemNotFound {
        system_name: SystemLabel,
        state: Entity,
    },
    /// An error occurred while running a transition's guard system.
    #[cfg(feature = "hsm")]
    GuardRunFailed {
        state_machine: Entity,
        from_state: Entity,
        to_state: Option<Entity>, // `to_state` is for enter guards
        source: bevy::ecs::system::RunSystemError,
    },
    /// A super state was not found for a given state within its `StateTree`.
    #[cfg(feature = "hsm")]
    SuperStateNotFound {
        state_tree: Entity,
        state: Entity,
    },
    /// A sub state was not found for a given state within its `StateTree`.
    #[cfg(feature = "hsm")]
    SubStateNotFound {
        state_tree: Entity,
        state: Entity,
    },
    /// A required [`FsmStateMachine`] component was not found on an entity.
    #[cfg(feature = "fsm")]
    FsmStateMachineMissing(Entity),
    /// A required [`FsmGraph`] component was not found on an entity.
    #[cfg(feature = "fsm")]
    GraphMissing(Entity),
    /// A state was not found within the [`FsmGraph`].
    #[cfg(feature = "fsm")]
    StateNotInGraph {
        graph: Entity,
        state: Entity,
    },
    /// An attempt was made to transition to a target that is not a valid state in the graph.
    #[cfg(feature = "fsm")]
    InvalidTransitionTarget {
        graph: Entity,
        from_state: Entity,
        to_state: Entity,
    },
    ActionBufferAlreadyExists(SystemLabel, &'static str),
    ActionBufferNotExists(SystemLabel, &'static str),
    ActionNotFound(SystemLabel),
    ScheduleError(ScheduleError),
}

impl fmt::Display for StateMachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "hsm")]
            StateMachineError::StateTreeNotFound(tree_entity) => {
                write!(
                    f,
                    "StateTree component not found on entity {:?}",
                    tree_entity
                )
            }
            #[cfg(feature = "hsm")]
            StateMachineError::HsmStateMachineMissing(entity) => {
                write!(
                    f,
                    "HsmStateMachine component not found on entity {:?}",
                    entity
                )
            }
            #[cfg(feature = "hsm")]
            StateMachineError::HsmStateMissing(entity) => {
                write!(f, "HsmState component not found on entity {:?}", entity)
            }
            #[cfg(feature = "hsm")]
            StateMachineError::StateLifecycleMissing(entity) => {
                write!(
                    f,
                    "StateLifecycle component not found on entity {:?}",
                    entity
                )
            }
            StateMachineError::SystemNotFound { system_name, state } => write!(
                f,
                "System '{}' not found for state {:?}",
                system_name, state
            ),
            #[cfg(feature = "hsm")]
            StateMachineError::GuardRunFailed {
                state_machine,
                from_state,
                to_state,
                source,
            } => {
                if let Some(to_state) = to_state {
                    write!(
                        f,
                        "Failed to run enter guard for transition from {:?} to {:?} on state machine {:?}. Source: {:?}",
                        from_state, to_state, state_machine, source
                    )
                } else {
                    write!(
                        f,
                        "Failed to run exit guard for state {:?} on state machine {:?}. Source: {:?}",
                        from_state, state_machine, source
                    )
                }
            }
            #[cfg(feature = "hsm")]
            StateMachineError::SuperStateNotFound { state_tree, state } => {
                write!(
                    f,
                    "Super state not found for state {:?} in StateTree {:?}",
                    state, state_tree
                )
            }
            #[cfg(feature = "hsm")]
            StateMachineError::SubStateNotFound { state_tree, state } => {
                write!(
                    f,
                    "Sub state not found for state {:?} in StateTree {:?}",
                    state, state_tree
                )
            }
            #[cfg(feature = "fsm")]
            StateMachineError::FsmStateMachineMissing(entity) => {
                write!(
                    f,
                    "FsmStateMachine component not found on entity {:?}",
                    entity
                )
            }
            #[cfg(feature = "fsm")]
            StateMachineError::GraphMissing(graph_entity) => {
                write!(
                    f,
                    "FsmGraph component not found on entity {:?}",
                    graph_entity
                )
            }
            #[cfg(feature = "fsm")]
            StateMachineError::StateNotInGraph { graph, state } => {
                write!(f, "State {:?} not found in FsmGraph {:?}", state, graph)
            }
            #[cfg(feature = "fsm")]
            StateMachineError::InvalidTransitionTarget {
                graph,
                from_state,
                to_state,
            } => {
                write!(
                    f,
                    "Invalid transition from {:?} to {:?} in FsmGraph {:?}: target state does not exist in graph.",
                    from_state, to_state, graph
                )
            }
            StateMachineError::ActionBufferAlreadyExists(system_label, schedule_name) => write!(
                f,
                "The system<{}> for this ScheduleLabel<{}> already exists",
                system_label, schedule_name
            ),
            StateMachineError::ActionBufferNotExists(system_label, schedule_name) => write!(
                f,
                "The system<{}> for this ScheduleLabel<{}> does not exist",
                system_label, schedule_name
            ),
            StateMachineError::ActionNotFound(system_label) => {
                write!(f, "Action with label {} not found", system_label)
            }
            StateMachineError::ScheduleError(schedule_error) => schedule_error.fmt(f),
        }
    }
}

impl From<ScheduleError> for StateMachineError {
    fn from(value: ScheduleError) -> Self {
        StateMachineError::ScheduleError(value)
    }
}

impl std::error::Error for StateMachineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}
