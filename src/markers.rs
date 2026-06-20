use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{context::*, prelude::StateActionBuffer, state_actions::*};

#[cfg(any(feature = "hsm", feature = "fsm"))]
use crate::state_machine::StateMachineState;

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
        {
            let _ = FsmStateMachine::reset_to_init_state(&mut world, entity);
        }

        #[cfg(feature = "hsm")]
        HsmStateMachine::reset_to_init_state(&mut world, entity);
    }
}

/// # 状态机组件\State Machine Component
/// * 用于静止拥有该组件的状态机
/// - Used for state machines that statically possess this component
/// * 如果存在, 系统不会在运行状态机的状态转换时调用状态的Enter、Exit、Update系统
/// - If it exists, the Enter, Exit, and Update systems of the state machine will not be called during the running of the state machine's state transition
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
        pause_resume_helper(&mut world, entity, service_target, true);
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let service_target = match world.get::<ServiceTarget>(entity) {
            Some(service_target) => service_target.0,
            None => entity,
        };
        pause_resume_helper(&mut world, entity, service_target, false);
    }
}

/// Helper shared by [`Paused::on_insert`] (pause) and [`Paused::on_remove`] (resume).
/// Applies `add_filter` (pause) or `add` (resume) to the state action buffer for the
/// current state of either an HSM or FSM state machine.
fn pause_resume_helper(
    world: &mut DeferredWorld,
    entity: Entity,
    service_target: Entity,
    is_pause: bool,
) {
    #[cfg(feature = "hsm")]
    if try_pause_resume::<HsmStateMachine>(world, entity, service_target, is_pause) {
        return;
    }

    #[cfg(feature = "fsm")]
    {
        let _ = try_pause_resume::<FsmStateMachine>(world, entity, service_target, is_pause);
    }
}

/// Applies pause/resume buffer operation for a specific state machine type.
/// Returns `true` if the entity had this type of state machine component.
fn try_pause_resume<S: StateMachineState + Component>(
    world: &mut DeferredWorld,
    entity: Entity,
    service_target: Entity,
    is_pause: bool,
) -> bool {
    let Some(sm) = world.get::<S>(entity) else {
        return false;
    };
    let curr = sm.curr_state_id();
    let ctx = ActionContext::new(service_target, entity, curr);
    let cell = world.as_unsafe_world_cell();
    StateActionBuffer::buffer_scope(cell, curr, move |buff| {
        if is_pause {
            buff.add_filter(ctx);
        } else {
            buff.remove_interceptor(ctx);
            buff.remove_filter(ctx);
            buff.add(ctx);
        }
    });
    true
}

/// # 延迟生成状态机组件\Deferred State Machine Spawner
/// * 一个包装组件，允许通过闭包延迟创建状态机。插入实体时会自动执行闭包并移除自身。
/// - A wrapper component that allows deferred state machine creation via a closure.
///   Executes the closure automatically on insert and removes itself.
///
/// ## Example
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn setup(mut commands: Commands) {
/// commands.spawn(SpawnStateMachine::new(|entity| {
///     entity.insert(HsmStateMachine::with(
///         entity.id(), entity.id(),
///         #[cfg(feature = "history")] 10,
///     ));
/// }));
/// # }
/// ```
#[derive(Component)]
#[allow(clippy::type_complexity)]
#[component(on_insert=Self::on_insert)]
pub struct SpawnStateMachine(
    Option<Box<dyn for<'w> FnOnce(&'w mut EntityWorldMut<'w>) + 'static + Send + Sync>>,
);

impl SpawnStateMachine {
    /// 创建一个新的 [`SpawnStateMachine`]，传入一个在插入时执行的闭包。
    ///
    /// Creates a new [`SpawnStateMachine`] with a closure that runs on insert.
    pub fn new<F>(f: F) -> Self
    where
        F: for<'w> FnOnce(&'w mut EntityWorldMut<'w>) + 'static + Send + Sync,
    {
        Self(Some(Box::new(f)))
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        world.commands().queue(move |world: &mut World| {
            let mut e_ref = world.entity_mut(entity);
            let Some(mut sp) = e_ref.get_mut::<SpawnStateMachine>() else {
                return;
            };
            let Some(f) = sp.0.take() else {
                return;
            };
            e_ref.remove::<Self>();
            (f)(&mut world.entity_mut(entity));
        });
    }
}
