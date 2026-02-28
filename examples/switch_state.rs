use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<StateActionContext>, Query<&Name, With<HsmState>>) {
    move |context: In<StateActionContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

#[derive(Component, Default)]
pub struct Count(usize);

impl Count {
    fn action(
        states: In<Vec<StateActionContext>>,
        mut query: Query<(&Name, &mut Count)>,
    ) -> Option<Vec<StateActionContext>> {
        let mut iter = query.iter_many_mut(states.0.iter().map(|a| a.service_target));
        while let Some((name, mut count)) = iter.fetch_next() {
            count.0 += 1;
            println!("{} 计数: {}", name, count.0);
        }
        // 当返回值为 Some 时, 状态会延长更新
        // When return value is Some, the state will continue updating
        // 当返回值为 None 时, 状态则会截流, 后续的状态更新会被忽略
        // When return value is None, the state will be intercepted and subsequent updates will be ignored
        Some(states.0)
        // None
    }
}

#[derive(Component, Default, Debug, Clone, Copy)]
pub enum Switch {
    Open,
    #[default]
    Close,
}

impl Switch {
    fn condition_with_open(entity: In<GuardContext>, query: Query<&Switch>) -> bool {
        let switch = query.get(entity.service_target).unwrap();
        matches!(switch, Switch::Open)
    }

    fn condition_with_close(entity: In<GuardContext>, query: Query<&Switch>) -> bool {
        let switch = query.get(entity.service_target).unwrap();
        matches!(switch, Switch::Close)
    }
}

fn register_condition(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<StateActionRegistry>,
) {
    let id = commands.register_system(Switch::condition_with_open);
    guard_registry.insert("is_open", id);
    let id = commands.register_system(Switch::condition_with_close);
    guard_registry.insert("is_close", id);

    let id = commands.register_system(debug_on_state("Entering state."));
    action_registry.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("Exiting state."));
    action_registry.insert("debug_on_exit", id);
}

fn startup(mut commands: Commands) {
    let start_id = commands
        .spawn((
            Name::new("Start"),
            HsmState::default(),
            OnEnterSystem::new("debug_on_enter"),
            OnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id = commands
        .spawn((
            Name::new("Counter"),
            HsmState::default(),
            EnterGuard::new("is_open"),
            ExitGuard::new("is_close"),
            OnEnterSystem::new("debug_on_enter"),
            OnUpdateSystem::new("Update:计数"),
            OnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let state_machine = commands.spawn_empty().id();
    println!("State Machines: {:?}", state_machine);

    let traversal = TraversalStrategy::default();
    let mut state_tree = StateTree::new(start_id);
    state_tree
        .with_traversal(start_id, traversal)
        .with_add(start_id, id);

    commands.entity(state_machine).insert((
        HsmStateMachine::new(HsmStateId::new(state_machine, start_id), 10),
        Name::new("Switch Counter"),
        StateLifecycle::default(),
        Switch::Close,
        state_tree,
        Count(0),
    ));
}

fn key_event(input: Res<ButtonInput<KeyCode>>, mut query: Query<&mut Switch>) {
    if input.any_just_pressed([KeyCode::Space]) {
        let mut switch = query.single_mut().unwrap();
        let old = *switch;
        *switch = match old {
            Switch::Open => Switch::Close,
            Switch::Close => Switch::Open,
        };
        println!("{:?}->{:?}", old, switch);
    }
}
///
/// # 状态机示例\State Machine Example
///
/// 本示例演示了如何使用 bevy_hsm 库创建一个简单的状态机
///
/// This example demonstrates how to use the bevy_hsm library to create a simple state machine
/// ## 实体说明\Entity Description
/// * [Count] - 计数器组件，用于在"计数"状态下增加计数
/// - [Count] - Counter component, used to increase the counter in the "counting" state
/// * [HsmStateMachine] - 状态机组件，管理当前状态和状态转换
/// - [HsmStateMachine] - State machine component, managing the current state and state transitions
///
///
/// ## 状态转换\State Transition
/// [Start] <-> [Counter] - 通过切换开关状态来在两个状态间转换
/// [Start] <-> [Counter] - Transition between two states by switching the switch status
///
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::<Last>::default());

    app.add_action_system(Update, "计数", Count::action);

    app.add_systems(Startup, (register_condition, startup).chain());

    app.add_systems(Update, key_event);

    app.run();
}
