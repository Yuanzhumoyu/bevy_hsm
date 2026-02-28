# Bevy HSM - 混合状态机系统

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[English](./docs/en/README-en.md)

一个为 [Bevy 游戏引擎](https://bevyengine.org/) 设计的、强大的混合状态机系统。它无缝集成了**层级状态机 (Hierarchical State Machine, HSM)** 和**有限状态机 (Finite State Machine, FSM)**，让您可以为不同的场景选择最合适的工具。

- 使用 **HSM** 来管理应用中复杂的、高层级的行为状态，这些状态拥有自己的生命周期（进入、更新、退出）。
- 使用 **FSM** 来管理某个特定层级状态内部的、更简单的子状态，由事件驱动进行快速切换。

## 功能特性

- **混合模型**: 在一个统一的框架内同时支持 HSM 和 FSM。
- **状态生命周期**: 支持状态的 `OnEnter`、`OnUpdate` 和 `OnExit` 三个生命周期，并可关联独立的 Bevy 系统。
- **层级结构**: 支持状态的嵌套（父状态和子状态），实现逻辑的复用与组合。
- **灵活的转换触发器**:
  - **HSM**: 通过可组合的**条件系统** (`EnterGuard`, `ExitGuard`) 自动触发转换。
  - **FSM**: 通过发送**事件** (`FsmTrigger`) 来精确控制转换。
- **高级转换控制 (HSM)**:
  - **转换策略**: 可配置父子状态转换时的行为 (`StateTransitionStrategy`: `Nested` / `Parallel`)。
  - **返回行为**: 可配置子状态返回后，父状态的行为 (`ExitTransitionBehavior`: `Rebirth` / `Resurrection` / `Death`)。
- **Bevy 范式**: 整体架构遵循 Bevy 的 ECS 设计哲学，由组件、事件和系统驱动，与引擎无缝集成。
- **状态历史**: 内置状态转换历史记录功能，方便调试。

## 基本方法

在您的 Bevy 应用中添加 `StateMachinePlugin` 插件:

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default())
        // ... 在这里注册你的状态和系统
        .run();
}
```

## 核心概念

### 通用概念

- `OnEnterSystem` / `OnUpdateSystem` / `OnExitSystem`: 分别在状态进入、更新和退出时执行的系统。
- `GuardRegistry`: 用于注册和管理所有条件系统的资源。
- `Paused`: 一个标记组件，用于临时“暂停”一个状态机，使其不响应任何转换。
- `Terminated`: 一个标记组件，表示状态机已运行结束。

### 层级状态机 (HSM) - 状态驱动

HSM 的运行由其内部状态驱动，非常适合管理复杂的、有生命周期的行为。

- `HsmStateMachine`: HSM 的核心组件，管理当前状态、转换队列和历史记录。
- `StateLifecycle`: **HSM 的引擎**。这是一个特殊的组件，它的值 (`Enter`, `Update`, `Exit`) 决定了状态机当前所处的生命周期阶段，并通过 `on_insert` 钩子驱动所有逻辑。
- `StateTree`: 定义状态之间的父子层级关系。
- `EnterGuard` / `ExitGuard`: 附加在状态实体上的组件，用于指定进入或退出该状态的条件。

### 有限状态机 (FSM) - 事件驱动

FSM 的运行由外部事件驱动，非常适合响应式的、直接的状态切换。

- `FsmStateMachine`: FSM 的核心组件，管理当前状态和图。
- `FsmTrigger`: **FSM 的引擎**。这是一个 Bevy 事件，发送此事件会触发 FSM 的状态转换。
- `FsmGraph`: 定义一个 FSM 中所有有效的转换路径。一个转换必须在图中被定义才能执行。
- `StateEvent`: 一个 Trait，允许你使用自定义的任何类型（结构体、枚举、整数等）作为触发 FSM 转换的特定事件。

## Cargo 特性

本 crate 提供了一个条件编译特性：

- **`history`**: 为 `FsmStateMachine` 和 `HsmStateMachine` 启用状态历史记录功能。这让您可以查看已激活的状态序列。
- **`hybrid`**: 启用混合状态机功能, 同时支持 HSM 和 FSM。
- **`hsm`**: 启用 HSM 的功能。
- **`fsm`**: 启用 FSM 的功能。
要启用此特性，请将其添加到您的 `Cargo.toml` 文件中：

```toml
[dependencies]
bevy_hsm = { version = "0.18", features = ["history", "hsm", "fsm"] }
```

## 结语

`bevy_hsm` 仍处于积极开发阶段，后续会继续完善和添加新功能。欢迎通过提交 Issue 或 Pull Request 来帮助我改进这个库。

## 协议

本项目为 MIT 或 Apache 2.0 协议，您可以根据需要选择其中之一来使用本项目。

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
