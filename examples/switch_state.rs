use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<HsmStateContext>, Query<&Name, With<HsmState>>) {
    move |context: In<HsmStateContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state).unwrap();
        println!("[{}]{}: {}", context.state, state_name, info);
    }
}

#[derive(Component, Default)]
pub struct Count(usize);

impl Count {
    fn action(
        states: In<Vec<HsmStateContext>>,
        mut query: Query<(&Name, &mut Count)>,
    ) -> Option<Vec<HsmStateContext>> {
        let mut iter = query.iter_many_mut(states.0.iter().map(|a| a.main_body));
        while let Some((name, mut count)) = iter.fetch_next() {
            count.0 += 1;
            println!("{} 计数: {}", name, count.0);
        }
        // 当返回值为 Some 时, 状态会延长更新
        // 当返回值为 None 时, 状态则会截流, 后续的状态更新会被忽略
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
    fn condition_with_open(entity: In<HsmStateContext>, query: Query<&Switch>) -> bool {
        let switch = query.get(entity.main_body).unwrap();
        matches!(switch, Switch::Open)
    }

    fn condition_with_close(entity: In<HsmStateContext>, query: Query<&Switch>) -> bool {
        let switch = query.get(entity.main_body).unwrap();
        matches!(switch, Switch::Close)
    }
}

fn register_condition(
    mut commands: Commands,
    mut action_systems: ResMut<StateConditions>,
    mut on_enter_disposable_systems: ResMut<HsmOnEnterDisposableSystems>,
    mut on_exit_disposable_systems: ResMut<HsmOnExitDisposableSystems>,
) {
    let id = commands.register_system(Switch::condition_with_open);
    action_systems.insert("is_open", id);
    let id = commands.register_system(Switch::condition_with_close);
    action_systems.insert("is_close", id);

    let id = commands.register_system(debug_on_state("进入状态"));
    on_enter_disposable_systems.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("退出状态"));
    on_exit_disposable_systems.insert("debug_on_exit", id);
}

fn startup(mut commands: Commands) {
    let start_state_id = commands.spawn_empty().id();
    let state_machines = commands.spawn(StateMachines::new(10, start_state_id)).id();

    commands.entity(start_state_id).insert((
        Name::new("起点"),
        HsmState::new(state_machines),
        HsmOnEnterSystem::new("debug_on_enter"),
        HsmOnExitSystem::new("debug_on_exit"),
    ));

    commands.spawn((
        SuperState(start_state_id),
        Name::new("计数"),
        HsmState::new(state_machines),
        HsmOnEnterCondition::new("is_open"),
        HsmOnExitCondition::new("is_close"),
        HsmOnEnterSystem::new("debug_on_enter"),
        HsmOnUpdateSystem::new("Update:计数"),
        HsmOnExitSystem::new("debug_on_exit"),
    ));

    println!("状态机: {:?}", state_machines);

    commands.entity(state_machines).insert((
        Name::new("开关计数"),
        Count(0),
        HsmOnState::default(),
        Switch::Close,
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
/// # 状态机示例
///
/// 本示例演示了如何使用 bevy_hsm 库创建一个简单的状态机
///
/// ## 实体说明
/// * [Count] - 计数器组件，用于在"计数"状态下增加计数
/// * [StateMachines] - 状态机组件，管理当前状态和状态转换
///
///
/// ## 状态转换
/// [起点] <-> [计数] - 通过切换开关状态来在两个状态间转换
///
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(HsmPlugin::default());

    app.add_action_system(Update, "计数", Count::action);

    app.add_systems(Startup, (register_condition, startup).chain());

    app.add_systems(Update, key_event);

    app.run();
}
