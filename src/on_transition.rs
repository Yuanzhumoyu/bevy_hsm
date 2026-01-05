use bevy::{ecs::schedule::ScheduleLabel, platform::collections::HashSet, prelude::*};

use crate::{
    prelude::HsmStateContext,
    state::{HsmOnState, HsmState, StateMachines, StationaryStateMachines},
    state_condition::{HsmOnEnterCondition, HsmOnExitCondition, StateConditions},
    sub_states::SubStates,
    super_state::SuperState,
};

/// 状态转换策略，用于控制状态转换行为
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTransitionStrategy {
    /// 子状态嵌套转换：父状态保持激活，子状态进入和退出发生在父状态内部
    /// ```toml
    ///    super_state: on_enter
    ///    sub_state: on_enter
    ///    sub_state: on_exit
    ///    super_state: on_exit
    /// ```
    ///
    /// 接受bool值，表示退出sub_state后super_state中on_update是否延续(当状态处于截流时,不会触发)
    Nested(bool),
    /// 平级转换：父状态先退出，然后子状态进入和退出，最后可能重新进入父状态
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
        matches!(self, Self::Nested(_))
    }

    pub fn is_parallel(&self) -> bool {
        matches!(self, Self::Parallel)
    }
}

impl Default for StateTransitionStrategy {
    fn default() -> Self {
        Self::Nested(false)
    }
}

/// 检查能否过渡状态的实体
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq, Deref, DerefMut)]
pub(super) struct CheckOnTransitionStates(HashSet<Entity>);

pub(super) fn add_handle_on_state<T: ScheduleLabel>(app: &mut App, schedule: T) {
    app.add_systems(
        schedule,
        (handle_on_enter_states, handle_on_exit_states)
            .chain()
            .run_if(|check_on_transition_states: Res<CheckOnTransitionStates>| {
                !check_on_transition_states.is_empty()
            }),
    );
}

/// 处理进入状态
fn handle_on_enter_states(
    mut commands: Commands,
    query_state_machines: Query<&StateMachines, Without<StationaryStateMachines>>,
    query_states: Query<(&HsmState, &SubStates), With<HsmState>>,
    query_sub_states: Query<(Entity, &HsmOnEnterCondition), (With<HsmState>, With<SuperState>)>,
    mut check_on_transition_states: ResMut<CheckOnTransitionStates>,
    state_conditions: Res<StateConditions>,
) {
    // 条件为空的状态
    let mut condition_with_empty = Vec::new();

    for state_machines in query_state_machines.iter_many(check_on_transition_states.iter()) {
        let Some(curr_state_id) = state_machines.curr_state_id() else {
            warn!("Current state not found in states map",);
            return;
        };
        let Ok((hsm_state, sub_states)) = query_states.get(curr_state_id) else {
            continue;
        };
        let collected = query_sub_states
            .iter_many_inner(sub_states.iter())
            .filter_map(|(super_state_id, condition)| {
                match state_conditions.to_combinator_condition_id(&condition.0) {
                    Some(id) => Some((super_state_id, id)),
                    None => {
                        warn!("不存在这个条件: {:?}", condition.0);
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        let main_body_id = hsm_state.main_body;

        if collected.is_empty() {
            condition_with_empty.push(main_body_id);
            continue;
        }

        commands.queue(move |world: &mut World| {
            for (sub_state_id, condition_id) in collected {
                match condition_id.run(world, HsmStateContext::new(main_body_id, sub_state_id)) {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(e) => {
                        warn!("Error running enter condition: {:?}", e);
                        continue;
                    }
                }

                world
                    .resource_mut::<CheckOnTransitionStates>()
                    .remove(&main_body_id);

                let transition_strategy = world
                    .get::<StateTransitionStrategy>(curr_state_id)
                    .copied()
                    .unwrap();
                let Some(curr_state_name) =
                    world.get::<Name>(curr_state_id).map(ToString::to_string)
                else {
                    warn!("{} 该实体不拥有[Name]", curr_state_id);
                    return;
                };
                let Some(sub_state_name) = world.get::<Name>(sub_state_id).map(ToString::to_string)
                else {
                    warn!("{} 该实体不拥有[Name]", sub_state_id);
                    continue;
                };
                let mut main_body = world.entity_mut(main_body_id);
                let Some(mut state_machines) = main_body.get_mut::<StateMachines>() else {
                    warn!("{} 该实体不拥有[StateMachines]", main_body_id);
                    return;
                };

                let next_on_state = match transition_strategy {
                    StateTransitionStrategy::Nested(_resurrection) => {
                        state_machines.push_history(curr_state_name);
                        state_machines.push_next_state(sub_state_name, HsmOnState::Enter);
                        HsmOnState::Exit
                    }
                    StateTransitionStrategy::Parallel => {
                        state_machines.push_history(sub_state_name);
                        HsmOnState::Enter
                    }
                };

                main_body.insert(next_on_state);

                return;
            }
        });
    }
    condition_with_empty.iter().for_each(move |e| {
        check_on_transition_states.remove(e);
    });
}

/// 处理退出状态
fn handle_on_exit_states(
    mut commands: Commands,
    query_state_machines: Query<&StateMachines, Without<StationaryStateMachines>>,
    query_states: Query<(&HsmState, &SuperState), With<HsmState>>,
    query_condtitions: Query<&HsmOnExitCondition, With<HsmState>>,
    mut check_on_transition_states: ResMut<CheckOnTransitionStates>,
    state_conditions: Res<StateConditions>,
) {
    // 条件为空的状态
    let mut condition_with_empty = Vec::new();
    for state_machines in query_state_machines.iter_many(check_on_transition_states.iter()) {
        let Some(curr_state_id) = state_machines.curr_state_id() else {
            warn!("Current state not found in states map",);
            return;
        };
        let Ok((hsm_state, super_state)) = query_states.get(curr_state_id) else {
            continue;
        };
        let main_body_id = hsm_state.main_body;
        let super_state_id = super_state.0;

        let Ok(condition) = query_condtitions.get(curr_state_id) else {
            condition_with_empty.push(main_body_id);
            continue;
        };
        let Some(condition_id) = state_conditions.to_combinator_condition_id(condition) else {
            warn!("[StateConditions]不存在这个条件: {:?}", condition.0);
            continue;
        };

        commands.queue(move |world: &mut World| {
            match condition_id.run(world, HsmStateContext::new(main_body_id, super_state_id)) {
                Ok(true) => {}
                Ok(false) => return,
                Err(e) => {
                    warn!("Error running exit condition: {:?}", e);
                    return;
                }
            }

            world
                .resource_mut::<CheckOnTransitionStates>()
                .remove(&main_body_id);

            let transition_strategy = world
                .get::<StateTransitionStrategy>(super_state_id)
                .copied()
                .unwrap();
            let Some(curr_state_name) = world.get::<Name>(curr_state_id).map(ToString::to_string)
            else {
                warn!("{} 该实体不拥有[Name]", curr_state_id);
                return;
            };
            let Some(super_state_name) = world.get::<Name>(super_state_id).map(ToString::to_string)
            else {
                warn!("{} 该实体不拥有[Name]", super_state_id);
                return;
            };
            let mut main_body = world.entity_mut(main_body_id);
            let Some(mut state_machines) = main_body.get_mut::<StateMachines>() else {
                warn!("{} 该实体不拥有[StateMachines]", main_body_id);
                return;
            };

            state_machines.push_history(curr_state_name);
            if let StateTransitionStrategy::Nested(resurrection) = transition_strategy {
                state_machines.push_next_state(
                    super_state_name,
                    match resurrection {
                        true => HsmOnState::Update,
                        false => HsmOnState::Enter,
                    },
                );
            }
            main_body.insert(HsmOnState::Exit);
        });
    }
    condition_with_empty.iter().for_each(|e| {
        check_on_transition_states.remove(e);
    });
}
