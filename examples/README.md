# Examples / 示例

本目录包含 bevy_hsm 的示例程序，涵盖从基础入门到高级用法的各种场景。

This directory contains example programs for bevy_hsm, covering everything from basic to advanced usage.

---

## 示例列表 / Example List

| 文件 | 功能介绍 | 核心概念 | 操作方式 |
| ------ | ---------- | ---------- | ---------- |
| [`hello_hsm.rs`](hello_hsm.rs) | HSM 入门：状态树构建、生命周期（Enter/Update/Exit）、ToSub/ToSuper 父子状态导航 | `StateTree`, `HsmState`, `StateLifecycle`, `ToSub`, `ToSuper`, `AfterEnterSystem`, `BeforeExitSystem`, `OnUpdateSystem` | 空格切换 Root↔Child, Esc 退出 |
| [`simple_fsm.rs`](simple_fsm.rs) | FSM 入门：事件驱动的有限状态机基本用法，自定义事件在状态间切换 | `FsmState`, `FsmGraph`, `StateEvent`, `FsmTrigger::with_event`, `BeforeEnterSystem`, `AfterExitSystem`, `Paused` | 空格发送 ToggleEvent 切换 A↔B, P 暂停/恢复 |
| [`event_fsm.rs`](event_fsm.rs) | 事件驱动 FSM：在 Red 和 Green 之间通过 ToggleEvent 循环切换，演示暂停/恢复 | `FsmGraph::with_event`, `FsmTrigger::with_event`, `Paused` | 空格切换 Red↔Green, P 暂停/恢复 |
| [`guard_hsm.rs`](guard_hsm.rs) | 守卫状态机：通过 GuardEnter 控制是否允许进入子状态（锁机制） | `GuardEnter`, `GuardRegistry`, `HsmTrigger::guard_sub` | 空格切换锁, Enter 返回 Root |
| [`guarded_counter.rs`](guarded_counter.rs) | 守卫计数器：GuardEnter/GuardExit 与 Parallel 策略组合，自动在计数和停止间切换 | `GuardEnter`, `GuardExit`, `StateTransitionStrategy::Parallel`, `OnUpdateSystem` | 空格切换 Open/Close 控制计数 |
| [`flashing_light.rs`](flashing_light.rs) | 闪烁灯：基于计时器的守卫实现自动循环状态切换（红黄交替） | `GuardEnter`, `GuardExit`, `StateTransitionStrategy::Parallel`, 计时器守卫 | 空格暂停/恢复闪烁 |
| [`deep_chain.rs`](deep_chain.rs) | 深层状态链：四级嵌套 (OFF→ON1→ON2→ON3)，通过 GuardEnter/GuardExit 及方向键逐级导航 | `GuardEnter`, `GuardExit`, `BeforeEnterSystem`, `AfterExitSystem`, `HsmTrigger::guard_sub` | ↑ 向下进入, ↓ 向上退出 |
| [`character_controller.rs`](character_controller.rs) | 角色控制器：使用 `hsm!` 宏声明式定义角色状态机，Chain 转换在互斥分支间切换 | `hsm!` 宏, `Chain 转换`, `ExitTransitionBehavior::Death`, LCA 路径 | Num 0/1/2/3 切换状态 |
| [`interrupt_hsm.rs`](interrupt_hsm.rs) | 中断与恢复：跨状态树中断到紧急状态（Alert），处理完毕后恢复原状态 | `Interrupt`, `Resume`, `InterruptStack`, 跨状态树转换 | I 中断到 Alert, R 恢复 |
| [`vending_machine.rs`](vending_machine.rs) | 售货机：带复合守卫 `and(has_enough_money, is_in_stock)` 的 FSM，商品购买流程 | `FsmGraph::with_condition`, `GuardCondition::and`, `ServiceTarget`, `StateEvent` | P 尝试购买, M 加钱, R 补货 |
| [`calculator.rs`](calculator.rs) | 交互式计算器：HSM+FSM 混合架构，HSM 管理命令(清除/等号/退格/符号)，FSM 管理表达式解析 | `hsm!` 宏, `fsm_graph!` 宏, Hybrid 架构, `FsmBlueprint`, `TransitionContext`, `chain`, 隐式乘法 | 鼠标点击虚拟键盘 |
| [`macros/hsm.rs`](macros/hsm.rs) | `hsm!` 宏声明式定义：用 DSL 语法定义状态树、守卫、动作系统、退出行为和 state_scene 数据组件 | `hsm!` 宏, `state_scene`, `guard_enter`/`guard_exit` 属性, `behavior=Rebirth` | ↑ 进入子状态, ↓ 退出到父状态 |
| [`macros/fsm.rs`](macros/fsm.rs) | `fsm!` 宏声明式定义：用 DSL 语法定义状态、事件转换和组件，一步完成状态机搭建 | `fsm!` 宏, `states`/`transitions`/`components` DSL, 事件驱动转换, 状态数据组件 | ↑ 发送 MyEvent::Go 前进, ↓ 发送 MyEvent::Back 后退 |

---

## 宏示例 / Macro Examples

`macros/` 子目录包含使用过程宏 (`hsm!` / `fsm!`) 声明式定义状态机的示例。宏将状态树、守卫、动作系统和转换规则集中在一段 DSL 中，减少样板代码。

The `macros/` subdirectory contains examples that use procedural macros (`hsm!` / `fsm!`) to declaratively define state machines. The macros consolidate state trees, guards, action systems, and transition rules into a single DSL, reducing boilerplate.

| 文件 | 功能介绍 | 宏特征 |
| ------ | ---------- | ---------- |
| [`macros/hsm.rs`](macros/hsm.rs) | 用 `hsm!` 宏构建带守卫的层级状态机，演示 `state_scene` 数据注入 | `guard_enter`, `guard_exit`, `behavior`, `state_scene` |
| [`macros/fsm.rs`](macros/fsm.rs) | 用 `fsm!` 宏构建事件驱动的有限状态机，演示组件附加 | `states`, `transitions`, `components` |

---

## 学习路径 / Learning Path

### 入门 / Beginner

1. **`hello_hsm.rs`** — 理解 HSM 的核心概念：状态树、生命周期、转换
2. **`simple_fsm.rs`** — 理解 FSM 的基本用法：事件驱动转换

### 进阶 / Intermediate

1. **`guard_hsm.rs`** — 学习守卫 (Guard) 的基本用法
2. **`event_fsm.rs`** — 深入 FSM 事件系统与暂停机制
3. **`guarded_counter.rs`** — 理解 Parallel 策略与 Guard 的组合
4. **`flashing_light.rs`** — 理解计时器驱动的自动守卫转换

### 高级 / Advanced

1. **`deep_chain.rs`** — 深层嵌套守卫导航
2. **`character_controller.rs`** — 使用 `hsm!` 宏声明式构建复杂状态机
3. **`interrupt_hsm.rs`** — 理解中断/恢复机制和跨状态树转换
4. **`vending_machine.rs`** — 理解复合守卫、ServiceTarget 和事件驱动购买流程
5. **`macros/hsm.rs`** — 学习 `hsm!` 宏的 DSL 语法：`state_scene`、`guard_enter`/`guard_exit`、`behavior`
6. **`macros/fsm.rs`** — 学习 `fsm!` 宏的 DSL 语法：`states`、`transitions`、`components`
7. **`calculator.rs`** — HSM+FSM 混合架构的综合应用

---

## 运行示例 / Running Examples

```bash
# 运行指定示例
cargo run --example hello_hsm

# 运行宏示例 (macros 子目录)
cargo run --example macros_hsm
cargo run --example macros_fsm

# 带 features 运行
cargo run --example vending_machine --features history
cargo run --example calculator --features "hybrid, history"
cargo run --example macros_hsm --features "state_data"

# 所有示例均支持的基本 features
cargo run --example <name> --features "hsm, fsm"
```
