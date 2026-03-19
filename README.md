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
  - **HSM**: 支持通过可组合的**条件系统** (`EnterGuard`, `ExitGuard`) 自动转换，或通过发送**事件** (`HsmTrigger`) 进行精确控制。
  - **FSM**: 通过发送**事件** (`FsmTrigger`) 来精确控制转换。
- **高级转换控制 (HSM)**:
  - **转换策略(`StateTransitionStrategy`)**: 可配置父子状态转换时的行为。
    - `Nested`: 嵌套模式。进入子状态时，父状态保持激活，子状态的生命周期在父状态内部执行。
    - `Parallel`:  平行模式。进入子状态前，父状态会先退出，两者生命周期是分离的。
  - **返回行为(`ExitTransitionBehavior`)**: 可配置子状态返回后，父状态的行为。
    - `Rebirth`: 重生。重新触发父状态的OnEnter。
    - `Resurrection`: 复活。返回到父状态的OnUpdate。
    - `Death`: 死亡。父状态也随之退出，并继续向上层传递退出行为。
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
- `StateLifecycle`: **HSM 的状态驱动引擎**。这是一个特殊的组件，它的值 (`Enter`, `Update`, `Exit`) 决定了状态机当前所处的生命周期阶段，并通过 `on_insert` 钩子驱动所有逻辑。
- `HsmTrigger`: **HSM 的事件驱动引擎**。这是一个 Bevy 事件，发送此事件会触发 HSM 的状态转换，提供了命令式的控制方式。
- `StateTree`: 定义状态之间的父子层级关系。
- `EnterGuard` / `ExitGuard`: 附加在状态实体上的组件，用于指定进入或退出该状态的条件。

### 有限状态机 (FSM) - 事件驱动

FSM 的运行由外部事件驱动，非常适合响应式的、直接的状态切换。

- `FsmStateMachine`: FSM 的核心组件，管理当前状态和图。
- `FsmTrigger`: **FSM 的引擎**。这是一个 Bevy 事件，发送此事件会触发 FSM 的状态转换。
- `FsmGraph`: 定义一个 FSM 中所有有效的转换路径。一个转换必须在图中被定义才能执行。
- `StateEvent`: 一个 Trait，允许你使用自定义的任何类型（结构体、枚举、整数等）作为触发 FSM 转换的特定事件。

## 宏语法 (EBNF)

### `hsm!`

`hsm!` 宏用于构建一个层级状态机（Hierarchical State Machine）。它定义了一个树状结构，其中包含一个根状态，以及可选的、附加到状态机实体上的额外 Bevy 组件。

```ebnf
hsm ::= state_node, { ',', component }, [ ',', ':', config_fn];
state_node ::= { state_attribute }, [ ':', state_name ], [ '(', { state_content }, ')' ];
state_content ::= ( state_node | component ), { ',', ( state_node | component ) };
state_attribute ::= '#[state', [ '(', state_attribute_param, { ',', state_attribute_param }, ')' ], ']'
                  | '#[state_data(', component, { ',', component }, ')]';
state_attribute_param ::= 'enter_guard' '=' guard_condition
                        | 'exit_guard' '=' guard_condition
                        | 'on_update' '=' string_literal
                        | 'on_enter' '=' string_literal
                        | 'on_exit' '=' string_literal
                        | 'strategy' '=' ( 'Nested' | 'Parallel' )
                        | 'behavior' '=' ( 'Rebirth' | 'Resurrection' | 'Death' )
                        | 'fsm_blueprint' '=' rust_expression
                        | 'minimal';
config_fn ::= expr_closure
            | fn_identifier;
guard_condition ::= rust_expression; (* 任何返回 bool 的 Rust 表达式 *)
component ::= rust_expression; (* 任何有效的 Bevy 组件 *)
state_name ::= identifier; (* 状态的名称 *)
identifier ::= (* Rust 标识符, e.g., MyState, StateA *) ;
string_literal ::= (* Rust 字符串字面量, e.g., "my_system" *) ;
rust_expression ::= (* 任何有效的 Rust 表达式 *) ;
expr_closure ::= (* Rust 闭包, e.g., |entity_commands: EntityCommands, states: &[Entity]| { ... } *) ;
fn_identifier ::= (* Rust 函数标识符, e.g., my_function *, 参数类型为 `fn(EntityCommands, &[Entity]){ ... }` *) ;
```

**关键点**:

- `hsm!` 宏的核心是一个 `state_node`，它代表状态树的根。
- 在根状态之后，您可以附加任意数量的 Bevy `component`，它们会和状态机一起被添加到同一个实体上。
- `state_node` 可以通过 `#[state(...)]` 属性进行配置，例如设置 `guard`、生命周期钩子（`on_update` 等）和层级行为（`strategy`, `behavior`）。
- `#[state_data(...)]` 属性用于附加只在该状态激活时才存在的组件。
- 状态可以嵌套。子状态和子组件都定义在父状态的 `()` 内部。

### `fsm!`

`fsm!` 宏用于构建一个扁平的有限状态机（Finite State Machine）。它定义了一组状态、一组转换规则，以及可选的附加组件。

```ebnf
fsm ::= fsm_graph, [ ',', 'components', ':', '{', [ component, { ',', component } ], '}' ],[',', ':', config_fn] ,[','];
fsm_graph ::= 'states', [ '<', state_ref, '>' ], ':', '{', state_definition, { ',', state_definition }, '}', [','],
              'transitions', ':', '{', transition, { ',', transition }, '}';
state_definition ::= { state_attribute }, [ ':', state_name ], [ '(', { component }, ')' ];
transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref [ ':', transition_condition ];
transition_condition ::= 'event', '(', rust_expression ')' (* 事件 *)
                       | 'guard', '(', guard_expression ')'; (* 条件守卫 *)
state_ref ::= identifier | integer_literal; (* 状态名称或索引 *)
(* `state_attribute`, `component`, `state_name`, `identifier`, `string_literal`, `rust_expression`, `config_fn` 的定义与 hsm! 宏相同 *)
```

**关键点**:

- `fsm!` 宏由三个部分组成：`fsm_graph` 一个可选的 `components` 块和一个可选的 `config_fn`。
- `fsm_graph` 是必需的，它包含 `states` 和 `transitions` 两个块。
- `states<...>` 语法允许您通过名称或索引（`state_ref`）来指定初始状态。如果省略，则列表中的第一个状态为初始状态。
- `state_definition` 的语法与 `hsm!` 中的 `state_node` 类似，但它不能嵌套其他状态。
- `transition` 定义了状态之间的转换规则，可以是有条件的（通过事件或 `guard`）或无条件的。
  - 箭头定义了转换的方向。存在三种有效的模式：
    - A => B: 表示从 A 到 B 的单向转换。
    - A <= B: 表示从 B 到 A 的单向转换。
    - A <=> B: 表示 A 和 B 之间的双向转换。
  - 请注意，转换条件两侧的箭头必须匹配。

### `hsm_tree!`

`hsm_tree!` 是一个工具宏，用于单独构建一个状态树（`StateTree`）。它的语法是 `hsm!` 宏的一个子集，只接受一个根 `state_node`。

```ebnf
hsm_tree ::= state_node;
 
(* `state_node` 的定义与 `hsm!` 宏中的完全相同。 *)
```

### `fsm_graph!`

`fsm_graph!` 是一个工具宏，用于单独构建一个状态图（`FsmGraph`）。它的语法是 `fsm!` 宏的一个子集。

```ebnf
fsm_graph ::= 'states', [ '<', state_ref, '>' ], ':', '{', state_definition, { ',', state_definition }, '}', ',',
              'transitions', ':', '{', transition, { ',', transition }, '}';
 
(* `state_ref`, `state_definition`, `transition` 的定义与 `fsm!` 宏中的完全相同。 *)
```

### `combination_condition!`

`combination_condition!` 用于在 `#[state]` 属性中构建复杂的组合守卫条件。

```ebnf
combination_condition ::= guard_expression;
 
guard_expression ::= 'and', '(', guard_expression, ',', guard_expression, { ',', guard_expression }, ')'
                   | 'or', '(', guard_expression, ',', guard_expression, { ',', guard_expression }, ')'
                   | 'not', '(', guard_expression, ')'
                   | guard_id;
guard_id ::= LitStr
           | '#' identifier
```

## Cargo 特性

本 crate 提供了一个条件编译特性：

- **`history`**: 为 `FsmStateMachine` 和 `HsmStateMachine` 启用状态历史记录功能。这让您可以查看已激活的状态序列。
- **`state_data`**: 启用 `StateData` 功能。允许您将组件作为“状态本地数据”附加到状态上，当状态机进入该状态时，这些组件会被自动克隆到状态机实体上；离开时则被移除。
- **`hybrid`**: 启用混合状态机功能, 同时支持 HSM 和 FSM。
- **`hsm`**: 启用 HSM 的功能。
- **`fsm`**: 启用 FSM 的功能。
要启用此特性，请将其添加到您的 `Cargo.toml` 文件中：

```toml
[dependencies]
bevy_hsm = { version = "0.18", features = ["history", "hybrid"] }
```

## 结语

`bevy_hsm` 仍处于积极开发阶段，后续会继续完善和添加新功能。欢迎通过提交 Issue 或 Pull Request 来帮助我改进这个库。

## 协议

本项目为 MIT 或 Apache 2.0 协议，您可以根据需要选择其中之一来使用本项目。

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
