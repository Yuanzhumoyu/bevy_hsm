use bevy::{ecs::schedule::ScheduleLabel, platform::collections::HashSet, prelude::*};

use crate::{
    prelude::HsmStateContext,
    state::{HsmOnState, HsmState, StateMachines},
    state_condition::{HsmOnEnterCondition, HsmOnExitCondition, StateConditions},
    sub_states::SubStates,
    super_state::SuperState,
};

/// 状态转换策略，用于控制状态转换行为
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateTransitionStrategy {
    /// 重新进入状态
    ReEnter,
    /// 继续更新状态
    #[default]
    ContinueUpdate,
    /// 直接退出状态
    Exit,
}

impl StateTransitionStrategy {
    pub fn is_reenter(&self) -> bool {
        matches!(self, Self::ReEnter)
    }

    pub fn is_continue_update(&self) -> bool {
        matches!(self, Self::ContinueUpdate)
    }

    pub fn is_exit(&self) -> bool {
        matches!(self, Self::Exit)
    }
}

impl From<StateTransitionStrategy> for HsmOnState {
    fn from(value: StateTransitionStrategy) -> Self {
        match value {
            StateTransitionStrategy::ReEnter => HsmOnState::Enter,
            StateTransitionStrategy::ContinueUpdate => HsmOnState::Update,
            StateTransitionStrategy::Exit => HsmOnState::Exit,
        }
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
    query_state_machines: Query<&StateMachines>,
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

                let Some(name) = world.get::<Name>(sub_state_id).map(ToString::to_string) else {
                    warn!("{} 该实体不拥有[Name]", sub_state_id);
                    continue;
                };
                let mut main_body = world.entity_mut(main_body_id);
                let Some(mut state_machines) = main_body.get_mut::<StateMachines>() else {
                    warn!("{} 该实体不拥有[StateMachines]", main_body_id);
                    return;
                };
                state_machines.next_state = Some(name);
                main_body.insert(HsmOnState::Enter);

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
    query_state_machines: Query<&StateMachines>,
    query_states: Query<(Entity, &HsmState, &SuperState), With<HsmState>>,
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
        let Ok((curr_state, hsm_state, super_state)) =
            query_states.get(curr_state_id)
        else {
            continue;
        };
        let main_body_id = hsm_state.main_body;
        let super_state_id = super_state.0;

        let Ok(condition) = query_condtitions.get(curr_state) else {
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

            let Some(name) = world.get::<Name>(super_state_id).map(ToString::to_string) else {
                warn!("{} 该实体不拥有[Name]", super_state_id);
                return;
            };
            let mut main_body = world.entity_mut(main_body_id);
            let Some(mut state_machines) = main_body.get_mut::<StateMachines>() else {
                warn!("{} 该实体不拥有[StateMachines]", main_body_id);
                return;
            };

            state_machines.next_state = Some(name);
            main_body.insert(HsmOnState::Exit);
        });
    }
    condition_with_empty.iter().for_each(|e| {
        check_on_transition_states.remove(e);
    });
}
