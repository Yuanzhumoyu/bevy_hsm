# bevy_hsm

一个基于 Bevy 引擎的分层状态机（Hierarchical State Machine）实现，用于管理游戏实体的复杂状态转换逻辑。

## 简介

`bevy_hsm` 是一个为 Bevy 游戏引擎设计的状态机系统，它允许您轻松地管理实体在不同状态之间的转换。该库支持分层状态机结构，包括父状态和子状态，以及状态转换条件检查。

## 特性

- ✅ 分层状态机支持（父状态和子状态）
- ✅ 状态生命周期管理（进入、更新、退出）
- ✅ 状态转换条件检查
- ✅ 状态优先级管理
- ✅ 与 Bevy ECS 架构无缝集成
- ✅ 灵活的状态系统注册机制
- ✅ 类型安全的状态转换

## 核心概念

------------------

- StateMachines
    管理实体的状态转换，包括当前状态、下一状态以及状态映射表。

- HsmState
    表示一个状态，与主实体（拥有 StateMachines 组件的实体）相关联。

### HsmOnState

### 状态机生命周期的三个阶段

- Enter：进入状态
- Update：更新状态
- Exit：退出状态

### 状态转换条件[StateTransitionCondition]

- HsmOnEnterCondition：进入状态的条件
- HsmOnExitCondition：退出状态的条件

### 分层状态

- SuperState：父状态
- SubStates：子状态集合

### 状态优先级[StateHistory]

- 允许你通过优先级从高到低遍历顺序来管理状态转换。
- 优先级越高，越先被检查。

### 状态过渡规划[StateTransitionStrategy]

- 允许当状态从子状态转换到父状态时，通过[StateTransitionStrategy]确定是否重置,继续和直接退出

### 组合条件[CombinationCondition]

- 允许你使用多个条件来组合多个条件，支持and,or,not操作符。
- 同时允许使用combination_condition!(and("a","b"))或者CombinationCondition::parse("And(a,b)")编写组合条件。

## API 文档

有关完整 API 文档，请查看 docs.rs（如果已发布）或使用 cargo doc --open 生成本地文档。

## 贡献

欢迎提交 Issue 和 Pull Request 来改进这个库！

## 协议

本项目为 MIT 或 Apache 2.0 协议，您可以根据需要选择其中之一来使用本项目。

- MIT License ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
