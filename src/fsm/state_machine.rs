#[cfg(feature = "hybrid")]
use crate::fsm::hybrid::NestedFsm;
use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{
    context::*,
    error::{StateMachineError, error_event, error_event_world, trace_event},
    fsm::{event::FsmTrigger, graph::FsmGraph},
    guards::GuardCondition,
    interrupt::{InterruptFrame, InterruptStack},
    markers::Paused,
    prelude::{ActionDispatch, FsmTriggerType, GetBufferId, GuardRegistry, StateActionBuffer},
    state_actions::*,
};

#[cfg(feature = "state_data")]
use crate::state_data::StateScenePatch;

use crate::fsm::history::{FsmHistoricalNode, FsmStateHistory};

/// # 预解析转换\Pre-resolved Transition
/// * 保存在执行状态转换前预解析的系统 ID 和上下文。将耗时的查找操作前置，使实际转换执行更高效。
/// - Holds pre-resolved system IDs and context for executing a state transition.
///   Front-loads expensive lookups so the actual transition execution is more efficient.
struct ResolvedTransition {
    remove_buffer: Option<(GetBufferId, ActionContext)>,
    exit_action: Option<(ActionId, ActionContext)>,
    after_exit: Option<(TransitionId, TransitionContext)>,
    before_enter: Option<(TransitionId, TransitionContext)>,
    enter_action: Option<(ActionId, ActionContext)>,
    add_buffer: Option<(GetBufferId, ActionContext)>,
    to: Entity,
    state_machine_id: Entity,
    service_target: Entity,
}

/// Executes all transition lifecycle steps on a [`World`].
///
/// The steps are:
/// 1. Remove from update buffer (from_state)
/// 2. Run exit action (from_state)
/// 3. Reclaim state scene (from_state, if `state_data` feature)
/// 4. Run after_exit transition (from_state)
/// 5. Set current state to `to`
/// 6. Run before_enter transition (to_state)
/// 7. Apply state scene (to_state, if `state_data` feature)
/// 8. Run enter action (to_state)
/// 9. Add to update buffer (to_state)
fn execute_transition_steps(
    world: &mut World,
    resolved: ResolvedTransition,
    new_graph_id: Option<Entity>,
) -> bevy::prelude::Result<()> {
    let ResolvedTransition {
        remove_buffer,
        exit_action,
        after_exit,
        before_enter,
        enter_action,
        add_buffer,
        to,
        state_machine_id,
        service_target,
    } = resolved;
    if let Some((get_buff_id, ctx)) = remove_buffer {
        (get_buff_id)(
            world,
            Box::new({
                move |buffer: &mut StateActionBuffer| {
                    buffer.remove_interceptor(ctx);
                    buffer.add_filter(ctx);
                }
            }),
        );
    }

    if let Some((id, ref ctx)) = exit_action {
        ctx.queue_system_command(id).apply(world)?;

        #[cfg(feature = "state_data")]
        StateScenePatch::reclaim_state_scene_command(ctx.state(), state_machine_id, service_target)
            .apply(world);
    }

    if let Some((id, ctx)) = after_exit {
        ctx.queue_system_command(id).apply(world)?;
    }

    let Some(mut sm) = world.get_mut::<FsmStateMachine>(state_machine_id) else {
        error_event_world(
            world,
            state_machine_id,
            StateMachineError::FsmStateMachineMissing(state_machine_id),
        );
        return Ok(());
    };
    if let Some(gid) = new_graph_id {
        sm.graph_id = gid;
    }
    sm.set_curr_state(to);

    if let Some((id, ctx)) = before_enter {
        ctx.queue_system_command(id).apply(world)?;
    }

    if let Some((id, ref ctx)) = enter_action {
        #[cfg(feature = "state_data")]
        if let Some(patch) = world.get::<StateScenePatch>(ctx.state()).cloned() {
            patch
                .apply_state_scene_command(ctx.state(), state_machine_id, service_target)
                .apply(world);
        }
        ctx.queue_system_command(id).apply(world)?;
    }

    if let Some((get_buff_id, ctx)) = add_buffer {
        (get_buff_id)(
            world,
            Box::new({
                move |buffer: &mut StateActionBuffer| {
                    buffer.add(ctx);
                }
            }),
        );
    }

    Ok(())
}

/// # FSM 状态机
/// * 一个有限状态机（FSM）的运行时实例。
///
/// 该组件负责跟踪一个具体状态机的当前状态 (`curr_state`)。每个 [`FsmStateMachine`] 都必须关联到一个
/// 定义了其拓扑结构的 [`FsmGraph`]。
///
/// 多个 [`FsmStateMachine`] 实例可以共享同一个 [`FsmGraph`]，从而允许创建多个行为相同但状态独立的"智能体"。
///
/// 它的 `on_insert` 和 `on_remove` 钩子负责处理进入初始状态和在状态机被销毁时进行清理的逻辑。
///
/// # FSM State Machine
/// * A runtime instance of a Finite State Machine (FSM).
///
/// This component is responsible for tracking the current state (`curr_state`) of a specific state machine.
/// Each [`FsmStateMachine`] must be associated with a [`FsmGraph`] that defines its topology.
///
/// Multiple [`FsmStateMachine`] instances can share the same [`FsmGraph`], allowing for the creation of
/// multiple "agents" that have the same behavior but independent states.
///
/// Its `on_insert` and `on_remove` hooks handle the logic for entering the initial state and
/// cleaning up when the state machine is destroyed.
#[derive(Component)]
#[component(on_insert = Self::on_insert,on_remove = Self::on_remove)]
pub struct FsmStateMachine {
    /// 包含状态机拓扑 ([`FsmGraph`]) 的实体。
    /// The entity that holds the state machine's topology ([`FsmGraph`]).
    graph_id: Entity,
    /// 状态机的初始状态，在创建时从图中复制。
    /// The initial state of the state machine, copied from the graph upon creation.
    pub(super) init_state: Entity,
    /// 此状态机实例当前所处的活动状态。
    /// The currently active state for this state machine instance.
    pub(super) curr_state: Entity,
    /// 中断栈，保存被中断的状态图和状态以便后续恢复。支持嵌套中断。
    /// Interrupt stack that saves interrupted state graphs and states for later resume. Supports nested interrupts.
    pub(super) interrupt_stack: InterruptStack,
    /// (当 `history` 特性启用时) 跟踪此状态机访问过的状态历史。
    /// (When the `history` feature is enabled) Tracks the history of visited states for this state machine.
    #[cfg(feature = "history")]
    pub history: FsmStateHistory,
}

impl FsmStateMachine {
    pub fn new(
        graph_id: Entity,
        init_state: Entity,
        curr_state: Entity,
        #[cfg(feature = "history")] history_size: usize,
    ) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state,
            interrupt_stack: InterruptStack::default(),
            #[cfg(feature = "history")]
            history: FsmStateHistory::new(history_size),
        }
    }

    pub fn with(
        graph_id: Entity,
        init_state: Entity,
        #[cfg(feature = "history")] history_size: usize,
    ) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state: init_state,
            interrupt_stack: InterruptStack::default(),
            #[cfg(feature = "history")]
            history: FsmStateHistory::new(history_size),
        }
    }

    pub const fn graph_id(&self) -> Entity {
        self.graph_id
    }

    pub const fn curr_state_id(&self) -> Entity {
        self.curr_state
    }

    pub const fn init_state_id(&self) -> Entity {
        self.init_state
    }

    /// 设置当前状态, 并记录历史
    ///
    /// Set current state and record history
    pub fn set_curr_state(&mut self, state: Entity) {
        #[cfg(feature = "history")]
        self.history.push(FsmHistoricalNode::new(
            state,
            self.graph_id,
            self.interrupt_stack.interrupt_depth(),
        ));
        self.curr_state = state;
    }

    /// 将当前状态图和状态压入中断栈，通常在触发中断前调用。
    ///
    /// Push the current state graph and state onto the interrupt stack.
    #[inline]
    pub fn push_interrupt(&mut self, graph_id: Entity, state: Entity) {
        self.interrupt_stack.push_interrupt(graph_id, state);
    }

    /// 从中断栈中弹出最近保存的状态帧，用于恢复。
    ///
    /// Pop the most recently saved interrupt frame from the interrupt stack for resume.
    #[inline]
    pub fn pop_interrupt(&mut self) -> Option<InterruptFrame> {
        self.interrupt_stack.pop_interrupt()
    }

    /// 检查状态机是否处于中断状态（即中断栈非空）。
    ///
    /// Check whether the state machine is currently interrupted.
    #[inline]
    pub fn is_interrupted(&self) -> bool {
        self.interrupt_stack.is_interrupted()
    }

    /// 返回当前中断嵌套深度。
    ///
    /// Returns the current interrupt nesting depth.
    #[inline]
    pub fn interrupt_depth(&self) -> usize {
        self.interrupt_stack.interrupt_depth()
    }

    /// 清空中断栈，放弃所有待恢复的状态。
    ///
    /// Clear the interrupt stack, abandoning all pending resumes.
    #[inline]
    pub fn clear_interrupt_stack(&mut self) {
        self.interrupt_stack.clear_interrupt_stack();
    }

    #[cfg(feature = "history")]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        #[cfg(feature = "history")]
        let Some(mut fsm_state_machine) = world.get_mut::<FsmStateMachine>(entity) else {
            let err = StateMachineError::FsmStateMachineMissing(entity);
            error!("{}", err);
            return;
        };
        #[cfg(not(feature = "history"))]
        let Some(fsm_state_machine) = world.get::<FsmStateMachine>(entity) else {
            let err = StateMachineError::FsmStateMachineMissing(entity);
            error!("{}", err);
            return;
        };

        let curr_state = fsm_state_machine.curr_state_id();

        #[cfg(feature = "history")]
        {
            let graph_id = fsm_state_machine.graph_id;
            let depth = fsm_state_machine.interrupt_stack.interrupt_depth();
            fsm_state_machine
                .history
                .push(FsmHistoricalNode::new(curr_state, graph_id, depth));
        }
        let service_target = get_service_target(&world, entity);

        if let Some(id) =
            TransitionRegistry::get_transition_id::<BeforeEnterSystem>(&world, curr_state)
        {
            let context = TransitionContext::with_initial(service_target, entity, curr_state);
            context.run_system(&mut world, id);
        }

        #[cfg(feature = "state_data")]
        StateScenePatch::spawn_state_scene(&mut world, curr_state, entity, service_target);

        let context = ActionContext::new(service_target, entity, curr_state);

        if let Some(id) = ActionRegistry::get_action_id::<AfterEnterSystem>(&world, curr_state) {
            context.run_system(&mut world, id);
        }

        StateActionBuffer::buffer_scope(
            world.as_unsafe_world_cell(),
            curr_state,
            move |buffer: &mut StateActionBuffer| {
                buffer.add(context);
            },
        );
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(fsm_state_machine) = world.get::<FsmStateMachine>(entity) else {
            let err = StateMachineError::FsmStateMachineMissing(entity);
            error!("{}", err);
            return;
        };

        let curr_state = fsm_state_machine.curr_state_id();
        let service_target = get_service_target(&world, entity);

        let context = ActionContext::new(service_target, entity, curr_state);

        if let Some(id) = ActionRegistry::get_action_id::<BeforeExitSystem>(&world, curr_state) {
            context.run_system(&mut world, id);
        }

        #[cfg(feature = "state_data")]
        StateScenePatch::reclaim_state_scene(&mut world, curr_state, entity, service_target);

        if let Some(id) =
            TransitionRegistry::get_transition_id::<AfterExitSystem>(&world, curr_state)
        {
            let context = TransitionContext::with_final(service_target, entity, curr_state);
            context.run_system(&mut world, id);
        }

        StateActionBuffer::buffer_scope(
            world.as_unsafe_world_cell(),
            curr_state,
            move |buffer: &mut StateActionBuffer| {
                buffer.remove_interceptor(context);
                buffer.add_filter(context);
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_fsm_trigger(
        mut on: On<FsmTrigger>,
        mut commands: Commands,
        guard_registry: Res<GuardRegistry>,
        fsm_graph: Query<&FsmGraph>,
        query: Query<&FsmStateMachine, Without<Paused>>,
    ) {
        let FsmTrigger {
            state_machine,
            typed,
        } = on.event_mut();
        let state_machine_id = *state_machine;

        let Ok(state_machine) = query.get(state_machine_id) else {
            error_event(
                &mut commands,
                state_machine_id,
                StateMachineError::FsmStateMachineMissing(state_machine_id),
            );
            return;
        };
        let Ok(fsm_graph) = fsm_graph.get(state_machine.graph_id) else {
            error_event(
                &mut commands,
                state_machine_id,
                StateMachineError::GraphMissing(state_machine.graph_id),
            );
            return;
        };
        let Some(state_transitions) = fsm_graph.get(state_machine.curr_state_id()) else {
            error_event(
                &mut commands,
                state_machine_id,
                StateMachineError::StateNotInGraph {
                    graph: state_machine.graph_id,
                    state: state_machine.curr_state_id(),
                },
            );
            return;
        };

        match typed {
            FsmTriggerType::Guard(target) => {
                if let Some(guard) = state_transitions.get_by_guard(*target) {
                    Self::handle_guard_transition(
                        &mut commands,
                        &guard_registry,
                        state_machine_id,
                        guard,
                        *target,
                    );
                } else {
                    trace_event(
                        &mut commands,
                        state_machine_id,
                        StateMachineError::InvalidTransitionTarget {
                            graph: state_machine.graph_id,
                            from_state: state_machine.curr_state_id(),
                            to_state: *target,
                        },
                    );
                }
            }
            FsmTriggerType::Next(target) => {
                if state_transitions.contains(*target) {
                    Self::handle_direct_transition(&mut commands, state_machine_id, *target);
                } else {
                    trace_event(
                        &mut commands,
                        state_machine_id,
                        StateMachineError::InvalidTransitionTarget {
                            graph: state_machine.graph_id,
                            from_state: state_machine.curr_state_id(),
                            to_state: *target,
                        },
                    );
                }
            }
            FsmTriggerType::Event(fsm_on_event) => {
                if let Some(target) = fsm_on_event.get_target(state_transitions) {
                    Self::handle_direct_transition(&mut commands, state_machine_id, target);
                } else {
                    trace_event(
                        &mut commands,
                        state_machine_id,
                        StateMachineError::NoMatchingEventTransition {
                            graph: state_machine.graph_id,
                            state: state_machine.curr_state_id(),
                        },
                    );
                }
            }
            FsmTriggerType::Interrupt(target_graph, target_state) => {
                let target_graph = *target_graph;
                let target_state = *target_state;
                if target_state == state_machine.curr_state_id()
                    && target_graph == state_machine.graph_id
                {
                    trace!("Self-interrupt: already in target state, skipping");
                    return;
                }
                let save_graph = state_machine.graph_id;
                let save_state = state_machine.curr_state_id();
                commands.queue(move |world: &mut World| -> bevy::prelude::Result<()> {
                    let from = match world.get::<FsmStateMachine>(state_machine_id) {
                        Some(sm) => sm.curr_state_id(),
                        None => return Ok(()),
                    };
                    if let Some(mut sm) = world.get_mut::<FsmStateMachine>(state_machine_id) {
                        sm.push_interrupt(save_graph, save_state);
                        sm.graph_id = target_graph;
                    }
                    let resolved =
                        resolve_transition_in_world(world, from, target_state, state_machine_id);
                    execute_transition_steps(world, resolved, None)
                });
            }
            FsmTriggerType::Resume => {
                commands.queue(move |world: &mut World| -> bevy::prelude::Result<()> {
                    // Peek first — validate before consuming the interrupt frame
                    let frame = {
                        let Some(sm) = world.get::<FsmStateMachine>(state_machine_id) else {
                            return Ok(());
                        };
                        match sm.interrupt_stack.peek_interrupt() {
                            Some(frame) => frame,
                            None => {
                                error_event_world(
                                    world,
                                    state_machine_id,
                                    StateMachineError::InterruptStackEmpty(state_machine_id),
                                );
                                return Ok(());
                            }
                        }
                    };

                    let from = match world.get::<FsmStateMachine>(state_machine_id) {
                        Some(sm) => sm.curr_state_id(),
                        None => return Ok(()),
                    };

                    if from == frame.state_id && {
                        let Some(sm) = world.get::<FsmStateMachine>(state_machine_id) else {
                            return Ok(());
                        };
                        sm.graph_id == frame.graph_id
                    } {
                        trace!("Resume to same state and graph, skipping");
                        if let Some(mut sm) = world.get_mut::<FsmStateMachine>(state_machine_id) {
                            sm.pop_interrupt();
                        }
                        return Ok(());
                    }

                    // All valid — now safe to pop.
                    // pop_interrupt() only removes the top frame; it does not restore
                    // graph_id. So sm.graph_id is still the graph set during the interrupt.
                    // We compare it against the saved frame.graph_id to decide whether
                    // execute_transition_steps needs to restore the original graph.
                    let new_graph_id = {
                        let Some(mut sm) = world.get_mut::<FsmStateMachine>(state_machine_id)
                        else {
                            return Ok(());
                        };
                        sm.pop_interrupt();
                        (sm.graph_id != frame.graph_id).then_some(frame.graph_id)
                    };

                    let to = frame.state_id;
                    let resolved = resolve_transition_in_world(world, from, to, state_machine_id);
                    execute_transition_steps(world, resolved, new_graph_id)
                });
            }
        };
    }

    fn handle_direct_transition(commands: &mut Commands, state_machine_id: Entity, to: Entity) {
        commands.queue(move |world: &mut World| -> bevy::prelude::Result<()> {
            let from = match world.get::<FsmStateMachine>(state_machine_id) {
                Some(sm) => sm.curr_state_id(),
                None => return Ok(()),
            };
            let resolved = resolve_transition_in_world(world, from, to, state_machine_id);
            execute_transition_steps(world, resolved, None)
        });
    }

    fn handle_guard_transition(
        commands: &mut Commands,
        guard_registry: &GuardRegistry,
        state_machine_id: Entity,
        guard: &GuardCondition,
        target: Entity,
    ) {
        let id = match guard_registry.to_combinator_condition_id(guard) {
            Ok(id) => id,
            Err(e) => {
                warn!("{}", e);
                error_event(
                    commands,
                    state_machine_id,
                    StateMachineError::GuardNotFound {
                        condition: format!("{:?}", guard),
                        target,
                    },
                );
                return;
            }
        };

        commands.queue(move |world: &mut World| -> bevy::prelude::Result<()> {
            let from = match world.get::<FsmStateMachine>(state_machine_id) {
                Some(sm) => sm.curr_state_id(),
                None => return Ok(()),
            };
            let service_target = resolve_service_target_in_world(world, state_machine_id);
            let context = GuardContext::new(service_target, state_machine_id, from, target);
            if !id.run(world, context)? {
                return Ok(());
            }
            let resolved = resolve_transition_in_world(world, from, target, state_machine_id);
            execute_transition_steps(world, resolved, None)
        });
    }
}

#[inline]
fn resolve_service_target_in_world(world: &World, state_machine_id: Entity) -> Entity {
    #[cfg(feature = "hybrid")]
    if let Some(child_of) = world.get::<NestedFsm>(state_machine_id) {
        return child_of.state_machine;
    }
    get_service_target(world, state_machine_id)
}

/// 通过直接查询 [`World`] 来解析 [`ResolvedTransition`]。将状态转换所需的所有系统 ID
/// 和上下文预先查找出来，用于排队的命令中（如直接转换、守卫转换和恢复操作）。
///
/// Resolves a [`ResolvedTransition`] by querying the [`World`] directly.
/// Used inside queued commands (e.g., for direct transitions, guard transitions, and resume).
fn resolve_transition_in_world(
    world: &mut World,
    from: Entity,
    to: Entity,
    state_machine_id: Entity,
) -> ResolvedTransition {
    let service_target = resolve_service_target_in_world(world, state_machine_id);

    let get_buffer_id = |state: Entity| -> Option<GetBufferId> {
        let update = world.get::<OnUpdateSystem>(state)?;
        world.resource::<ActionDispatch>().get(update)
    };
    let get_enter_action_id = |state: Entity| -> Option<ActionId> {
        let enter = world.get::<AfterEnterSystem>(state)?;
        world.resource::<ActionRegistry>().get(enter)
    };
    let get_exit_action_id = |state: Entity| -> Option<ActionId> {
        let exit = world.get::<BeforeExitSystem>(state)?;
        world.resource::<ActionRegistry>().get(exit)
    };
    let get_enter_transition_id = |state: Entity| -> Option<TransitionId> {
        let enter = world.get::<BeforeEnterSystem>(state)?;
        world.resource::<TransitionRegistry>().get(enter)
    };
    let get_exit_transition_id = |state: Entity| -> Option<TransitionId> {
        let exit = world.get::<AfterExitSystem>(state)?;
        world.resource::<TransitionRegistry>().get(exit)
    };

    ResolvedTransition {
        remove_buffer: get_buffer_id(from).map(|id| {
            (
                id,
                ActionContext::new(service_target, state_machine_id, from),
            )
        }),
        exit_action: get_exit_action_id(from).map(|id| {
            (
                id,
                ActionContext::new(service_target, state_machine_id, from),
            )
        }),
        after_exit: get_exit_transition_id(from).map(|id| {
            (
                id,
                TransitionContext::with_transition(service_target, state_machine_id, from, to),
            )
        }),
        before_enter: get_enter_transition_id(to).map(|id| {
            (
                id,
                TransitionContext::with_transition(service_target, state_machine_id, from, to),
            )
        }),
        enter_action: get_enter_action_id(to)
            .map(|id| (id, ActionContext::new(service_target, state_machine_id, to))),
        add_buffer: get_buffer_id(to)
            .map(|id| (id, ActionContext::new(service_target, state_machine_id, to))),
        to,
        state_machine_id,
        service_target,
    }
}

/// # FSM 蓝图
/// * 一个用于配置和创建 [`FsmStateMachine`] 实例的数据结构。
///
/// 这不是一个组件，而是一个普通的结构体，用作数据传输对象（DTO）。
/// 它的主要用途是在更复杂的结构中（例如 `HsmState`）定义一个嵌套的 FSM，
/// 允许在创建时精确控制 FSM 的初始状态和配置。
///
/// # FSM Blueprint
/// * A data structure for configuring and creating an [`FsmStateMachine`] instance.
///
/// This is not a component but a plain struct that acts as a Data Transfer Object (DTO).
/// Its primary use is to define a nested FSM within more complex structures (e.g., an `HsmState`),
/// allowing for precise control over the FSM's initial state and configuration upon creation.
#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsmBlueprint {
    /// FSM 将要使用的图实体。
    /// The graph entity that the FSM will use.
    pub graph_id: Entity,
    /// 可选的当前状态。如果设置了此值，状态机将从这个状态开始，而不是 `init_state`。
    /// Optional current state. If this is set, the state machine will start in this state instead of `init_state`.
    pub curr_state: Option<Entity>,
    #[cfg(feature = "history")]
    /// 状态历史记录大小（当 `history` 特性启用时）。
    /// The size of the state history (when the `history` feature is enabled).
    pub history_size: usize,
}

impl FsmBlueprint {
    pub fn new(graph_id: Entity, #[cfg(feature = "history")] history_size: usize) -> Self {
        Self {
            graph_id,
            curr_state: None,
            #[cfg(feature = "history")]
            history_size,
        }
    }

    pub fn with_curr_state(mut self, curr_state: Entity) -> Self {
        self.curr_state = Some(curr_state);
        self
    }
}

#[cfg(test)]
#[path = "../tests/fsm_state_machine_tests.rs"]
mod tests;
