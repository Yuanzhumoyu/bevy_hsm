use bevy::{ecs::schedule::ScheduleLabel, platform::collections::HashSet, prelude::*};

use crate::{
    context::OnStateConditionContext,
    error::HsmError,
    hsm::{
        HsmState,
        state_machine::{NextState, *},
        state_tree::StateTree,
    },
    prelude::{
        HsmOnEnterCondition, HsmOnExitCondition, ServiceTarget, StateEnterConditionBuffer,
        StateExitConditionBuffer, TreeStateId,
    },
    state_machine_component::*,
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
    ///    super_state: on_enter
    ///    sub_state: on_enter
    ///    sub_state: on_exit
    ///    super_state: on_exit
    /// ```
    #[default]
    Nested,
    /// 平级转换：父状态先退出，然后子状态进入和退出，最后可能重新进入父状态
    ///
    /// Level-to-level transition: The parent state exits first, followed by the entry and exit of the child state, and finally, the parent state may be re-entered
    /// ```toml
    ///    super_state: on_enter
    ///    super_state: on_exit
    ///    sub_state: on_enter
    ///    sub_state: on_exit
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
    /// 从sub_state退出后，重新进入super_state的on_enter阶段
    ///
    /// From sub_state exit, re-enter the super_state's on_enter phase
    Rebirth,
    /// # 复活\Resurrection
    ///
    /// 从sub_state退出后，进入super_state的on_update阶段
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

impl From<ExitTransitionBehavior> for HsmOnState {
    fn from(value: ExitTransitionBehavior) -> Self {
        match value {
            ExitTransitionBehavior::Rebirth => HsmOnState::Enter,
            ExitTransitionBehavior::Resurrection => HsmOnState::Update,
            ExitTransitionBehavior::Death => HsmOnState::Exit,
        }
    }
}

impl From<HsmOnState> for ExitTransitionBehavior {
    fn from(value: HsmOnState) -> Self {
        match value {
            HsmOnState::Enter => ExitTransitionBehavior::Rebirth,
            HsmOnState::Update => ExitTransitionBehavior::Resurrection,
            HsmOnState::Exit => ExitTransitionBehavior::Death,
        }
    }
}

fn get_on_exit_next_states(
    world: &World,
    mut state_id: TreeStateId,
    strategy: StateTransitionStrategy,
    mut behavior: ExitTransitionBehavior,
) -> Result<Vec<NextState>, String> {
    match (strategy, behavior) {
        (
            StateTransitionStrategy::Nested | StateTransitionStrategy::Parallel,
            ExitTransitionBehavior::Resurrection,
        ) => Ok(vec![NextState::Next((state_id, HsmOnState::Update))]),
        (
            StateTransitionStrategy::Nested | StateTransitionStrategy::Parallel,
            ExitTransitionBehavior::Rebirth,
        ) => Ok(vec![NextState::Next((state_id, HsmOnState::Enter))]),
        (StateTransitionStrategy::Nested, ExitTransitionBehavior::Death) => {
            let Some(state_tree) = world.get::<StateTree>(state_id.tree()) else {
                return Err(format!(
                    "The entity<{}> does not contain [StateTree]",
                    state_id
                ));
            };
            if state_tree.get_root() == state_id.state() {
                let nex_state = match behavior == ExitTransitionBehavior::Death {
                    true => NextState::Next((state_id, behavior.into())),
                    false => NextState::None,
                };
                return Ok(vec![nex_state]);
            }
            let mut next_states = vec![NextState::Next((state_id, HsmOnState::Exit))];

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
                let state_id = TreeStateId::new(state_id.tree(), state);
                if state_tree.get_root() == state_id.state() {
                    let nex_state = match behavior == ExitTransitionBehavior::Death {
                        true => NextState::Next((state_id, behavior.into())),
                        false => NextState::None,
                    };
                    next_states.push(nex_state);
                    return Ok(next_states);
                }
                match !(strategy == StateTransitionStrategy::Nested
                    && behavior == ExitTransitionBehavior::Death)
                {
                    true => {
                        next_states.extend(get_on_exit_next_states(
                            world, state_id, strategy, behavior,
                        )?);
                        return Ok(next_states);
                    }
                    false => {
                        next_states.push(NextState::Next((state_id, HsmOnState::Exit)));
                    }
                }
            }
            Ok(next_states)
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
                let new_state_id = TreeStateId::new(state_id.tree(), state);
                if !(strategy == StateTransitionStrategy::Parallel
                    && new_behavior == ExitTransitionBehavior::Death)
                {
                    return get_on_exit_next_states(world, new_state_id, strategy, new_behavior);
                }
                state_id = new_state_id;
                behavior = new_behavior;
            }
            match behavior {
                ExitTransitionBehavior::Rebirth => {
                    Ok(vec![NextState::Next((state_id, HsmOnState::Enter))])
                }
                ExitTransitionBehavior::Resurrection => {
                    Ok(vec![NextState::Next((state_id, HsmOnState::Update))])
                }
                ExitTransitionBehavior::Death => Ok(vec![NextState::None]),
            }
        }
    }
}

/// 检查能否过渡状态的实体
///
/// Check whether the entity can transition
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq, Deref, DerefMut)]
pub(crate) struct CheckOnTransitionStates(HashSet<Entity>);

pub(crate) fn add_handle_on_state<T: ScheduleLabel>(app: &mut App, schedule: T) {
    app.add_systems(
        schedule,
        (handle_on_enter_states, handle_on_exit_states)
            .chain()
            .run_if(|check_on_transition_states: Res<CheckOnTransitionStates>| {
                !check_on_transition_states.is_empty()
            }),
    );
}

fn handle_on_enter_states(
    mut commands: Commands,
    check_on_transition_states: Res<CheckOnTransitionStates>,
    query_state_machines: Query<(Entity, &HsmStateMachine), Without<StationaryStateMachine>>,
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
                warn!("{}", HsmError::StateTreeNotFound(curr_state_id.tree()));
                return;
            };
            let sub_state_iter =
                state_tree.traversal_iter_with(world, curr_state_id.state(), |e| {
                    if !e.contains::<HsmState>() {
                        warn!("{}", HsmError::HsmStateMissing(e.id()));
                        return false;
                    }
                    e.contains::<HsmOnEnterCondition>()
                });
            let Some(enter_state_id) = world.resource_scope(
                |world: &mut World, condition_buffer: Mut<StateEnterConditionBuffer>| {
                    for sub_state_id in sub_state_iter {
                        let Some(condition_id) = condition_buffer.get(&sub_state_id) else {
                            continue;
                        };

                        match condition_id.run(
                            world,
                            OnStateConditionContext::new(
                                match world.get::<ServiceTarget>(state_machine_id) {
                                    Some(service_target) => service_target.0,
                                    None => state_machine_id,
                                },
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
                                    HsmError::ConditionRunFailed {
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

            world
                .resource_mut::<CheckOnTransitionStates>()
                .remove(&state_machine_id);

            let mut service_target = world.entity_mut(state_machine_id);
            let Some(mut state_machine) = service_target.get_mut::<HsmStateMachine>() else {
                warn!("{}", HsmError::StateMachineMissing(state_machine_id));
                return;
            };

            let state_id = TreeStateId::new(curr_state_id.tree(), enter_state_id);
            let next_on_state: HsmOnState = match strategy {
                StateTransitionStrategy::Nested => {
                    state_machine.set_curr_state(state_id);
                    HsmOnState::Enter
                }
                StateTransitionStrategy::Parallel => {
                    state_machine.set_curr_state(curr_state_id);
                    state_machine.push_next_state(NextState::Next((state_id, HsmOnState::Enter)));
                    HsmOnState::Exit
                }
            };

            service_target.insert(next_on_state);
        });
    }
}

fn handle_on_exit_states(
    mut commands: Commands,
    check_on_transition_states: Res<CheckOnTransitionStates>,
    query_state_machines: Query<(Entity, &HsmStateMachine), Without<StationaryStateMachine>>,
    query_on_exit_conditions: Query<Has<HsmOnExitCondition>, With<HsmState>>,
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
            warn!("{}", HsmError::StateTreeNotFound(curr_state_id.tree()));
            continue;
        };
        let Some(super_state_id) = state_tree.get_super_state(curr_state_id.state()) else {
            warn!(
                "{}",
                HsmError::SuperStateNotFound {
                    state_tree: curr_state_id.tree(),
                    state: curr_state_id.state()
                }
            );
            continue;
        };
        commands.queue(move |world: &mut World| -> Result<()> {
            match world.resource_scope(
                |world: &mut World, condition_buffer: Mut<StateExitConditionBuffer>| {
                    match condition_buffer.get(&curr_state_id.state()) {
                        Some(condition_id) => condition_id.run(
                            world,
                            OnStateConditionContext::new(
                                match world.get::<ServiceTarget>(state_machine_id) {
                                    Some(service_target) => service_target.0,
                                    None => state_machine_id,
                                },
                                state_machine_id,
                                curr_state_id.state(),
                                super_state_id,
                            ),
                        ),
                        None => Ok(false),
                    }
                },
            ) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(e) => {
                    error!(
                        "{}",
                        HsmError::ConditionRunFailed {
                            state_machine: state_machine_id,
                            from_state: curr_state_id.state(),
                            to_state: None,
                            source: e.into(),
                        }
                    );
                    return Ok(());
                }
            }

            world
                .resource_mut::<CheckOnTransitionStates>()
                .remove(&state_machine_id);

            let Some((strategy, behavior)) = world
                .get::<HsmState>(super_state_id)
                .map(|state| (state.strategy, state.behavior))
            else {
                warn!("{}", HsmError::HsmStateMissing(state_machine_id));
                return Ok(());
            };

            let state_id = TreeStateId::new(curr_state_id.tree(), super_state_id);
            let next_states = get_on_exit_next_states(world, state_id, strategy, behavior)?;

            let mut service_target = world.entity_mut(state_machine_id);
            let Some(mut state_machine) = service_target.get_mut::<HsmStateMachine>() else {
                warn!("{}", HsmError::StateMachineMissing(state_machine_id));
                return Ok(());
            };

            state_machine.push_next_states(next_states);
            service_target.insert(HsmOnState::Exit);
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::platform::collections::HashMap;

    use crate::{
        HsmPlugin, context::*, hook_system::*, hsm::state_condition::StateConditions,
        prelude::SystemState,
    };

    use super::*;

    #[derive(Resource)]
    struct DebugInfoCollector(Vec<String>);

    #[derive(Component, Debug)]
    struct Condition(bool);

    fn log_on_enter(
        entity: In<OnStateContext>,
        query: Query<&Name, With<HsmState>>,
        mut collector: ResMut<DebugInfoCollector>,
    ) {
        let state_name = query
            .get(entity.state())
            .expect("State should have a Name component");
        collector.0.push(format!("{}: Enter", state_name));
    }

    fn log_on_exit(
        entity: In<OnStateContext>,
        query: Query<&Name, With<HsmState>>,
        mut collector: ResMut<DebugInfoCollector>,
    ) {
        let state_name = query
            .get(entity.state())
            .expect("State should have a Name component");
        collector.0.push(format!("{}: Exit", state_name));
    }

    fn is_condition_true(entity: In<OnStateConditionContext>, query: Query<&Condition>) -> bool {
        let condition = query
            .get(entity.state_machine)
            .expect("State machine should have a Condition component");
        condition.0
    }

    fn is_condition_false(entity: In<OnStateConditionContext>, query: Query<&Condition>) -> bool {
        let condition = query
            .get(entity.state_machine)
            .expect("State machine should have a Condition component");
        !condition.0
    }

    fn set_condition_false(
        contexts: In<Vec<OnStateContext>>,
        mut query: Query<&mut Condition>,
    ) -> Option<Vec<OnStateContext>> {
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
            .add_plugins(HsmPlugin::default());

        app.add_action_system(Update, "set_condition_false", set_condition_false);

        let world = app.world_mut();
        let systems = NamedStateSystems(HashMap::from([
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

        let state_conditions = StateConditions(HashMap::from([
            (
                "is_condition_true".to_string(),
                world.register_system(is_condition_true),
            ),
            (
                "is_condition_false".to_string(),
                world.register_system(is_condition_false),
            ),
        ]));

        world.insert_resource(state_conditions);

        world.insert_resource(DebugInfoCollector(Vec::new()));

        let start_id = world.spawn_empty().id();
        let state_machine_id = world.spawn_empty().id();

        let mut curr_state_id = world
            .entity_mut(start_id)
            .insert((
                Name::new("OFF"),
                HsmState::with(states[0].0, states[0].1),
                OnEnterSystem::new("log_on_enter"),
                OnExitSystem::new("log_on_exit"),
            ))
            .id();
        let mut state_tree = StateTree::new(curr_state_id);

        for (i, (strategy, behavior)) in states[1..].iter().enumerate() {
            let new_state_id = world
                .spawn((
                    Name::new(format!("ON{}", i)),
                    HsmState::with(*strategy, *behavior),
                    OnEnterSystem::new("log_on_enter"),
                    OnExitSystem::new("log_on_exit"),
                    HsmOnEnterCondition::new("is_condition_true"),
                    HsmOnExitCondition::new("is_condition_false"),
                ))
                .id();
            state_tree.add(curr_state_id, new_state_id);
            curr_state_id = new_state_id;
        }

        world
            .entity_mut(curr_state_id)
            .insert(OnUpdateSystem::new("Update:set_condition_false"));

        world.entity_mut(state_machine_id).insert((
            state_tree,
            HsmStateMachine::new(10, TreeStateId::new(state_machine_id, start_id)),
            Name::new("StateMachines"),
            HsmOnState::default(),
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
