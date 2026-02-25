use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{
    context::*, fsm::state_machine::FsmStateMachine, hook_system::*,
    hsm::state_machine::HsmStateMachine, prelude::HsmActionSystemBuffer,
};

/// # 终止状态机标记组件\Termination Marker Component
/// 表示状态机已经终止，不再处理状态转换
///
/// Indicates that the state machine has terminated and no longer processes state transitions
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(on_remove = Self::on_remove)]
#[require(StationaryStateMachine)]
pub struct Terminated;

impl Terminated {
    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        if let Some(mut state_machine) = world.get_mut::<HsmStateMachine>(entity) {
            state_machine.clear_next_states();
            state_machine.clear_history();

            let init_state = state_machine.init_state();
            state_machine.set_curr_state(init_state);
        }

        let Some(fsm_state_machine) = world.get::<FsmStateMachine>(entity) else {
            return;
        };
        'on_exit: {
            let Some(on_exit) = world.get::<OnExitSystem>(fsm_state_machine.curr_state) else {
                break 'on_exit;
            };
            let Some(id) = world
                .resource::<NamedStateSystems>()
                .get(on_exit.as_str())
                .cloned()
            else {
                break 'on_exit;
            };
            let context = OnStateContext::new(
                match world.get::<ServiceTarget>(entity) {
                    Some(service_target) => service_target.0,
                    None => entity,
                },
                entity,
                fsm_state_machine.curr_state,
            );
            unsafe {
                let _ = world
                    .as_unsafe_world_cell()
                    .world_mut()
                    .run_system_with(id, context);
            };
        }

        let Some(mut fsm_state_machine) = world.get_mut::<FsmStateMachine>(entity) else {
            return;
        };

        let init_state = fsm_state_machine.init_state;
        fsm_state_machine.set_curr_state(init_state);

        'set_fsm_history: {
            if fsm_state_machine.history.is_empty() {
                break 'set_fsm_history;
            }

            let fsm_history = fsm_state_machine.history.take();

            let Some(mut state_machine) = world.get_mut::<HsmStateMachine>(entity) else {
                return;
            };
            state_machine
                .history
                .set_last_state_fsm_history(fsm_history);
        };

        'on_enter: {
            let Some(on_enter) = world.get::<OnEnterSystem>(init_state) else {
                break 'on_enter;
            };
            let Some(id) = world
                .resource::<NamedStateSystems>()
                .get(on_enter.as_str())
                .cloned()
            else {
                break 'on_enter;
            };
            let context = OnStateContext::new(
                match world.get::<ServiceTarget>(entity) {
                    Some(service_target) => service_target.0,
                    None => entity,
                },
                entity,
                init_state,
            );
            unsafe {
                let _ = world
                    .as_unsafe_world_cell()
                    .world_mut()
                    .run_system_with(id, context);
            };
        }
    }
}

/// # 状态机组件\State Machine Component
/// * 用于静止拥有该组件的状态机
/// - Used for state machines that statically possess this component
/// * 如果存在, 系统不会在运行状态机的状态转换时调用状态的OnEnter、OnExit、OnUpdate系统
/// - If it exists, the OnEnter, OnExit, and OnUpdate systems of the state machine will not be called during the running of the state machine's state transition
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(on_insert = Self::on_insert,on_remove = Self::on_remove)]
pub struct StationaryStateMachine;

impl StationaryStateMachine {
    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(state_machine) = world.get::<HsmStateMachine>(entity) else {
            return;
        };
        // 查看当前状态是否有OnUpdateSystem,则将其添加进延期表中
        let curr_state_id = state_machine.curr_state_id();
        let state_context = OnStateContext::new(
            match world.get::<ServiceTarget>(entity) {
                Some(service_target) => service_target.0,
                None => entity,
            },
            entity,
            curr_state_id.state(),
        );

        let unsafe_world_cell = world.as_unsafe_world_cell();
        HsmActionSystemBuffer::buffer_scope(
            unsafe_world_cell,
            curr_state_id.state(),
            move |_world, buff| {
                buff.add(state_context);
            },
        );
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(state_machine) = world.get::<HsmStateMachine>(entity) else {
            return;
        };
        let curr_state_id = state_machine.curr_state_id();
        let state_context = OnStateContext::new(
            match world.get::<ServiceTarget>(entity) {
                Some(service_target) => service_target.0,
                None => entity,
            },
            entity,
            curr_state_id.state(),
        );

        let unsafe_world_cell = world.as_unsafe_world_cell();
        HsmActionSystemBuffer::buffer_scope(
            unsafe_world_cell,
            curr_state_id.state(),
            move |_world, buff| {
                buff.add(state_context);
            },
        );
    }
}
