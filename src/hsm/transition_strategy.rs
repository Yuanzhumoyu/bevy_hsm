use std::{any::type_name, fmt::Debug, sync::Arc};

use bevy::{ecs::schedule::ScheduleLabel, platform::collections::HashSet, prelude::*};

use crate::{
    context::GuardContext,
    error::StateMachineError,
    hsm::{
        HsmState,
        state_machine::{Transition, *},
        state_tree::StateTree,
    },
    markers::*,
    prelude::{GuardEnter, GuardEnterCache, GuardExit, GuardExitCache, HsmStateId, ServiceTarget},
};

/// 状态转换策略，用于控制状态转换行为
///
/// State transition strategy, used to control state transition behavior
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateTransitionStrategy {
    /// 子状态嵌套转换：父状态保持激活，子状态进入和退出发生在父状态内部
    ///
    /// Sub state nested transition: The parent state remains active, and the sub state enters and exits occur within the parent state
    /// ```toml
    ///    super_state: after_enter
    ///    sub_state: after_enter
    ///    sub_state: before_exit
    ///    super_state: before_exit
    /// ```
    #[default]
    Nested,
    /// 平级转换：父状态先退出，然后子状态进入和退出，最后可能重新进入父状态
    ///
    /// Level-to-level transition: The parent state exits first, followed by the entry and exit of the child state, and finally, the parent state may be re-entered
    /// ```toml
    ///    super_state: after_enter
    ///    super_state: before_exit
    ///    sub_state: after_enter
    ///    sub_state: before_exit
    /// ```
    Parallel,
}

impl StateTransitionStrategy {
    pub fn is_nested(&self) -> bool {
        matches!(self, Self::Nested)
    }

    pub fn is_parallel(&self) -> bool {
        matches!(self, Self::Parallel)
    }
}

/// # 退出过渡状态行为\Exit Transition Behavior
///
/// * 用于定义状态在退出时的行为，包括重生、复活和死亡
/// - Used to define the behavior of a state when exiting, including rebirth, resurrection, and death
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitTransitionBehavior {
    /// # 重生\Rebirth
    ///
    /// 从sub_state退出后，重新进入super_state的enter阶段
    ///
    /// From sub_state exit, re-enter the super_state's after_enter phase
    Rebirth,
    /// # 复活\Resurrection
    ///
    /// 从sub_state退出后，进入super_state的update阶段
    ///
    /// From sub_state exit, enter the super_state's on_update phase
    #[default]
    Resurrection,
    /// # 死亡\Death
    ///
    /// 从sub_state退出后，不再进入super_state, 而是向上层状态继续判断[ExitTransitionBehavior]和[StateTransitionStrategy]
    ///
    /// From sub_state exit, do not enter super_state, but continue to judge [ExitTransitionBehavior] and [StateTransitionStrategy] to the upper state
    Death,
}

impl From<ExitTransitionBehavior> for StateLifecycle {
    fn from(value: ExitTransitionBehavior) -> Self {
        match value {
            ExitTransitionBehavior::Rebirth => StateLifecycle::Enter,
            ExitTransitionBehavior::Resurrection => StateLifecycle::Update,
            ExitTransitionBehavior::Death => StateLifecycle::Exit,
        }
    }
}

impl From<StateLifecycle> for ExitTransitionBehavior {
    fn from(value: StateLifecycle) -> Self {
        match value {
            StateLifecycle::Enter => ExitTransitionBehavior::Rebirth,
            StateLifecycle::Update => ExitTransitionBehavior::Resurrection,
            StateLifecycle::Exit => ExitTransitionBehavior::Death,
        }
    }
}

/// 一个用于定义子状态应如何遍历的 trait。
///
/// 此 trait 的实现将决定子状态在激活或其他操作中被考虑的顺序。
pub trait StateTraversalStrategy: Send + Sync + 'static {
    /// 给定一个子状态实体列表，按照期望的遍历顺序返回它们。
    fn traverse(&self, world: &World, children: &[Entity]) -> Vec<Entity>;

    /// 返回遍历策略的名称。
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }
}

/// 一个包装结构体，用于持有动态的 `StateTraversalStrategy`。
///
/// 这允许在运行时互换使用不同的遍历策略。
pub struct TraversalStrategy(pub(crate) Arc<dyn StateTraversalStrategy>);

impl TraversalStrategy {
    /// 使用给定的实现创建一个新的 `TraversalStrategy`。
    pub fn new<T: StateTraversalStrategy>(strategy: T) -> Self {
        Self(Arc::new(strategy))
    }
}

impl Eq for TraversalStrategy {}

impl PartialEq for TraversalStrategy {
    fn eq(&self, other: &Self) -> bool {
        self.0.name() == other.0.name()
    }
}

impl Clone for TraversalStrategy {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl Default for TraversalStrategy {
    fn default() -> Self {
        Self(Arc::new(SequentialTraversal))
    }
}

impl Debug for TraversalStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name())
    }
}

/// 一个基本的顺序遍历策略。
///
/// 此策略简单地按照提供的顺序返回子状态。
pub struct SequentialTraversal;

impl StateTraversalStrategy for SequentialTraversal {
    fn traverse(&self, _world: &World, children: &[Entity]) -> Vec<Entity> {
        children.to_vec()
    }
}

/// 一个基本的逆序遍历策略
///
/// 此策略简单地按照提供的逆序返回子状态。
pub struct ReverseTraversal;

impl StateTraversalStrategy for ReverseTraversal {
    fn traverse(&self, _world: &World, children: &[Entity]) -> Vec<Entity> {
        children.iter().rev().cloned().collect()
    }
}

fn build_exit_transition_plan(
    world: &World,
    mut state_id: HsmStateId,
    strategy: StateTransitionStrategy,
    mut behavior: ExitTransitionBehavior,
) -> Result<Vec<Transition>, String> {
    match (strategy, behavior) {
        (
            StateTransitionStrategy::Nested | StateTransitionStrategy::Parallel,
            ExitTransitionBehavior::Resurrection,
        ) => Ok(vec![Transition::Update(state_id)]),
        (
            StateTransitionStrategy::Nested | StateTransitionStrategy::Parallel,
            ExitTransitionBehavior::Rebirth,
        ) => Ok(vec![Transition::Enter(state_id)]),
        (StateTransitionStrategy::Nested, ExitTransitionBehavior::Death) => {
            let Some(state_tree) = world.get::<StateTree>(state_id.tree()) else {
                return Err(format!(
                    "The entity<{}> does not contain [StateTree]",
                    state_id
                ));
            };

            let mut transition_queue = vec![Transition::Exit(state_id)];

            if state_tree.get_root() == state_id.state() {
                return Ok(transition_queue);
            }

            while let Some(state) = state_tree.get_super_state(state_id.state()) {
                let Some(HsmState {
                    strategy, behavior, ..
                }) = world.get::<HsmState>(state).copied()
                else {
                    return Err(format!(
                        "The entity<{}> does not contain [HsmState]",
                        state_id
                    ));
                };
                let state_id = HsmStateId::new(state_id.tree(), state);
                if state_tree.get_root() == state_id.state() {
                    transition_queue.push(Transition::with_behavior(state_id, behavior));
                    return Ok(transition_queue);
                }
                match !(strategy == StateTransitionStrategy::Nested
                    && behavior == ExitTransitionBehavior::Death)
                {
                    true => {
                        transition_queue.extend(build_exit_transition_plan(
                            world, state_id, strategy, behavior,
                        )?);
                        return Ok(transition_queue);
                    }
                    false => {
                        transition_queue.push(Transition::Exit(state_id));
                    }
                }
            }
            Ok(transition_queue)
        }
        (StateTransitionStrategy::Parallel, ExitTransitionBehavior::Death) => {
            let Some(state_tree) = world.get::<StateTree>(state_id.tree()) else {
                return Err(format!(
                    "The entity<{}> does not contain [StateTree]",
                    state_id
                ));
            };

            while let Some(state) = state_tree.get_super_state(state_id.state())
                && let Some(HsmState {
                    strategy,
                    behavior: new_behavior,
                    ..
                }) = world.get::<HsmState>(state).copied()
            {
                let new_state_id = HsmStateId::new(state_id.tree(), state);
                if !(strategy == StateTransitionStrategy::Parallel
                    && new_behavior == ExitTransitionBehavior::Death)
                {
                    return build_exit_transition_plan(world, new_state_id, strategy, new_behavior);
                }
                state_id = new_state_id;
                behavior = new_behavior;
            }
            match behavior {
                ExitTransitionBehavior::Rebirth => Ok(vec![Transition::Enter(state_id)]),
                ExitTransitionBehavior::Resurrection => Ok(vec![Transition::Update(state_id)]),
                ExitTransitionBehavior::Death => Ok(vec![Transition::End]),
            }
        }
    }
}

/// 检查能否过渡状态的实体
///
/// Check whether the entity can transition
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq, Deref, DerefMut)]
pub(crate) struct CheckOnTransitionStates(HashSet<Entity>);

/// 在指定的调度中安装状态转换系统。
///
/// # Arguments
///
/// * `app` - Bevy 应用实例。
/// * `schedule` - 要安装系统的调度标签。
pub(crate) fn install_transition_systems<T: ScheduleLabel>(app: &mut App, schedule: T) {
    app.add_systems(
        schedule,
        (handle_enter_transitions, handle_exit_transitions)
            .chain()
            .run_if(|check_on_transition_states: Res<CheckOnTransitionStates>| {
                !check_on_transition_states.is_empty()
            }),
    );
}

fn handle_enter_transitions(
    mut commands: Commands,
    check_on_transition_states: Res<CheckOnTransitionStates>,
    query_state_machines: Query<(Entity, &HsmStateMachine), Without<Paused>>,
    query_states: Query<&HsmState, With<HsmState>>,
) {
    for (state_machine_id, state_machine) in
        query_state_machines.iter_many(check_on_transition_states.iter())
    {
        let curr_state_id = state_machine.curr_state_id();
        let Ok(strategy) = query_states
            .get(curr_state_id.state())
            .map(|hsm_state| hsm_state.strategy)
        else {
            continue;
        };
        commands.queue(move |world: &mut World| {
            let Some(state_tree) = world.get::<StateTree>(curr_state_id.tree()) else {
                warn!(
                    "{}",
                    StateMachineError::StateTreeNotFound(curr_state_id.tree())
                );
                return;
            };
            let sub_state_iter =
                state_tree.traversal_iter_with(world, curr_state_id.state(), |e| {
                    if !e.contains::<HsmState>() {
                        warn!("{}", StateMachineError::HsmStateMissing(e.id()));
                        return false;
                    }
                    e.contains::<GuardEnter>()
                });
            let Some(enter_state_id) = world.resource_scope(
                |world: &mut World, condition_buffer: Mut<GuardEnterCache>| {
                    for sub_state_id in sub_state_iter {
                        let Some(condition_id) = condition_buffer.get(&sub_state_id) else {
                            continue;
                        };

                        let service_target = get_service_target(world, state_machine_id);
                        match condition_id.run(
                            world,
                            GuardContext::new(
                                service_target,
                                state_machine_id,
                                curr_state_id.state(),
                                sub_state_id,
                            ),
                        ) {
                            Ok(true) => return Some(sub_state_id),
                            Ok(false) => continue,
                            Err(e) => {
                                error!(
                                    "{}",
                                    StateMachineError::GuardRunFailed {
                                        state_machine: state_machine_id,
                                        from_state: curr_state_id.state(),
                                        to_state: Some(sub_state_id),
                                        source: e.into(),
                                    }
                                );
                                continue;
                            }
                        }
                    }
                    None
                },
            ) else {
                return;
            };

            let _ =
                handle_enter_transition(state_machine_id, curr_state_id, enter_state_id, strategy)
                    .apply(world);
        });
    }
}

pub(super) fn handle_enter_transition(
    state_machine_id: Entity,
    curr_state_id: HsmStateId,
    enter_state_id: Entity,
    strategy: StateTransitionStrategy,
) -> impl Command<Result<()>> {
    move |world: &mut World| {
        world
            .resource_mut::<CheckOnTransitionStates>()
            .remove(&state_machine_id);

        let mut service_target = world.entity_mut(state_machine_id);
        let Some(mut state_machine) = service_target.get_mut::<HsmStateMachine>() else {
            warn!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return Ok(());
        };

        let state_id = HsmStateId::new(curr_state_id.tree(), enter_state_id);
        let next_on_state: StateLifecycle = match strategy {
            StateTransitionStrategy::Nested => {
                state_machine.set_curr_state(state_id);
                StateLifecycle::Enter
            }
            StateTransitionStrategy::Parallel => {
                state_machine.set_curr_state(curr_state_id);
                state_machine.push_next_state(Transition::Enter(state_id));
                StateLifecycle::Exit
            }
        };

        service_target.insert(next_on_state);
        Ok(())
    }
}

fn handle_exit_transitions(
    mut commands: Commands,
    check_on_transition_states: Res<CheckOnTransitionStates>,
    query_state_machines: Query<(Entity, &HsmStateMachine), Without<Paused>>,
    query_on_exit_conditions: Query<Has<GuardExit>, With<HsmState>>,
    query_state_trees: Query<&StateTree>,
) {
    // 条件为空的状态
    for (state_machine_id, state_machine) in
        query_state_machines.iter_many(check_on_transition_states.iter())
    {
        let curr_state_id = state_machine.curr_state_id();
        let Ok(true) = query_on_exit_conditions.get(curr_state_id.state()) else {
            continue;
        };
        let Ok(state_tree) = query_state_trees.get(curr_state_id.tree()) else {
            warn!(
                "{}",
                StateMachineError::StateTreeNotFound(curr_state_id.tree())
            );
            continue;
        };
        let Some(super_state_id) = state_tree.get_super_state(curr_state_id.state()) else {
            warn!(
                "{}",
                StateMachineError::SuperStateNotFound {
                    state_tree: curr_state_id.tree(),
                    state: curr_state_id.state()
                }
            );
            continue;
        };
        commands.queue(move |world: &mut World| -> Result<()> {
            match world.resource_scope(
                |world: &mut World, exit_guard_cache: Mut<GuardExitCache>| match exit_guard_cache
                    .get(&curr_state_id.state())
                {
                    Some(guard) => {
                        let service_target = get_service_target(world, state_machine_id);
                        guard.run(
                            world,
                            GuardContext::new(
                                service_target,
                                state_machine_id,
                                curr_state_id.state(),
                                super_state_id,
                            ),
                        )
                    }
                    None => Ok(false),
                },
            ) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(e) => {
                    error!(
                        "{}",
                        StateMachineError::GuardRunFailed {
                            state_machine: state_machine_id,
                            from_state: curr_state_id.state(),
                            to_state: None,
                            source: e.into(),
                        }
                    );
                    return Ok(());
                }
            };

            handle_exit_transition(state_machine_id, curr_state_id, super_state_id).apply(world)
        });
    }
}

#[inline]
pub(super) fn handle_exit_transition(
    state_machine_id: Entity,
    curr_state_id: HsmStateId,
    exit_state_id: Entity,
) -> impl Command<Result<()>> {
    move |world: &mut World| -> Result<()> {
        world
            .resource_mut::<CheckOnTransitionStates>()
            .remove(&state_machine_id);

        let Some((strategy, behavior)) = world
            .get::<HsmState>(exit_state_id)
            .map(|state| (state.strategy, state.behavior))
        else {
            warn!("{}", StateMachineError::HsmStateMissing(exit_state_id));
            return Ok(());
        };

        let state_id = HsmStateId::new(curr_state_id.tree(), exit_state_id);
        let transition_queue = build_exit_transition_plan(world, state_id, strategy, behavior)?;

        let mut service_target = world.entity_mut(state_machine_id);
        let Some(mut state_machine) = service_target.get_mut::<HsmStateMachine>() else {
            warn!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return Ok(());
        };

        state_machine.push_next_states(transition_queue);
        state_machine.set_curr_state(curr_state_id);
        service_target.insert(StateLifecycle::Exit);
        Ok(())
    }
}

fn get_service_target(world: &World, state_machine_id: Entity) -> Entity {
    world
        .get::<ServiceTarget>(state_machine_id)
        .map_or(state_machine_id, |st| st.0)
}

#[cfg(test)]
mod tests {
    use bevy::platform::collections::HashMap;

    use crate::{
        StateMachinePlugin, context::*, guards::GuardRegistry, prelude::SystemState,
        state_actions::*,
    };

    use super::*;

    #[derive(Resource)]
    struct DebugInfoCollector(Vec<String>);

    #[derive(Component, Debug)]
    struct Condition(bool);

    fn log_on_enter(
        entity: In<ActionContext>,
        query: Query<&Name, With<HsmState>>,
        mut collector: ResMut<DebugInfoCollector>,
    ) {
        let state_name = query
            .get(entity.state())
            .expect("State should have a Name component");
        collector.0.push(format!("{}: Enter", state_name));
    }

    fn log_on_exit(
        entity: In<ActionContext>,
        query: Query<&Name, With<HsmState>>,
        mut collector: ResMut<DebugInfoCollector>,
    ) {
        let state_name = query
            .get(entity.state())
            .expect("State should have a Name component");
        collector.0.push(format!("{}: Exit", state_name));
    }

    fn is_condition_true(entity: In<GuardContext>, query: Query<&Condition>) -> bool {
        let condition = query
            .get(entity.state_machine)
            .expect("State machine should have a Condition component");
        condition.0
    }

    fn is_condition_false(entity: In<GuardContext>, query: Query<&Condition>) -> bool {
        let condition = query
            .get(entity.state_machine)
            .expect("State machine should have a Condition component");
        !condition.0
    }

    fn set_condition_false(
        contexts: In<Vec<ActionContext>>,
        mut query: Query<&mut Condition>,
    ) -> Option<Vec<ActionContext>> {
        let mut iter = query.iter_many_mut(contexts.0.iter().map(|a| a.state_machine));
        while let Some(mut condition) = iter.fetch_next() {
            condition.0 = false;
        }
        None
    }

    fn create_state_machine(
        app: &mut App,
        states: Vec<(StateTransitionStrategy, ExitTransitionBehavior)>,
    ) {
        app.add_plugins(MinimalPlugins)
            .add_plugins(StateMachinePlugin::default());

        app.add_action_system(Update, "set_condition_false", set_condition_false);

        let world = app.world_mut();
        let systems = ActionRegistry(HashMap::from([
            (
                "log_on_enter".to_string(),
                world.register_system(log_on_enter),
            ),
            (
                "log_on_exit".to_string(),
                world.register_system(log_on_exit),
            ),
        ]));
        world.insert_resource(systems);

        let guard_registry = GuardRegistry(HashMap::from([
            (
                "is_condition_true".to_string(),
                world.register_system(is_condition_true),
            ),
            (
                "is_condition_false".to_string(),
                world.register_system(is_condition_false),
            ),
        ]));

        world.insert_resource(guard_registry);

        world.insert_resource(DebugInfoCollector(Vec::new()));

        let start_id = world.spawn_empty().id();
        let state_machine_id = world.spawn_empty().id();

        let mut curr_state_id = world
            .entity_mut(start_id)
            .insert((
                Name::new("OFF"),
                HsmState::with(states[0].0, states[0].1),
                AfterEnterSystem::new("log_on_enter"),
                BeforeExitSystem::new("log_on_exit"),
            ))
            .id();
        let mut state_tree = StateTree::new(curr_state_id);

        for (i, (strategy, behavior)) in states[1..].iter().enumerate() {
            let new_state_id = world
                .spawn((
                    Name::new(format!("ON{}", i)),
                    HsmState::with(*strategy, *behavior),
                    AfterEnterSystem::new("log_on_enter"),
                    BeforeExitSystem::new("log_on_exit"),
                    GuardEnter::new("is_condition_true"),
                    GuardExit::new("is_condition_false"),
                ))
                .id();
            state_tree.with_child(curr_state_id, new_state_id);
            curr_state_id = new_state_id;
        }

        world
            .entity_mut(curr_state_id)
            .insert(OnUpdateSystem::new("Update:set_condition_false"));

        world.entity_mut(state_machine_id).insert((
            state_tree,
            HsmStateMachine::with(
                HsmStateId::new(state_machine_id, start_id),
                #[cfg(feature = "history")]
                10,
            ),
            Name::new("StateMachines"),
            StateLifecycle::default(),
            Condition(true),
        ));
    }

    // strategy:Nested,Parallel,
    // behavior:Rebirth,Resurrection,Death,
    // 三进制表示法
    // xx：第一位表示strategy，0为Nested，1为Parallel；后一位表示behavior，0为Rebirth，1为Resurrection，2为Death,

    fn create_states_from_trinary(
        trinary: &str,
    ) -> Vec<(StateTransitionStrategy, ExitTransitionBehavior)> {
        let mut states = Vec::new();
        for c in trinary.split('_') {
            let chars: Vec<char> = c.chars().collect();
            let strategy = match chars[0] {
                '0' => StateTransitionStrategy::Nested,
                '1' => StateTransitionStrategy::Parallel,
                _ => panic!("Invalid strategy character: {}", chars[0]),
            };
            let behavior = match &chars[1..] {
                ['0'] => ExitTransitionBehavior::Rebirth,
                ['1'] => ExitTransitionBehavior::Resurrection,
                ['2'] => ExitTransitionBehavior::Death,
                _ => panic!("Invalid behavior characters: {:?}", &chars[1..]),
            };
            states.push((strategy, behavior));
        }
        states
    }

    fn create_transition_strategy_test(v: Vec<(&str, Vec<&str>)>) {
        for (i, (binary, expected)) in v.into_iter().enumerate() {
            let mut app = App::new();
            let states = create_states_from_trinary(binary);
            create_state_machine(&mut app, states);
            for _ in 0..expected.len() {
                app.update();
            }
            let collector = app.world().get_resource::<DebugInfoCollector>().unwrap();
            assert_eq!(expected, collector.0, "error in strategy<{i}>: {}", binary);
        }
    }

    #[test]
    fn test_transition_strategies() {
        create_transition_strategy_test(vec![
            (
                "00_00_00",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "00_00_01",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "00_01_00",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "00_01_01",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "01_00_00",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                ],
            ),
            (
                "01_00_01",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                ],
            ),
            (
                "01_01_00",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "01_01_01",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "01_01_02",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "01_02_01",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "01_02_02",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "02_01_01",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Exit",
                ],
            ),
            (
                "02_01_02",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Exit",
                ],
            ),
            (
                "02_02_01",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Exit",
                ],
            ),
            (
                "02_02_02",
                vec![
                    "OFF: Enter",
                    "ON0: Enter",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Exit",
                ],
            ),
            (
                "10_10_10",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "10_10_11",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "10_11_10",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "10_11_11",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                    "OFF: Enter",
                ],
            ),
            (
                "11_10_10",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                ],
            ),
            (
                "11_10_11",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                ],
            ),
            (
                "11_11_10",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "11_11_11",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "11_11_12",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "11_12_11",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                ],
            ),
            (
                "11_12_12",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                ],
            ),
            (
                "12_11_11",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "12_11_12",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                    "ON0: Exit",
                ],
            ),
            (
                "12_12_11",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                ],
            ),
            (
                "12_12_12",
                vec![
                    "OFF: Enter",
                    "OFF: Exit",
                    "ON0: Enter",
                    "ON0: Exit",
                    "ON1: Enter",
                    "ON1: Exit",
                ],
            ),
        ]);
    }
}
