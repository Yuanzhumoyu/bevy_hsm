use bevy::prelude::*;

use crate::{
    StateMachinePlugin, context::*, guards::GuardRegistry, prelude::SystemState, state_actions::*,
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
    let systems = ActionRegistry::from([
        ("log_on_enter", world.register_system(log_on_enter)),
        ("log_on_exit", world.register_system(log_on_exit)),
    ]);
    world.insert_resource(systems);

    let guard_registry = GuardRegistry::from([
        (
            "is_condition_true",
            world.register_system(is_condition_true),
        ),
        (
            "is_condition_false",
            world.register_system(is_condition_false),
        ),
    ]);

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
            state_machine_id,
            start_id,
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
        let collector = app
            .world()
            .get_resource::<DebugInfoCollector>()
            .expect("DebugInfoCollector missing in test app world");
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
