use std::{fmt, sync::Arc};

use bevy::{ecs::schedule::ScheduleError, prelude::*};

use crate::labels::SystemLabel;

/// The error type for operations within the state machine crate.
#[derive(Debug, Clone)]
#[non_exhaustive]
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
        source: String,
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
    /// An event-triggered FSM transition did not match any transition from the current state.
    #[cfg(feature = "fsm")]
    NoMatchingEventTransition {
        graph: Entity,
        state: Entity,
    },
    /// The lowest common ancestor (LCA) could not be found between two states in an HSM transition.
    #[cfg(feature = "hsm")]
    LcaNotFound {
        state_machine: Entity,
        from: Entity,
        to: Entity,
    },
    /// A resume was triggered on a state machine with an empty interrupt stack.
    InterruptStackEmpty(Entity),
    /// A guard condition was not found in the guard registry.
    GuardNotFound {
        condition: String,
        target: Entity,
    },
    ActionBufferAlreadyExists(SystemLabel, &'static str),
    ActionBufferNotExists(SystemLabel, &'static str),
    ActionNotFound(SystemLabel),
    ScheduleError(Arc<ScheduleError>),
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
                        "Failed to run enter guard for transition from {:?} to {:?} on state machine {:?}. Source: {}",
                        from_state, to_state, state_machine, source
                    )
                } else {
                    write!(
                        f,
                        "Failed to run exit guard for state {:?} on state machine {:?}. Source: {}",
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
            #[cfg(feature = "fsm")]
            StateMachineError::NoMatchingEventTransition { graph, state } => {
                write!(
                    f,
                    "No matching event transition found from state {:?} in FsmGraph {:?}",
                    state, graph
                )
            }
            #[cfg(feature = "hsm")]
            StateMachineError::LcaNotFound {
                state_machine,
                from,
                to,
            } => {
                write!(
                    f,
                    "Cannot find lowest common ancestor between {:?} and {:?} on state machine {:?}",
                    from, to, state_machine
                )
            }
            StateMachineError::InterruptStackEmpty(state_machine) => {
                write!(
                    f,
                    "Resume called on state machine {:?} with empty interrupt stack",
                    state_machine
                )
            }
            StateMachineError::GuardNotFound { condition, target } => {
                write!(
                    f,
                    "Guard condition {:?} not found in registry for target {:?}",
                    condition, target
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
            StateMachineError::ScheduleError(schedule_error) => {
                write!(f, "Schedule error: {}", schedule_error)
            }
        }
    }
}

impl From<ScheduleError> for StateMachineError {
    fn from(value: ScheduleError) -> Self {
        StateMachineError::ScheduleError(Arc::new(value))
    }
}

impl std::error::Error for StateMachineError {}

/// # 状态机运行时错误事件
///
/// 当状态机在运行时遇到错误时触发，用户可通过 Bevy 的观察者系统注册处理逻辑。
/// 在观察者中，用户可以选择记录日志、触发中断、或执行其他自定义恢复逻辑。
///
/// # State Machine Runtime Error Event
///
/// Triggered when a state machine encounters a runtime error. Users can register
/// observer systems to handle these errors, choosing to log, trigger an interrupt,
/// or perform other custom recovery logic.
///
/// ## Example
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// fn handle_errors(on: On<StateMachineErrorEvent>, mut commands: Commands) {
///     match &on.error {
///         StateMachineError::InvalidTransitionTarget { from_state, to_state, .. } => {
///             warn!("Invalid transition {:?} -> {:?}, delegating to interrupt handler", from_state, to_state);
///             // commands.trigger(FsmTrigger::with_interrupt(on.state_machine, error_recovery_state));
///         }
///         _ => error!("{}", on.error),
///     }
/// }
/// ```
#[derive(EntityEvent, Clone, Debug)]
pub struct StateMachineErrorEvent {
    #[event_target]
    pub state_machine: Entity,
    pub error: StateMachineError,
}

impl StateMachineErrorEvent {
    pub const fn new(state_machine: Entity, error: StateMachineError) -> Self {
        Self {
            state_machine,
            error,
        }
    }
}

#[cfg(any(feature = "hsm", feature = "fsm"))]
macro_rules! define_error_event {
    ($fn_name:ident, $world_fn_name:ident, $log_level:ident) => {
        /// Convenience helper to log and trigger a [`StateMachineErrorEvent`].
        ///
        /// Use this in contexts where `Commands` is available (systems, observers).
        pub(crate) fn $fn_name(
            commands: &mut Commands,
            state_machine: Entity,
            error: StateMachineError,
        ) {
            $log_level!("{}", error);
            commands.trigger(StateMachineErrorEvent::new(state_machine, error));
        }

        /// Convenience helper to log and trigger a [`StateMachineErrorEvent`] from `&mut World`.
        ///
        /// Use this inside `commands.queue()` closures or other World-only contexts.
        pub(crate) fn $world_fn_name(
            world: &mut World,
            state_machine: Entity,
            error: StateMachineError,
        ) {
            $log_level!("{}", error);
            if let Ok(mut entity) = world.get_entity_mut(state_machine) {
                entity.trigger(|sm| StateMachineErrorEvent::new(sm, error.clone()));
            }
        }
    };
}

#[cfg(any(feature = "hsm", feature = "fsm"))]
define_error_event!(error_event, error_event_world, error);
#[cfg(feature = "hsm")]
define_error_event!(warn_event, warn_event_world, warn);

/// Convenience helper to log a trace and trigger a [`StateMachineErrorEvent`].
#[cfg(feature = "fsm")]
pub(crate) fn trace_event(
    commands: &mut Commands,
    state_machine: Entity,
    error: StateMachineError,
) {
    trace!("{}", error);
    commands.trigger(StateMachineErrorEvent::new(state_machine, error));
}
