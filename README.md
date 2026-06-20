# Bevy HSM — 混合状态机系统

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[English](docs/en/README-en.md)

一个为 [Bevy 游戏引擎](https://bevyengine.org/) 设计的、强大的混合状态机系统。它无缝集成了**层级状态机 (HSM)** 和**有限状态机 (FSM)**，让您可以为不同的场景选择最合适的工具：

- 使用 **HSM** 管理复杂的、有生命周期的层级行为状态
- 使用 **FSM** 管理扁平、事件驱动的快速状态切换
- 通过 **中断/恢复** 机制在两种模式间自由切换，实现应急处理

---

## 功能特性

- **双模式支持**: 在统一框架内同时支持 HSM 和 FSM，可按需组合
- **状态生命周期**: 支持 `BeforeEnter` → `AfterEnter` → `OnUpdate` → `BeforeExit` → `AfterExit` 五个生命周期阶段，每个阶段关联独立的 Bevy 系统
- **层级结构 (HSM)**: 支持状态的任意嵌套，实现逻辑复用与组合
- **转换策略 (HSM)**:
  - `Nested` — 嵌套模式：进入子状态时父状态保持激活
  - `Parallel` — 平行模式：进入子状态时父状态先退出再重新进入
- **退出行为 (HSM)**:
  - `Rebirth` — 重生：从子状态退出后重新执行父状态的 Enter
  - `Resurrection` — 复活：从子状态退出后进入父状态的 Update
  - `Death` — 死亡：父状态随之退出，向后继续级联
- **守卫机制 (HSM)**: 使用 `GuardEnter` / `GuardExit` 自动控制状态转换，支持 `and` / `or` / `not` 组合条件
- **转换系统**: `BeforeEnterSystem` / `AfterExitSystem` 关联转换专属系统
- **状态数据 (`state_data`)**: 进入/退出状态时自动为实体添加/移除组件和子实体
- **中断/恢复**: HSM 和 FSM 均支持基于栈的嵌套中断，可跨状态图/树跳转
- **历史状态 (`history`)**: 记录状态转换历史，支持回溯和 FSM 快照
- **Hybrid 架构**: 在 HSM 状态内部嵌套 FSM，实现"状态机内的状态机"
- **声明式宏**: `hsm!` / `fsm!` / `hsm_tree!` / `fsm_graph!` / `combination_condition!` / `system_registry!`
- **可定制调度**: 通过 `StateMachinePlugin::with_schedule()` 指定运行阶段
- **ServiceTarget**: 将状态机事件委托到独立实体，实现逻辑分离

---

## 快速开始

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
bevy_hsm = "0.19"
```

在 Bevy 应用中注册插件：

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default())
        .run();
}
```

### 第一个 HSM 示例

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, handle_input)
        .run();
}

fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    // 注册动作系统
    let enter_id = commands.register_system(|In(ctx): In<ActionContext>| {
        info!("进入状态: {:?}", ctx.state());
    });
    action_registry.insert("log_enter", enter_id);

    // 创建状态
    let root = commands.spawn((
        Name::new("Root"),
        HsmState::default(),
        AfterEnterSystem::new("log_enter"),
    )).id();

    let child = commands.spawn((
        Name::new("Child"),
        HsmState::default(),
        AfterEnterSystem::new("log_enter"),
    )).id();

    // 构建状态树
    let mut tree = StateTree::new(root);
    let tree_id = tree.with_child(root, child);

    // 生成状态机
    commands.spawn((
        tree,
        StateLifecycle::default(),
        HsmStateMachine::with(
            tree_id,
            root,
            #[cfg(feature = "history")] 10,
        ),
    ));
}
```

更多完整示例请参见 [examples/](examples/) 目录和 [examples/README.md](examples/README.md)。

---

## 核心概念

### 通用组件

| 组件 | 作用 |
| ----------- | --------- |
| `BeforeEnterSystem` | 进入状态前执行 |
| `AfterEnterSystem` | 进入状态后执行 |
| `OnUpdateSystem` | 状态激活时每帧执行 |
| `BeforeExitSystem` | 退出状态前执行 |
| `AfterExitSystem` | 退出状态后执行 |
| `Paused` | 暂停状态机，不响应任何转换 |
| `Terminated` | 标记状态机已终止 |
| `ServiceTarget` | 将状态机事件委托到指定实体 |
| `SpawnStateMachine` | 通过闭包延迟创建状态机 |

### 上下文类型

| 类型 | 用途 |
| ----------- | --------- |
| `ActionContext` | 传递给动作系统，包含 `service_target`、`state_machine`、`state` |
| `GuardContext` | 传递给守卫系统，包含 `service_target`、`state_machine`、`from_state`、`to_state` |
| `TransitionContext` | 传递给转换系统，包含 `service_target`、`state_machine`、转换关系 |

### 注册表

| 资源 | 用途 |
| ----------- | --------- |
| `ActionRegistry` | 注册动作系统 (按键值对) |
| `GuardRegistry` | 注册守卫系统 |
| `TransitionRegistry` | 注册转换系统 |
| `ActionDispatch` | 动作调度映射表 (自动管理) |

### HSM 核心

| 类型 | 说明 |
| ----------- | --------- |
| `HsmState` | HSM 状态组件，配置 `strategy` 和 `behavior` |
| `HsmStateMachine` | HSM 运行时组件，管理当前状态、状态树和转换队列 |
| `StateTree` | 状态树组件，定义父子层级关系 |
| `StateLifecycle` | 生命周期检测组件 (`Enter` / `Update` / `Exit`) |
| `HsmTrigger` | HSM 转换触发器，支持 `to_sub` / `to_super` / `chain` / `guard_sub` / `guard_super` / `interrupt` / `resume` |
| `GuardEnter` / `GuardExit` | 附加到状态的守卫组件 |
| `StateTransitionStrategy` | 转换策略 (`Nested` / `Parallel`) |
| `ExitTransitionBehavior` | 退出行为 (`Rebirth` / `Resurrection` / `Death`) |

### FSM 核心

| 类型 | 说明 |
| ----------- | --------- |
| `FsmState` | FSM 状态标记组件 |
| `FsmStateMachine` | FSM 运行时组件，管理当前状态和图引用 |
| `FsmGraph` | FSM 图组件，定义状态拓扑和转换规则 |
| `FsmTrigger` | FSM 转换触发器，支持 `next` / `guard` / `event` / `interrupt` / `resume` |
| `FsmBlueprint` | FSM 蓝图，用于 Hybrid 架构中在 HSM 状态内嵌套 FSM |

### 中断机制

HSM 和 FSM 均支持基于栈的中断/恢复机制。中断时保存当前状态和图，跳转到处理器状态；处理后通过 `Resume` 恢复。支持嵌套中断和跨图/树跳转。

```rust
// HSM: 保存当前状态并中断到处理状态
commands.trigger(HsmTrigger::interrupt(sm, target_tree, target_state));
// 处理完毕后恢复
commands.trigger(HsmTrigger::resume(sm));

// FSM: 保存当前状态并中断到其他图
commands.trigger(FsmTrigger::with_interrupt(sm, target_graph, target_state));
// 处理完毕后恢复
commands.trigger(FsmTrigger::with_resume(sm));
```

---

## 使用 hsm! / fsm! 宏

```rust
use bevy_hsm::prelude::*;

// HSM: 声明式定义状态树
hsm!(
    init(init_state = root, history_capacity = 10),
    #[state(on_update = "Update:log_update", behavior = Rebirth)]
    root: Root (
        #[state(guard_enter = "is_open", guard_exit = "is_close")]
        child: Child
    ),
    Paused
);

// FSM: 声明式定义状态图
fsm!(
    init(init_state = idle),
    states: {
        #[state(after_enter = "log_enter")]
        idle: Idle,
        #[state(after_enter = "log_enter")]
        walking: Walking
    },
    transitions: {
        idle => walking: event(ToggleEvent),
        walking => idle: event(ToggleEvent)
    }
);
```

---

## Cargo 特性

| 特性 | 默认 | 说明 |
| ----------- | --------- | --------- |
| `hsm` | ✓ (via hybrid) | 启用层级状态机 |
| `fsm` | ✓ (via hybrid) | 启用有限状态机 |
| `hybrid` | ✓ | 同时启用 `hsm` + `fsm`，支持嵌套架构 |
| `history` | ✓ | 状态转换历史记录 |
| `state_data` | ✓ | 状态关联的场景数据（组件/子实体自动管理） |

自定义配置：

```toml
[dependencies]
bevy_hsm = { version = "0.19", default-features = false, features = ["hsm", "history"] }
```

---

## 示例

所有示例位于 [examples/](examples/) 目录，详细说明见 [examples/README.md](examples/README.md)。

| 示例 | 说明 |
| ----------- | --------- |
| `hello_hsm` | HSM 入门：状态树、生命周期、ToSub/ToSuper |
| `simple_fsm` | FSM 入门：事件驱动转换 |
| `event_fsm` | FSM 事件系统与暂停/恢复 |
| `guard_hsm` | 守卫 (GuardEnter) 基本用法 |
| `guarded_counter` | Parallel 策略与 Guard 组合 |
| `flashing_light` | 计时器驱动的自动守卫转换 |
| `deep_chain` | 深层嵌套守卫导航 |
| `character_controller` | `hsm!` 宏声明式角色状态机 |
| `interrupt_hsm` | 中断/恢复与跨状态树转换 |
| `vending_machine` | 复合守卫、ServiceTarget、事件驱动 |
| `macros/hsm` | `hsm!` 宏 DSL 完整用法 |
| `macros/fsm` | `fsm!` 宏 DSL 完整用法 |
| `calculator` | HSM+FSM Hybrid 架构 GUI 应用 |

运行示例：

```bash
cargo run --example hello_hsm
cargo run --example calculator --features "hybrid, history"
```

---

## 协议

本项目采用 MIT 或 Apache 2.0 双协议，您可以根据需要选择其中之一。

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
