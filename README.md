# Bevy HSM - 混合状态机系统

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[English](./docs/en/README-en.md)

一个为 [Bevy 游戏引擎](https://bevyengine.org/) 设计的、强大的混合状态机系统。它无缝集成了**层级状态机 (Hierarchical State Machine, HSM)** 和**有限状态机 (Finite State Machine, FSM)**，让您可以为不同的场景选择最合适的工具。

- 使用 **HSM** 来管理应用中复杂的、高层级的行为状态，这些状态拥有自己的生命周期（进入、更新、退出）。
- 使用 **FSM** 来管理某个特定层级状态内部的、更简单的子状态，由事件驱动进行快速切换。

## 功能特性

- **混合模型**: 在一个统一的框架内同时支持 HSM 和 FSM。
- **状态生命周期**: 支持状态的 `Enter`、`Update` 和 `Exit` 三个生命周期，并可关联独立的 Bevy 系统。
- **层级结构**: 支持状态的嵌套（父状态和子状态），实现逻辑的复用与组合。
- **灵活的转换触发器**:
  - **HSM**: 支持通过可组合的**条件系统** (`GuardEnter`, `GuardExit`) 自动转换，或通过发送**事件** (`HsmTrigger`) 进行精确控制。
  - **FSM**: 通过发送**事件** (`FsmTrigger`) 来精确控制转换。
- **高级转换控制 (HSM)**:
  - **转换策略(`StateTransitionStrategy`)**: 可配置父子状态转换时的行为。
    - `Nested`: 嵌套模式。进入子状态时，父状态保持激活，子状态的生命周期在父状态内部执行。
    - `Parallel`:  平行模式。进入子状态前，父状态会先退出，两者生命周期是分离的。
  - **返回行为(`ExitTransitionBehavior`)**: 可配置子状态返回后，父状态的行为。
    - `Rebirth`: 重生。重新触发父状态的Enter。
    - `Resurrection`: 复活。返回到父状态的Update。
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

- `BeforeEnterSystem` / `AfterEnterSystem` / `OnUpdateSystem` / `BeforeExitSystem` / `AfterExitSystem`: 分别在状态进入前、进入后、更新、退出前和退出后执行的系统。
- `ActionRegistry` / `GuardRegistry` / `TransitionRegistry`: 分别用于注册和管理所有动作、守卫和转换系统的资源。
- `ActionContext` / `GuardContext` / `TransitionContext`: 这些是专门的系统参数，用于在动作、守卫和转换系统中提供关于状态和转换的上下文信息。例如，`ActionContext` 提供了当前状态的实体，而 `GuardContext` 提供了转换的来源和目标状态。
- `Paused`: 一个标记组件，用于临时“暂停”一个状态机，使其不响应任何转换。
- `Terminated`: 一个标记组件，表示状态机已运行结束。

### 层级状态机 (HSM) - 状态驱动

HSM 的运行由其内部状态驱动，非常适合管理复杂的、有生命周期的行为。它支持两种驱动模式：

- **状态驱动 (自动)**: 通过 `StateLifecycle` 组件。这是一个特殊的组件，它的值 (`Enter`, `Update`, `Exit`) 决定了状态机当前所处的生命周期阶段，并通过 `on_insert` 钩子驱动所有逻辑。这种模式通常用于由状态自身条件触发的自动转换。
- **事件驱动 (手动)**: 通过发送 `HsmTrigger` 事件。这是一个 Bevy 事件，发送此事件会强制触发 HSM 的状态转换，提供了命令式的、精确的控制方式。
- `StateTree`: 定义状态之间的父子层级关系。
- `GuardEnter` / `GuardExit`: 附加在状态实体上的组件，用于指定进入或退出该状态的条件。

#### HSM 高级功能

##### 转换策略 (Transition Strategy)

通过 `strategy` 属性，可以控制当进入或退出一个父状态时，其子状态的行为。

- **`Nested`** (嵌套, 默认): 父状态保持激活，子状态的进入和退出发生在父状态的生命周期内部。
- **`Parallel`** (平行): 转换时，父状态会先退出，然后子状态完成其生命周期，之后父状态可能会根据 `ExitTransitionBehavior` 重新进入。

##### 状态行为 (State Behavior)

通过 `behavior` 属性，可以定义当一个状态被重新进入时的行为。

- **`Rebirth`** (重生): 从子状态退出后，父状态会重新执行其 `Enter` 阶段。
- **`Resurrection`** (复活, 默认): 从子状态退出后，父状态会直接进入其 `Update` 阶段。
- **`Death`** (死亡): 从子状态退出后，父状态自身也会退出，并将退出行为继续向上传递。

##### 历史状态 (History State)

HSM 支持历史状态功能。通过在 `hsm!` 宏的 `init` 部分设置 `history_capacity`，状态机可以“记住”最近访问过的子状态。当一个父状态被重新进入时，它可以直接恢复到上次离开时的那个子状态，而不是其初始子状态，这对于实现类似“返回”的功能非常有用。

#### 插件配置

##### 自定义调度 (Custom Scheduling)

默认情况下，状态机系统在 `Last` 调度阶段运行。如果需要更精细的控制，你可以通过 `StateMachinePlugin::with_schedule(MySchedule)` 来指定状态机系统在你的自定义调度阶段运行。

```rust,ignore
use bevy::prelude::*;
use bevy_hsm::prelude::*;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct MyUpdate;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::with_schedule(MyUpdate))
        .run();
}
```

### 有限状态机 (FSM) - 事件驱动

FSM 的运行由外部事件驱动，非常适合响应式的、直接的状态切换。

- `FsmState`: 一个标记组件，用于将一个实体标识为 FSM 的状态。
- `FsmStateMachine`: FSM 的核心组件，管理当前状态和图。
- `FsmTrigger`: **FSM 的事件引擎**。这是一个 Bevy 事件，用于驱动 FSM 进行状态转换。您可以通过它来触发无条件转换，或包装一个自定义事件（如枚举或结构体）来触发一个特定的事件驱动转换。
- `FsmGraph`: 定义一个 FSM 中所有有效的转换路径。一个转换必须在图中被定义才能执行。

## 宏语法 (EBNF)

### `hsm!`

`hsm!` 宏用于构建一个层级状态机（Hierarchical State Machine）。它定义了一个树状结构，其中包含一个根状态，以及可选的、附加到状态机实体上的额外 Bevy 组件。

```ebnf
hsm ::= [ machine_config, ',', ], state_node, { ',', component }, [ ',', config_fn ];
machine_config ::= 'init', '(', [ machine_config_param, { ',', machine_config_param } ], ')';
machine_config_param ::= 'history_capacity', '=', integer_literal
                       | ( 'init_state' | 'curr_state' ), '=', state_ref;
state_node ::= state_attribute, [ ':', state_name ], [ '(', { state_content }, ')' ];
state_content ::= ( state_node | component ), { ',', ( state_node | component ) };
state_attribute ::= '#[state', [ '(', state_attribute_param, { ',', state_attribute_param }, ')' ], ']' 
                  | '#[state_data(', component, { ',', component }, ')]';
state_attribute_param ::= ( 'guard_enter' | 'guard_exit' ), '=', guard_expression
                        | ( 'before_enter' | 'after_enter' | 'before_exit' | 'after_exit' ), '=', action_id
                        | 'on_update', '=', lit_str
                        | 'strategy', '=', ( 'Nested' | 'Parallel' )
                        | 'behavior', '=', ( 'Rebirth' | 'Resurrection' | 'Death' )
                        | 'fsm_blueprint', '=', rust_expression
                        | 'minimal';
config_fn ::= ':', ( expr_closure | fn_identifier | expr_call );
component ::= rust_expression; (* 任何有效的 Bevy 组件 *)
state_name ::= identifier; (* 状态的名称 *)
state_ref ::= identifier | integer_literal;
action_id ::= lit_str
            | fn_identifier
            | action_name, ':', ( expr_closure | expr_call | fn_identifier );
action_name ::= identifier;
identifier ::= (* Rust 标识符, e.g., MyState, StateA *) ;
lit_str ::= (* Rust 字符串字面量, e.g., "my_system" *) ;
rust_expression ::= (* 任何有效的 Rust 表达式 *) ;
expr_closure ::= (* Rust 闭包, e.g., 签名需为 `|EntityCommands, &[Entity]|{...}` *) ;
fn_identifier ::= (* Rust 函数标识符, e.g., my_function *, 签名需为 `fn(EntityCommands, &[Entity])` *) ;
expr_call ::= (* 任何有效的 Rust 函数调用表达式, e.g., my_function(a, b) *) ;
```

**关键点**:

- `hsm!` 宏的核心是一个 `state_node`，它代表状态树的根。
- 在根状态之后，您可以附加任意数量的 Bevy `component`，它们会和状态机一起被添加到同一个实体上。
- `state_node` 可以通过 `#[state(...)]` 属性进行配置。除了通用的生命周期钩子（如 `on_update`, `after_enter`）外，它还支持 HSM 独有的属性，包括用于自动转换的守卫（`guard_enter`, `guard_exit`）和用于控制层级行为的 `strategy`、`behavior` 等。
- `#[state_data(...)]` 属性用于附加只在该状态激活时才存在的组件。
- 状态可以嵌套。子状态和子组件都定义在父状态的 `()` 内部。

### `fsm!`

`fsm!` 宏用于构建一个扁平的有限状态机（Finite State Machine）。它定义了一组状态、一组转换规则，以及可选的附加组件。

```ebnf
fsm ::= [ machine_config, ',' ], fsm_graph, [ ',', 'components', ':', '{', [ component, { ',', component } ], '}' ], [ ',', config_fn ];
fsm_graph ::= 'states', ':', '{', state_definition, { ',', state_definition }, '}', ',',
              'transitions', ':', '{', transition, { ',', transition }, '}';
state_definition ::= state_attribute, [ ':', state_name ], [ '(', { component }, ')' ];
transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref, [ ':', transition_condition ];
transition_condition ::= 'event', '(', rust_expression ')' (* 事件 *)
                       | 'guard', '(', guard_expression ')'; (* 条件守卫 *)
state_ref ::= identifier | integer_literal; (* 状态名称或索引 *)
(* `state_attribute`, `component`, `state_name`, `identifier`, `lit_str`, `rust_expression`, `config_fn`, `action_id`, `machine_config`, `state_ref`, `fsm_graph` 的定义与 hsm! 宏相同 *)

```

**关键点**:

- `fsm!` 宏由三个部分组成：`fsm_graph` 一个可选的 `components` 块和一个可选的 `config_fn`。
- `fsm_graph` 是必需的，它包含 `states` 和 `transitions` 两个块。
- `state_definition` 的语法与 `hsm!` 中的 `state_node` 类似，但它不能嵌套其他状态。
- `state_definition` 同样支持 `#[state(...)]` 和 `#[state_data(...)]` 属性。但请注意，由于 FSM 是扁平且由事件驱动的结构，`#[state(...)]` 中与 HSM 自动转换和层级相关的参数（如 `guard_enter`, `guard_exit`, `strategy`, `behavior`）在此处是无效的。
- `transition` 定义了状态之间的转换规则，可以是有条件的（通过事件或 `guard`）或无条件的。
  - 箭头定义了转换的方向。存在三种有效的模式：
    - A => B: 表示从 A 到 B 的单向转换。
    - A <= B: 表示从 B 到 A 的单向转换。
    - A <=> B: 表示 A 和 B 之间的双向转换。
  - 请注意，转换条件两侧的箭头必须匹配。

### `hsm_tree!`

`hsm_tree!` 是一个工具宏，用于单独构建一个状态树（`StateTree`）。它的语法是 `hsm!` 宏的一个子集，只接受一个根 `state_node`。

```ebnf
hsm_tree ::= state_node, [ ',', config_fn ];

(* `state_node` 和 `config_fn` 的定义与 `hsm!` 宏相同。 *)
```

### `fsm_graph!`

`fsm_graph!` 是一个工具宏，用于单独构建一个状态图（`FsmGraph`）。它的语法是 `fsm!` 宏的一个子集。

```ebnf
fsm_graph! ::= fsm_graph, [ ',', config_fn ];

(* `fsm_graph` 和 `config_fn` 的定义与 `fsm!` 宏相同。 *)
```

### `system_registry!`

`system_registry!` 是一个辅助宏，用于将多个 Bevy 系统动态注册到一个 `SystemRegistry` 资源中。这在您需要将一组相关的系统（例如，作为状态动作）传递给状态机时非常有用。

```ebnf
system_registry ::= '<', source, ',', system_registry, '>', '[', [ system_definition, { ',', system_definition } ], ']';
system_definition ::= ( lit_str | fn_identifier ), '=>', rust_expression;

source ::= identifier; (* `Commands` 或 `World` 类型的变量 *)
system_registry ::= identifier; (* 实现了 `Extend<(String, SystemId)>` 的类型的变量 *)
lit_str ::= (* system_registry 中的唯一名称 *)
fn_identifier ::= (* system_registry 中的唯一名称 *)
rust_expression ::= (* Bevy 系统 (函数或闭包) *)
```

**示例**:

```rust
let mut system_registry = SystemRegistry::new();
system_registry!(<commands, system_registry>[
    "on_enter_a" => on_enter_a,
    "on_update_a" => || info!("Updating A"),
]);
```

### `combination_condition!`

`combination_condition!` 用于在 `#[state]` 属性中构建复杂的组合守卫条件。

```ebnf
combination_condition ::= guard_expression;
 
guard_expression ::= ( 'and' | 'or' ), '(', guard_expression, ',', guard_expression, { ',', guard_expression }, ')'
                   | 'not', '(', guard_expression, ')'
                   | guard_id;
guard_id ::= lit_str | ( '#', identifier );
```

## Cargo 特性

本 crate 提供了以下 Cargo 特性：

- **`hsm`** (默认启用): 启用层级状态机（HSM）功能。
- **`fsm`** (默认启用): 启用有限状态机（FSM）功能。
- **`hybrid`**: 一个便捷特性，同时启用 `hsm` 和 `fsm`。
- **`history`**: 为状态机启用历史记录功能，允许您追踪状态转换序列。
- **`state_data`**: 启用 `StateData` 功能，允许您将组件作为“状态本地数据”附加到状态上。
默认情况下，`hybrid` , `history`和 `state_data` 都已启用。如果您想自己配置，可以这样做：

```toml
[dependencies]
bevy_hsm = { version = "0.18", default-features = false, features = ["history", "hybrid"] }
```

## 结语

`bevy_hsm` 仍处于积极开发阶段，后续会继续完善和添加新功能。欢迎通过提交 Issue 或 Pull Request 来帮助我改进这个库。

## 协议

本项目为 MIT 或 Apache 2.0 协议，您可以根据需要选择其中之一来使用本项目。

- MIT License ([LICENSE-MIT](LICENSE-MIT.txt) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
