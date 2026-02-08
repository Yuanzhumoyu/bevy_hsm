# Bevy HSM (Hierarchical State Machine)

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[English](./docs/en/README-en.md)

一个基于 Bevy 游戏引擎的分层状态机系统，实现了分层状态机的功能。

## 功能特性

- 支持状态的进入、更新和退出三个生命周期阶段
- 支持层次化状态（父状态和子状态）
- 支持状态转换条件
- 支持状态机系统和条件系统注册
- 提供状态转换历史记录功能
- 支持组合条件系统

## 使用方法

在您的 Bevy 应用中添加 HSM 插件:

```rust
use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(HsmPlugin::default())
        .run();
}
```

## 核心概念

- StateMachine: 管理实体的状态转换，包括当前状态、下一状态以及状态映射表
- HsmOnState: 状态生命周期管理，用于管理状态的进入、更新和退出
- StationaryStateMachine：用于将状态机静止
- Terminated: 用于表示状态机已终止
- StateConditions: 状态转换条件系统，用于判断状态是否满足进入或退出的条件
- HsmState: 表示一个状态，与主实体（拥有 StateMachine 组件的实体）相关联
- StateTree: 状态树，用于描述状态之间的转换关系
- HsmOnEnterCondition: 进入状态的条件
- HsmOnExitCondition: 退出状态的条件
- HsmOnEnterSystem: 进入状态的系统，用于在状态进入时执行逻辑
- HsmOnUpdateSystem: 更新状态的系统，用于在状态更新时执行逻辑
- HsmOnExitSystem: 退出状态的系统，用于在状态退出时执行逻辑

## 结语

目前还有bevy_hsm处于开发阶段，后续会继续完善和添加新功能，当然你可以提issue或pr来帮助完善这个库。

## 协议

本项目为 MIT 或 Apache 2.0 协议，您可以根据需要选择其中之一来使用本项目。

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
