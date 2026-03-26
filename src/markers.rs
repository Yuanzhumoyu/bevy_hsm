use std::sync::Arc;

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{context::*, prelude::StateActionBuffer, state_actions::*};

#[cfg(feature = "hsm")]
use crate::hsm::state_machine::HsmStateMachine;

#[cfg(feature = "fsm")]
use crate::fsm::state_machine::FsmStateMachine;

/// # 终止状态机标记组件\Termination Marker Component
/// 表示状态机已经终止，不再处理状态转换
///
/// Indicates that the state machine has terminated and no longer processes state transitions
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[cfg_attr(any(feature = "hsm",feature = "fsm"), component(on_remove = Self::on_remove))]
#[require(Paused)]
pub struct Terminated;

#[cfg(any(feature = "hsm", feature = "fsm"))]
impl Terminated {
    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        #[cfg(feature = "fsm")]
        'fsm: {
            let service_target = match world.get::<ServiceTarget>(entity) {
                Some(service_target) => service_target.0,
                None => entity,
            };

            let Some(fsm_state_machine) = world.get::<FsmStateMachine>(entity) else {
                break 'fsm;
            };

            let curr_state = fsm_state_machine.curr_state_id();
            'before_exit: {
                let Some(before_exit) = world.get::<BeforeExitSystem>(curr_state) else {
                    break 'before_exit;
                };
                let Some(id) = world.resource::<ActionRegistry>().get(before_exit.as_str()) else {
                    break 'before_exit;
                };
                let context = ActionContext::new(service_target, entity, curr_state);
                unsafe {
                    let _ = world
                        .as_unsafe_world_cell()
                        .world_mut()
                        .run_system_with(id, context);
                };
            }

            #[cfg(feature = "state_data")]
            crate::state_data::StateData::remove_components(&mut world, curr_state, service_target);

            let Some(mut fsm_state_machine) = world.get_mut::<FsmStateMachine>(entity) else {
                break 'fsm;
            };

            let init_state = fsm_state_machine.init_state_id();
            fsm_state_machine.set_curr_state(init_state);

            #[cfg(all(feature = "history", feature = "hybrid"))]
            'set_fsm_history: {
                if fsm_state_machine.history.is_empty() {
                    break 'set_fsm_history;
                }

                let fsm_history = fsm_state_machine.history.take();

                let Some(mut state_machine) = world.get_mut::<HsmStateMachine>(entity) else {
                    break 'fsm;
                };
                state_machine
                    .history
                    .set_last_state_fsm_history(fsm_history);
            };

            #[cfg(feature = "state_data")]
            crate::state_data::StateData::clone_components(&mut world, init_state, service_target);

            'after_enter: {
                let Some(after_enter) = world.get::<AfterEnterSystem>(init_state) else {
                    break 'after_enter;
                };
                let Some(id) = world.resource::<ActionRegistry>().get(after_enter.as_str()) else {
                    break 'after_enter;
                };
                let context = ActionContext::new(service_target, entity, init_state);
                unsafe {
                    let _ = world
                        .as_unsafe_world_cell()
                        .world_mut()
                        .run_system_with(id, context);
                };
            }
        }

        #[cfg(feature = "hsm")]
        if let Some(mut state_machine) = world.get_mut::<HsmStateMachine>(entity) {
            use crate::prelude::StateLifecycle;

            state_machine.clear_next_states();
            #[cfg(feature = "history")]
            state_machine.clear_history();

            let init_state = state_machine.init_state();
            state_machine.set_curr_state(init_state);
            world
                .commands()
                .entity(entity)
                .insert(StateLifecycle::Enter);
        }
    }
}

/// # 状态机组件\State Machine Component
/// * 用于静止拥有该组件的状态机
/// - Used for state machines that statically possess this component
/// * 如果存在, 系统不会在运行状态机的状态转换时调用状态的OnEnter、BeforeExit、OnUpdate系统
/// - If it exists, the AfterEnter, BeforeExit, and OnUpdate systems of the state machine will not be called during the running of the state machine's state transition
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[cfg_attr(any(feature = "hsm",feature = "fsm"), component(on_insert = Self::on_insert, on_remove = Self::on_remove))]
pub struct Paused;

#[cfg(any(feature = "hsm", feature = "fsm"))]
impl Paused {
    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let service_target = match world.get::<ServiceTarget>(entity) {
            Some(service_target) => service_target.0,
            None => entity,
        };

        #[cfg(feature = "hsm")]
        'hsm: {
            let Some(state_machine) = world.get::<HsmStateMachine>(entity) else {
                break 'hsm;
            };
            // 查看当前状态是否有OnUpdateSystem,则将其添加进延期表中
            let curr_state_id = state_machine.curr_state_id();
            let state_context = ActionContext::new(service_target, entity, curr_state_id.state());

            let unsafe_world_cell = world.as_unsafe_world_cell();
            StateActionBuffer::buffer_scope(
                unsafe_world_cell,
                curr_state_id.state(),
                move |_world, buff| {
                    buff.add(state_context);
                },
            );
        }

        #[cfg(feature = "fsm")]
        'fsm: {
            let Some(state_machine) = world.get::<FsmStateMachine>(entity) else {
                break 'fsm;
            };

            let curr_state_id = state_machine.curr_state_id();
            let state_context = ActionContext::new(service_target, entity, curr_state_id);

            let unsafe_world_cell = world.as_unsafe_world_cell();
            StateActionBuffer::buffer_scope(
                unsafe_world_cell,
                curr_state_id,
                move |_world, buff| {
                    buff.add_filter(state_context);
                },
            );
        }
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let service_target = match world.get::<ServiceTarget>(entity) {
            Some(service_target) => service_target.0,
            None => entity,
        };

        #[cfg(feature = "hsm")]
        'hsm: {
            let Some(state_machine) = world.get::<HsmStateMachine>(entity) else {
                break 'hsm;
            };
            let curr_state_id = state_machine.curr_state_id();
            let state_context = ActionContext::new(service_target, entity, curr_state_id.state());

            let unsafe_world_cell = world.as_unsafe_world_cell();
            StateActionBuffer::buffer_scope(
                unsafe_world_cell,
                curr_state_id.state(),
                move |_world, buff| {
                    buff.add(state_context);
                },
            );
        }

        #[cfg(feature = "fsm")]
        'fsm: {
            let Some(state_machine) = world.get::<FsmStateMachine>(entity) else {
                break 'fsm;
            };

            let curr_state_id = state_machine.curr_state_id();
            let state_context = ActionContext::new(service_target, entity, curr_state_id);

            let unsafe_world_cell = world.as_unsafe_world_cell();
            StateActionBuffer::buffer_scope(
                unsafe_world_cell,
                curr_state_id,
                move |_world, buff| {
                    buff.add(state_context);
                },
            );
        }
    }
}

#[derive(Component, Clone)]
#[component(on_insert=Self::on_insert)]
pub struct SpawnStateMachine(Arc<dyn Fn(EntityCommands) + 'static + Send + Sync>);

impl SpawnStateMachine {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(EntityCommands) + 'static + Send + Sync,
    {
        Self(Arc::new(f))
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let (entitys, mut commands) = world.entities_and_commands();
        let Ok(entity_ref) = entitys.get(entity) else {
            return;
        };
        if let Some(f) = entity_ref.get::<Self>() {
            (f.0)(commands.entity(entity))
        }
        commands.entity(entity).remove::<Self>();
    }
}
