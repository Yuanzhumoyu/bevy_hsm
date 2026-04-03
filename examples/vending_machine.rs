//! 这个示例演示了如何使用带守卫（Guard）的有限状态机（FSM）。
//!
//! 核心概念：
//! - **FSM**: 一个简单的状态机，包含 `Idle` 和 `Dispensing` 两个状态。
//! - **Guard**: 一个名为 `has_enough_money` 的 Bevy 系统，它会在状态转换前进行检查。
//! - **事件驱动**: `Purchase` 事件触发状态转换的尝试。
//!
//! 运行指南：
//! - 按 `P` 键：尝试购买一件价格为 10 的商品。
//! - 按 `M` 键：给玩家增加 10 金钱。
//! - 按 `R` 键：向售货机添加 5 件商品库存。

use bevy::prelude::*;
use bevy_hsm::prelude::*;

/// 代表玩家的组件，包含其金钱。
#[derive(Component, Debug)]
struct Player {
    money: u32,
}

/// 售货机实体。
#[derive(Component)]
struct VendingMachine {
    stock: u32,
}

/// 玩家尝试购买商品时触发的事件。
#[derive(Component, Clone)]
struct Purchase {
    item_price: u32,
    buyer: Entity,
}

// 状态本身就是实体，这里我们用资源来存储它们的 ID。
#[derive(Resource)]
struct VendingMachineStates {
    idle: Entity,
    dispensing: Entity,
}

/// 它检查 `Purchase` 事件中的购买者是否有足够的金钱。
fn has_enough_money(
    In(context): In<GuardContext>,
    player_query: Query<&Player>,
    purchase_query: Query<&Purchase, With<VendingMachine>>,
) -> bool {
    let Ok(purchase_event) = purchase_query.get(context.service_target) else {
        return false;
    };
    let Ok(player) = player_query.get(purchase_event.buyer) else {
        return false;
    };

    let result = player.money >= purchase_event.item_price;
    println!(
        "[Guard] Checking if player has enough money for price {}. Player has {}. Allowed: {}",
        purchase_event.item_price, player.money, result
    );
    if !result {
        println!("[VendingMachine] Not enough money!");
    }
    result
}

/// 检查售货机是否有库存。
fn is_in_stock(
    In(context): In<GuardContext>,
    vending_machine_query: Query<&VendingMachine>,
) -> bool {
    let Ok(vending_machine) = vending_machine_query.get(context.service_target) else {
        return false;
    };

    let result = vending_machine.stock > 0;
    if !result {
        println!("[VendingMachine] Out of stock!");
    }
    result
}

// --- 4. 定义状态动作 ---

/// 进入 "Dispensing" 状态时执行的动作。
fn on_enter_dispensing(
    In(context): In<ActionContext>,
    mut vending_machine_query: Query<(&mut VendingMachine, &Purchase)>,
    mut player_query: Query<&mut Player>,
    vending_machine_states: Res<VendingMachineStates>,
    mut commands: Commands,
) {
    let Ok((mut vending_machine, purchase)) = vending_machine_query.get_mut(context.service_target)
    else {
        return;
    };
    if let Ok(mut player) = player_query.get_mut(purchase.buyer) {
        player.money -= purchase.item_price;
        vending_machine.stock -= 1;
        println!(
            "[VendingMachine] Dispensing item. Player money is now: {}. Stock is now: {}",
            player.money, vending_machine.stock
        );
    }
    commands.entity(context.service_target).remove::<Purchase>();
    // 完成后立即转换回 Idle 状态
    commands.trigger(FsmTrigger::with_next(
        context.state_machine,
        vending_machine_states.idle,
    ));
}

fn on_enter_idle(_: In<ActionContext>) {
    println!("[VendingMachine] Now in Idle state.");
}

fn setup(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<ActionRegistry>,
) {
    // 注册守卫系统，并给它一个字符串名称
    let has_enough_money_guard = commands.register_system(has_enough_money);
    guard_registry.insert("has_enough_money", has_enough_money_guard);
    let is_in_stock_guard = commands.register_system(is_in_stock);
    guard_registry.insert("is_in_stock", is_in_stock_guard);

    let id = commands.register_system(on_enter_dispensing);
    action_registry.insert("on_enter_dispensing", id);
    let id = commands.register_system(on_enter_idle);
    action_registry.insert("on_enter_idle", id);

    fn create_resource(mut entity_commands: EntityCommands, states: &[Entity]) {
        entity_commands
            .commands()
            .insert_resource(VendingMachineStates {
                idle: states[0],
                dispensing: states[1],
            });
    }

    commands.spawn(fsm! {
        states: {
            #[state(after_enter = "on_enter_idle")]: idle,
            #[state(after_enter = "on_enter_dispensing")]: dispensing,
        },
        transitions: {
            idle => dispensing :guard(and("has_enough_money", "is_in_stock")),
            dispensing => idle,
        },
        components:{
            VendingMachine { stock: 5 },
            Name::new("VendingMachine"),
        },
        :create_resource,
    });

    commands.spawn((Player { money: 20 }, Name::new("Player")));

    println!("--- Vending Machine Example ---");
    println!("Player starts with {} money.", 20);
    println!("Press 'P' to try to purchase an item for 10.");
    println!("Press 'M' to add 10 money to the player.");
    println!("Press 'R' to restock the vending machine.");
}

// --- 6. 辅助系统 ---

/// 处理用户输入
fn handle_input(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(Entity, &mut Player)>,
    vending_machine_states: Res<VendingMachineStates>,
    mut vending_machine_query: Query<(Entity, &mut VendingMachine)>,
) {
    let Ok((player_entity, mut player)) = player_query.single_mut() else {
        return;
    };
    let Ok((vending_machine_entity, mut vending_machine)) = vending_machine_query.single_mut()
    else {
        return;
    };

    if keyboard_input.just_pressed(KeyCode::KeyP) {
        println!("[Input] 'P' pressed. Attempting to purchase...");
        commands.entity(vending_machine_entity).insert(Purchase {
            buyer: player_entity,
            item_price: 10,
        });
        commands.trigger(FsmTrigger::with_guard(
            vending_machine_entity,
            vending_machine_states.dispensing,
        ))
    }

    if keyboard_input.just_pressed(KeyCode::KeyM) {
        player.money += 10;
        println!("[Input] 'M' pressed. Player money is now: {}", player.money);
    }

    if keyboard_input.just_pressed(KeyCode::KeyR) {
        vending_machine.stock += 5;
        println!(
            "[Input] 'R' pressed. Vending machine stock is now: {}",
            vending_machine.stock
        );
    }
}

/// 每当玩家金钱变化时打印日志
fn log_player_money(query: Query<&Player, Changed<Player>>) {
    if let Ok(player) = query.single() {
        println!("[Event] Player money changed to: {}", player.money);
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, StateMachinePlugin::default()))
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_input, log_player_money))
        .run();
}
