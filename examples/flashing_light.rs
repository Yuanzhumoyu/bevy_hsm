use bevy::{platform::collections::HashMap, prelude::*};
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<HsmStateContext>, Query<&Name, With<HsmState>>) {
    move |context: In<HsmStateContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state).unwrap();
        println!("[{}]{}: {}", context.state, state_name, info);
    }
}

fn debug_light(
    states: In<Vec<HsmStateContext>>,
    query: Query<&Name>,
) -> Option<Vec<HsmStateContext>> {
    for light in query.iter_many(states.0.iter().map(|a| a.state)) {
        println!("当前灯: {}", light);
    }
    None
}

#[derive(Component, Default)]
struct LightTimer(Timer);

impl LightTimer {
    fn light_timer(
        entity: In<HsmStateContext>,
        time: Res<Time<Fixed>>,
        mut query: Query<&mut LightTimer>,
    ) -> bool {
        let mut timer = query.get_mut(entity.main_body).unwrap();
        timer.0.tick(time.delta());
        timer.0.is_finished()
    }
}

fn register_condition(
    mut commands: Commands,
    mut action_systems: ResMut<StateConditions>,
    mut on_enter_disposable_systems: ResMut<HsmOnEnterDisposableSystems>,
    mut on_exit_disposable_systems: ResMut<HsmOnExitDisposableSystems>,
) {
    let id = commands.register_system(LightTimer::light_timer);
    action_systems.insert("light_timer", id);

    let id = commands.register_system(debug_on_state("进入状态"));
    on_enter_disposable_systems.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("退出状态"));
    on_exit_disposable_systems.insert("debug_on_exit", id);
}

fn setup(mut commands: Commands) {
    let state_machines = commands
        .spawn(StateMachines::new(HashMap::new(), 10, "red"))
        .id();

    let id1 = commands
        .spawn((
            Name::new("red"),
            HsmState::new(state_machines),
            StateTransitionStrategy::Nested(false),
            HsmOnUpdateSystem::new("Update:debug_light"),
            HsmOnEnterSystem::new("debug_on_enter"),
            HsmOnExitSystem::new("debug_on_exit"),
        ))
        .id();

    commands.spawn((
        SuperState(id1),
        Name::new("yellow"),
        HsmState::new(state_machines),
        HsmOnUpdateSystem::new("Update:debug_light"),
        HsmOnEnterSystem::new("debug_on_enter"),
        HsmOnExitSystem::new("debug_on_exit"),
        HsmOnEnterCondition::new("light_timer"),
        HsmOnExitCondition::new("light_timer"),
    ));

    println!("状态机: {:?}", state_machines);

    commands.entity(state_machines).insert((
        Name::new("闪烁灯暂停"),
        HsmOnState::default(),
        LightTimer(Timer::from_seconds(1.0, TimerMode::Repeating)),
    ));
}

fn blinking_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    state_machines: Single<Entity, With<StateMachines>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        let mut entity = commands.entity(state_machines.entity());
        entity.queue(|mut entity: EntityWorldMut<'_>| {
            match entity.get::<StationaryStateMachines>().is_some() {
                true => {
                    info!("恢复闪烁灯");
                    entity.remove::<StationaryStateMachines>();
                }
                false => {
                    info!("暂停闪烁灯");
                    entity.insert(StationaryStateMachines);
                }
            };
        });
    }
}

///
/// # 状态机示例
///
/// 本示例演示了如何使用 bevy_hsm 库创建一个具有暂停功能的状态机
///
/// ## 状态转换
/// [red] <-> [yellow] - 通过计时器来在两个状态间转换
///
/// ## 状态机暂停
/// 通过切换按键空格来暂停和恢复状态机
///
fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(HsmPlugin::default());

    app.add_action_system(Update, "debug_light", debug_light);

    app.add_systems(Startup, (register_condition, setup).chain());
    app.add_systems(Update, blinking_pause);

    app.run();
}
