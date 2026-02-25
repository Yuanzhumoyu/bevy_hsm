use bevy::prelude::Entity;
use std::fmt;

/// The error type for operations within the `bevy_hsm` crate.
#[derive(Debug)]
pub enum HsmError {
    /// A required `StateTree` component was not found on an entity.
    StateTreeNotFound(Entity),
    /// A required `HsmStateMachine` component was not found on an entity.
    StateMachineMissing(Entity),
    /// A required `HsmState` component was not found on a state entity.
    HsmStateMissing(Entity),
    /// A required `HsmOnState` component was not found on a state entity.
    HsmOnStateMissing(Entity),
    /// A registered system could not be found by its name.
    SystemNotFound { system_name: String, state: Entity },
    /// An error occurred while running a state's action system (OnEnter, OnUpdate, OnExit).
    SystemRunFailed {
        system_name: String,
        state: Entity,
        source: bevy::ecs::system::RunSystemError,
    },
    /// An error occurred while running a transition's condition system.
    ConditionRunFailed {
        state_machine: Entity,
        from_state: Entity,
        to_state: Option<Entity>, // `to_state` is for enter conditions
        source: bevy::ecs::system::RunSystemError,
    },
    /// A super state was not found for a given state within its `StateTree`.
    SuperStateNotFound { state_tree: Entity, state: Entity },
    /// A required `FsmStateMachine` component was not found on an entity.
    FsmStateMachineNotFound(Entity),
    /// A required `FsmGraph` component was not found on an entity.
    GraphNotFound(Entity),
    /// A state was not found within the `FsmGraph`.
    StateNotInGraph { graph: Entity, state: Entity },
    /// An attempt was made to transition to a target that is not a valid state in the graph.
    InvalidTransitionTarget {
        graph: Entity,
        from_state: Entity,
        to_state: Entity,
    },
}

impl fmt::Display for HsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HsmError::StateTreeNotFound(tree_entity) => {
                write!(
                    f,
                    "StateTree component not found on entity {:?}",
                    tree_entity
                )
            }
            HsmError::StateMachineMissing(entity) => {
                write!(
                    f,
                    "HsmStateMachine component not found on entity {:?}",
                    entity
                )
            }
            HsmError::HsmStateMissing(entity) => {
                write!(f, "HsmState component not found on entity {:?}", entity)
            }
            HsmError::HsmOnStateMissing(entity) => {
                write!(f, "HsmOnState component not found on entity {:?}", entity)
            }
            HsmError::SystemNotFound { system_name, state } => write!(
                f,
                "System '{}' not found for state {:?}",
                system_name, state
            ),
            HsmError::SystemRunFailed {
                system_name,
                state,
                source,
            } => write!(
                f,
                "Failed to run system '{}' for state {:?}. Source: {:?}",
                system_name, state, source
            ),
            HsmError::ConditionRunFailed {
                state_machine,
                from_state,
                to_state,
                source,
            } => {
                if let Some(to_state) = to_state {
                    write!(
                        f,
                        "Failed to run enter condition for transition from {:?} to {:?} on state machine {:?}. Source: {:?}",
                        from_state, to_state, state_machine, source
                    )
                } else {
                    write!(
                        f,
                        "Failed to run exit condition for state {:?} on state machine {:?}. Source: {:?}",
                        from_state, state_machine, source
                    )
                }
            }
            HsmError::SuperStateNotFound { state_tree, state } => {
                write!(
                    f,
                    "Super state not found for state {:?} in StateTree {:?}",
                    state, state_tree
                )
            }
            HsmError::FsmStateMachineNotFound(entity) => {
                write!(
                    f,
                    "FsmStateMachine component not found on entity {:?}",
                    entity
                )
            }
            HsmError::GraphNotFound(graph_entity) => {
                write!(
                    f,
                    "FsmGraph component not found on entity {:?}",
                    graph_entity
                )
            }
            HsmError::StateNotInGraph { graph, state } => {
                write!(f, "State {:?} not found in FsmGraph {:?}", state, graph)
            }
            HsmError::InvalidTransitionTarget {
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
        }
    }
}
