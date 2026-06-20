// --- Always available ---
mod guard_condition;
mod kw;

// --- Available when either FSM or HSM is enabled ---
#[cfg(any(feature = "fsm", feature = "hsm"))]
mod action_id;
#[cfg(any(feature = "fsm", feature = "hsm"))]
mod machine_config;
#[cfg(any(feature = "fsm", feature = "hsm"))]
mod state_config;

// --- FSM-only modules ---
#[cfg(feature = "fsm")]
mod fsm;
#[cfg(feature = "fsm")]
mod fsm_graph;

// --- HSM-only modules ---
#[cfg(feature = "hsm")]
mod hsm;
#[cfg(feature = "hsm")]
mod hsm_tree;

use proc_macro::TokenStream;

// ── combination_condition! ───────────────────────────────────────────────

/// # 组合守卫条件\Combination Guard Condition
///
/// 将多个守卫条件组合为一个复合条件表达式。支持 `and`、`or`、`not` 逻辑运算符，
/// 可任意嵌套。通常在 `#[state(guard_enter=...)]` 中使用，也可通过 `#variable`
/// 语法引用外部变量。
///
/// Combines multiple guard conditions into a single composite condition
/// expression. Supports `and`, `or`, `not` logical operators with arbitrary
/// nesting. Typically used inside `#[state(guard_enter=...)]`, and can also
/// reference external variables via `#variable` syntax.
///
/// # 逐步语法说明\Step-by-Step Syntax
///
/// ## 1. 叶子守卫\Leaf Guard
///
/// 最基础的守卫形式有两种：
/// There are two basic guard forms:
///
/// * **字符串守卫** — 按名称引用 [`GuardRegistry`] 中注册的守卫系统：
///   **String guard** — references a guard system registered in [`GuardRegistry`] by name:
///   ```rust,ignore
///   combination_condition!("my_guard")
///   ```
///
/// * **变量引用** — 通过 `#` 前缀引用作用域内已有的 [`GuardCondition`] 变量：
///   **Variable reference** — references an in-scope [`GuardCondition`] variable via `#`:
///   ```rust,ignore
///   let my_cond = combination_condition!(and("a", "b"));
///   let wrapped = combination_condition!(or(#my_cond, "c"));
///   ```
///
/// ## 2. `and` — 全部满足\All Must Pass
///
/// 所有子条件都返回 `true` 时，整体才为 `true`。至少需要两个参数。
/// The whole condition is `true` only when **all** sub-conditions return `true`.
/// Requires at least two arguments.
///
/// ```rust,ignore
/// combination_condition!(and("has_key", "door_unlocked"))
/// combination_condition!(and("a", "b", "c"))            // 三个及以上也支持
/// ```
///
/// ## 3. `or` — 任一满足\Any Passes
///
/// 任意一个子条件返回 `true` 时，整体就为 `true`。至少需要两个参数。
/// The whole condition is `true` when **any** sub-condition returns `true`.
/// Requires at least two arguments.
///
/// ```rust,ignore
/// combination_condition!(or("path_a", "path_b"))
/// ```
///
/// ## 4. `not` — 取反\Negation
///
/// 对单个子条件取反。恰好需要一个参数。
/// Inverts a single sub-condition. Requires exactly one argument.
///
/// ```rust,ignore
/// combination_condition!(not("is_locked"))
/// ```
///
/// ## 5. 嵌套组合\Nested Combinations
///
/// `and` / `or` / `not` 可以任意嵌套，构建复杂的布尔逻辑：
/// Can be nested arbitrarily to build complex boolean logic:
///
/// ```rust,ignore
/// combination_condition!(and(
///     "has_key",
///     or("door_open", not("is_locked"))
/// ))
/// ```
///
/// # EBNF 语法\EBNF Grammar
///
/// ```ebnf
/// combination_condition ::= guard_expression ;
///
/// guard_expression ::= ( 'and' | 'or' ), '(', guard_expression, { ',', guard_expression }, ')'
///                    | 'not', '(', guard_expression, ')'
///                    | guard_id ;
///
/// guard_id ::= lit_str
///            | '#', identifier ;
/// ```
///
/// # 示例\Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// fn is_a(_: In<GuardContext>) -> bool { true }
/// fn is_b(_: In<GuardContext>) -> bool { false }
///
/// fn setup(mut commands: Commands, mut guard_registry: ResMut<GuardRegistry>) {
///     system_registry!(<commands, guard_registry>[
///         "is_a" => is_a,
///         "is_b" => is_b,
///     ]);
///
///     let enter_condition = combination_condition!(and("is_a", not("is_b")));
///
///     commands.spawn(hsm!(
///         #[state(guard_enter = #enter_condition)]: Initial
///     ));
/// }
/// ```
#[proc_macro]
pub fn combination_condition(item: TokenStream) -> TokenStream {
    guard_condition::guard_condition_impl(item)
}

// ── hsm! ─────────────────────────────────────────────────────────────────

/// # 层级状态机\Hierarchical State Machine
///
/// 声明式构建一个完整的层级状态机 (HSM)。在一个 DSL 中定义状态树、生命周期动作、
/// 守卫条件、退出行为、场景数据，以及附加到状态机实体的 Bevy 组件。
///
/// Declaratively constructs a complete Hierarchical State Machine (HSM).
/// Defines the state tree, lifecycle actions, guard conditions, exit behavior,
/// state-scene data, and Bevy components on the SM entity — all in one DSL.
///
/// 宏展开后生成一个 [`SpawnStateMachine`] 组件，可通过 `commands.spawn(hsm!(...))`
/// 直接生成状态机实体。
///
/// The macro expands to a [`SpawnStateMachine`] component; use
/// `commands.spawn(hsm!(...))` to spawn the state machine entity.
///
/// # 逐步语法说明\Step-by-Step Syntax
///
/// ## 1. 机器配置 (可选)\Machine Config (optional)
///
/// 以 `init(...)` 开头，配置状态机的初始状态、当前状态和历史容量。
/// 注意：`init(...)` 之后必须有一个逗号 `,` 与后续内容分隔。
///
/// Starts with `init(...)` to configure the initial state, current state,
/// and history capacity. A comma `,` after `init(...)` is required.
///
/// ```rust,ignore
/// hsm!(
///     init(init_state = Root, curr_state = Child, history_capacity = 20),
///     // ... 状态树 / state tree ...
/// )
/// ```
///
/// | 参数\Parameter | 类型\Type | 说明\Description |
/// |---|---|------|
/// | `init_state = ...` | 状态名或索引\name or index | 启动时进入的状态 (默认: 第一个状态) |
/// | `curr_state = ...` | 状态名或索引\name or index | 当前激活状态 (默认: 同 init_state) |
/// | `history_capacity = ...` | 整数\integer | 历史记录容量 (需 `history` feature, 默认: 10) |
///
/// ## 2. 根状态节点\Root State Node
///
/// 每个 `hsm!` 必须有且仅有一个根状态。状态以 `#[state(...)]` 属性标记，
/// 可选地后跟 `:Name` 指定状态名称。
///
/// Every `hsm!` must have exactly one root state. States are marked with
/// `#[state(...)]`, optionally followed by `:Name` for the state name.
///
/// ```rust,ignore
/// hsm!(
///     #[state(after_enter = "log", on_update = "tick")]: Root
/// )
/// ```
///
/// 状态的子节点定义在紧跟的 `(...)` 中（见第 3 步）。
/// A state's children are defined in the `(...)` that follows (see step 3).
///
/// ## 3. 状态嵌套\State Nesting
///
/// 在状态名后的 `(...)` 内定义子状态和该状态专属的组件。子状态和组件可以
/// 任意混合、逗号分隔。
///
/// Define child states and state-local components inside `(...)` after the
/// state name. Children and components can be freely mixed, comma-separated.
///
/// ```rust,ignore
/// hsm!(
///     #[state]: Root (
///         #[state(guard_enter = "can_enter")]: ChildA,
///         #[state(strategy = Parallel, behavior = Rebirth)]: ChildB,
///         Name::new("child_state_label"),         // 附加到 ChildB 的组件
///     )
/// )
/// ```
///
/// 分组语法\Grouping syntax — 多状态或多组件可用括号分组：
/// Multiple states/components can be grouped in parentheses:
///
/// ```rust,ignore
/// hsm!(
///     #[state]: Root (
///         // 两个子状态作为一组\Two child states as a group:
///         (#[state]: A, #[state]: B),
///         // 一组组件附加到 Root\Group of components attached to Root:
///         (Name::new("Root"), StateLifecycle::default()),
///     )
/// )
/// ```
///
/// ## 4. 自由组件\Free Components
///
/// 与状态节点平级（不在任何状态的 `(...)` 内）的表达式会被视为附加到
/// **状态机实体** 上的 Bevy 组件。可以出现在状态树之前或之后。
///
/// Expressions at the top level (not inside any state's `(...)`) are treated
/// as Bevy components attached to the **state machine entity**. They can
/// appear before or after the state tree.
///
/// ```rust,ignore
/// hsm!(
///     Name::new("MyHSM"),              // 状态机实体上的组件
///     #[state]: Root (
///         #[state]: Child
///     ),
///     StateLifecycle::default(),       // 更多状态机组件
/// )
/// ```
///
/// ## 5. 配置回调 (可选)\Config Callback (optional)
///
/// 以 `:callback_fn` 结尾，该函数接收 `(&mut EntityWorldMut, &[Entity; N])` —
/// 状态机实体引用和所有状态实体的数组。用于在生成后进行额外配置。
///
/// Ends with `:callback_fn`, which receives `(&mut EntityWorldMut, &[Entity; N])`
/// — the SM entity reference and the array of all state entities. Use for
/// post-spawn customization.
///
/// ```rust,ignore
/// fn post_setup(entity_mut: &mut EntityWorldMut, ids: &[Entity]) {
///     // ids[0] = Root, ids[1] = Child, ...
/// }
///
/// hsm!(
///     #[state]: Root (#[state]: Child)
///     :post_setup
/// )
/// ```
///
/// # `#[state(...)]` 参数速查表\`#[state(...)]` Parameter Reference
///
/// ## 生命周期动作\Lifecycle Actions
///
/// | 参数\Parameter | 值类型\Value | 说明\Description |
/// |---|---|---|
/// | `before_enter` | [`ActionId`] | 进入状态**前**执行\Executes **before** entering |
/// | `after_enter` | [`ActionId`] | 进入状态**后**执行\Executes **after** entering |
/// | `on_update` | `LitStr` | 状态激活时每帧执行\Executes every frame while active |
/// | `before_exit` | [`ActionId`] | 退出状态**前**执行\Executes **before** exiting |
/// | `after_exit` | [`ActionId`] | 退出状态**后**执行\Executes **after** exiting |
///
/// [`ActionId`] 支持多种语法 (详见 [ActionId 语法](#actionid-语法actionid-syntax))：
///
/// ```rust,ignore
/// after_enter = "on_enter"            // 字符串引用\string reference
/// after_enter = on_enter              // 函数路径\function path
/// after_enter = tag: |ctx| { ... }    // 具名闭包\named closure
/// after_enter = tag: my_fn(a, b)      // 具名函数调用\named function call
/// ```
///
/// ## 守卫\Guards
///
/// | 参数\Parameter | 值类型\Value | 说明\Description |
/// |---|---|---|
/// | `guard_enter` | [`GuardCondition`] | 控制**进入**该状态的条件\Condition for **entering** this state |
/// | `guard_exit` | [`GuardCondition`] | 控制**退出**该状态的条件\Condition for **exiting** this state |
///
/// ## 层级行为\Hierarchical Behavior
///
/// | 参数\Parameter | 取值\Values | 说明\Description |
/// |---|---|---|
/// | `strategy` | `Nested` (默认) \| `Parallel` | 进入子状态时父状态的行为 |
/// | `behavior` | `Resurrection` (默认) \| `Rebirth` \| `Death` | 从子状态返回时父状态的行为 |
///
/// ## 其他\Other
///
/// | 参数\Parameter | 值类型\Value | 说明\Description |
/// |---|---|---|
/// | `state_scene` | `bsn!{...}` 或 `bsn_list![...]` | 进入时自动插入组件/子实体 (需 `state_data` feature) |
/// | `fsm_blueprint` | 表达式\expression | 在该 HSM 状态内嵌套 FSM (需 `hybrid` feature) |
/// | `minimal` | (无值\no value) | 最简状态：跳过子容器解析 |
///
/// # ActionId 语法\ActionId Syntax
///
/// ```ebnf
/// action_id ::= lit_str                                  (* "my_system" *)
///             | fn_identifier                            (* my_system *)
///             | action_name, ':', expr_closure           (* tag: |ctx| { ... } *)
///             | action_name, ':', expr_call              (* tag: my_fn(a, b) *)
///             | action_name, ':', fn_identifier          (* tag: my_system *)
/// ```
///
/// # EBNF 语法\EBNF Grammar
///
/// ```ebnf
/// (* ── 顶层结构\Top-level Structure ── *)
///
/// hsm ::= [ machine_config, ',' ], hsm_item, { ',', hsm_item }, [ ',' ] ;
///
/// hsm_item ::= state_node
///            | component
///            | config_fn ;
///
/// (* ── 机器配置\Machine Config ── *)
///
/// machine_config ::= 'init', '(', [ machine_config_param, { ',', machine_config_param } ], ')' ;
/// machine_config_param ::= ( 'init_state' | 'curr_state' ), '=', state_ref
///                        | 'history_capacity', '=', integer_literal ;   (* "history" feature *)
///
/// (* ── 状态节点\State Node ── *)
///
/// state_node ::= state_attribute, [ ':', identifier ], [ state_body ] ;
/// state_body ::= '(', { hsm_state_content, ',' }, [ hsm_state_content ], ')' ;
/// hsm_state_content ::= state_node
///                     | component
///                     | '(', { state_node, ',' }, [ state_node ], ')'
///                     | '(', { component, ',' }, [ component ], ')' ;
///
/// (* ── 状态属性\State Attribute ── *)
///
/// state_attribute ::= '#[state', [ '(', state_attr_param, { ',', state_attr_param }, ')' ], ']' ;
/// state_attr_param ::= ( 'before_enter' | 'after_enter' | 'before_exit' | 'after_exit' ), '=', action_id
///                    | 'on_update', '=', lit_str
///                    | ( 'guard_enter' | 'guard_exit' ), '=', guard_expression
///                    | 'strategy', '=', ( 'Nested' | 'Parallel' )
///                    | 'behavior', '=', ( 'Rebirth' | 'Resurrection' | 'Death' )
///                    | 'state_scene', '=', expr_bsn
///                    | 'fsm_blueprint', '=', rust_expression               (* "hybrid" feature *)
///                    | 'minimal' ;
///
/// expr_bsn ::= '{', bsn, '}'
///            | '[', bsn_list, ']' ;
///
/// (* ── 共享基元\Shared Primitives ── *)
///
/// config_fn ::= ':', ( fn_identifier | expr_closure | expr_call ) ;
/// state_ref ::= identifier | integer_literal ;
///
/// fn_identifier ::= (* Rust 函数路径\Rust path to a function, e.g., my_system *)
/// expr_closure  ::= (* Rust 闭包\Rust closure, e.g., |ctx| { ... } *)
/// expr_call     ::= (* Rust 函数调用\Rust function call, e.g., my_fn(a, b) *)
/// identifier    ::= (* Rust 标识符\Rust identifier, e.g., MyState *)
/// lit_str       ::= (* Rust 字符串字面量\Rust string literal, e.g., "my_system" *)
/// ```
///
/// # 示例\Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::{prelude::*, system_registry};
///
/// fn on_enter(ctx: In<ActionContext>) { info!("Entering {:?}", ctx.state()); }
/// fn on_exit(ctx: In<ActionContext>)  { info!("Exiting {:?}", ctx.state()); }
/// fn on_tick(ctx: In<ActionContext>)  { /* per-frame logic */ }
///
/// fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
///     system_registry!(<commands, action_registry>[
///         "on_enter" => on_enter,
///         "on_exit"  => on_exit,
///         "on_tick"  => on_tick,
///     ]);
///
///     commands.spawn(hsm!(
///         init(init_state = Root),
///         #[state(after_enter = "on_enter", on_update = "on_tick", behavior = Rebirth)]: Root (
///             #[state(guard_enter = "can_enter", after_enter = "on_enter")]: ChildA,
///             #[state(strategy = Parallel, before_exit = "on_exit")]: ChildB (
///                 #[state(minimal)]: Leaf
///             )
///         ),
///         Name::new("MyHSM"),
///         StateLifecycle::default(),
///     ));
/// }
/// ```
#[proc_macro]
#[cfg(feature = "hsm")]
pub fn hsm(item: TokenStream) -> TokenStream {
    hsm::hsm_impl(item)
}

// ── hsm_tree! ────────────────────────────────────────────────────────────

/// # 状态树组件\State Tree Component
///
/// 构建一个独立的 [`StateTree`] 组件。是 `hsm!` 的子集，仅接受一个根状态节点
/// 和可选的 `:config_fn`，不支持 `init(...)` 机器配置和自由组件。
///
/// Builds a standalone [`StateTree`] component. This is a subset of `hsm!` —
/// it only accepts a single root state node and an optional `:config_fn`,
/// without `init(...)` machine config or free components.
///
/// 适用于需要动态组合状态树或提前构建树的场景。
/// Useful for dynamically composing state trees or pre-building trees.
///
/// # 逐步语法说明\Step-by-Step Syntax
///
/// ## 1. 根状态节点\Root State Node
///
/// 以 `#[state(...)]` 开头，后跟可选的 `:Name` 和子节点 `(...)`。
/// 语法与 `hsm!` 中的状态节点完全一致。
///
/// Starts with `#[state(...)]`, optionally followed by `:Name` and child
/// nodes `(...)`. The syntax is identical to state nodes in `hsm!`.
///
/// ## 2. 配置回调 (可选)\Config Callback (optional)
///
/// 以 `:callback_fn` 结尾，接收 `(&mut EntityWorldMut, &[Entity; N])`。
///
/// # EBNF 语法\EBNF Grammar
///
/// ```ebnf
/// hsm_tree ::= state_node, [ ',', config_fn ] ;
///
/// (* state_node 和 config_fn 的定义与 hsm! 共享 *)
/// (* state_node and config_fn definitions are shared with hsm! *)
/// ```
///
/// # 示例\Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// fn setup(mut commands: Commands) {
///     let tree = hsm_tree!(
///         #[state(strategy = Parallel)]: Root (
///             #[state(strategy = Nested)]: ChildA,
///             #[state(strategy = Nested, behavior = Rebirth)]: ChildB,
///         ),
///         :|entity_mut, ids| {
///             info!("State tree spawned with {} states", ids.len());
///         }
///     );
///     commands.spawn(tree);
/// }
/// ```
#[proc_macro]
#[cfg(feature = "hsm")]
pub fn hsm_tree(item: TokenStream) -> TokenStream {
    hsm_tree::hsm_tree_impl(item)
}

// ── fsm_graph! ───────────────────────────────────────────────────────────

/// # FSM 状态图组件\FSM Graph Component
///
/// 构建一个独立的 [`FsmGraph`] 组件，定义 FSM 中的所有状态实体和它们之间
/// 的有效转换规则。是 `fsm!` 的子集。
///
/// Builds a standalone [`FsmGraph`] component that defines all state entities
/// and valid transition rules for an FSM. This is a subset of `fsm!`.
///
/// # 逐步语法说明\Step-by-Step Syntax
///
/// ## 1. 状态块\States Block
///
/// 以 `states: { ... }` 定义所有状态。每个状态以 `#[state(...)]` 开头，
/// 后可跟 `:Name` 和 `(附加组件)`。
///
/// Begins with `states: { ... }` to define all states. Each state starts with
/// `#[state(...)]`, optionally followed by `:Name` and `(attached components)`.
///
/// ```rust,ignore
/// fsm_graph!(
///     states: {
///         #[state(after_enter = "log")]: Idle,
///         #[state(after_enter = "log")]: Running,
///         #[state(minimal)]: Stopped,
///         // 匿名状态，附带组件\Anonymous state with components:
///         #[state(before_exit = "cleanup")] (ComponentA, ComponentB),
///     },
///     transitions: { /* ... */ }
/// )
/// ```
///
/// FSM 状态**不支持** HSM 专属参数（`strategy`、`behavior`、`guard_enter`、
/// `guard_exit`、`fsm_blueprint`）。使用这些参数会产生编译错误。
///
/// FSM states do **not** support HSM-only parameters (`strategy`, `behavior`,
/// `guard_enter`, `guard_exit`, `fsm_blueprint`). Using them emits a compile error.
///
/// ## 2. 转换块\Transitions Block
///
/// 以 `transitions: { ... }` 定义状态间的转换规则。每条规则格式为：
/// `from 箭头 to`，可选地附加转换条件。
///
/// Begins with `transitions: { ... }` to define transition rules. Each rule
/// is `from ARROW to`, optionally with a condition.
///
/// ### 2a. 方向箭头\Direction Arrows
///
/// | 箭头\Arrow | 方向\Direction | 含义\Meaning |
/// |---|---|---|
/// | `=>` | 单向右\Right | from → to |
/// | `<=` | 单向左\Left | to → from (反向) |
/// | `<=>` | 双向\Both | from ↔ to |
///
/// ### 2b. 转换条件\Transition Conditions
///
/// 条件放在 `: condition(...)` 中，紧跟目标状态。支持三种：
/// Conditions follow `:` after the target state. Three kinds are supported:
///
/// * **无条件\Unconditional** — 直接跳转，无需任何条件：
///   ```rust,ignore
///   A => B
///   ```
///
/// * **事件\Event** — 收到匹配事件时触发：
///   ```rust,ignore
///   A => B : event(MyEvent)
///   A => B : event(MyEvent::Variant)
///   ```
///
/// * **守卫\Guard** — 守卫条件通过时允许转换：
///   ```rust,ignore
///   A => B : guard("my_guard")
///   A => B : guard(and("a", not("b")))
///   ```
///
/// ## 3. 状态引用\State References
///
/// 在转换规则中，状态可以通过**名称**或**索引**（从 0 开始）引用：
/// In transitions, states can be referenced by **name** or **index** (0-based):
///
/// ```rust,ignore
/// transitions: {
///     Idle => Running,          // 按名称\by name
///     0 => 1 : event(Go),      // 按索引\by index
///     Running <=> Idle,         // 双向\bidirectional
/// }
/// ```
///
/// # `#[state(...)]` 可用参数\Available `#[state(...)]` Parameters for FSM
///
/// FSM 状态仅支持以下 `#[state(...)]` 参数：
/// Only the following `#[state(...)]` parameters are valid for FSM states:
///
/// | 参数\Parameter | 说明\Description |
/// |---|---|
/// | `before_enter` | 进入前执行 |
/// | `after_enter` | 进入后执行 |
/// | `on_update` | 每帧执行 |
/// | `before_exit` | 退出前执行 |
/// | `after_exit` | 退出后执行 |
/// | `state_scene` | 进入时插入组件 (需 `state_data` feature) |
/// | `minimal` | 最简状态 |
///
/// # EBNF 语法\EBNF Grammar
///
/// ```ebnf
/// fsm_graph ::= fsm_graph_content, [ ',', config_fn ] ;
///
/// fsm_graph_content ::= 'states', ':', '{', [ fsm_state, { ',', fsm_state } ], '}',
///                       [ ',' ],
///                       'transitions', ':', '{', [ transition, { ',', transition } ], '}' ;
///
/// fsm_state ::= state_attribute, [ ':', identifier ], [ '(', { component, ',' }, [ component ], ')' ] ;
///
/// transition ::= state_ref, ( '<=>' | '=>' | '<=' ), state_ref, [ ':', transition_condition ] ;
/// transition_condition ::= 'event', '(', rust_expression, ')'
///                        | 'guard', '(', guard_expression, ')' ;
///
/// (* state_attribute, state_ref, config_fn 与 hsm! 共享 *)
/// (* state_attribute, state_ref, config_fn are shared with hsm! *)
/// ```
///
/// # 示例\Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// #[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
/// struct Go;
///
/// fn setup(mut commands: Commands) {
///     let graph = fsm_graph!(
///         states: {
///             #[state(after_enter = "log_enter")]: Idle,
///             #[state(after_enter = "log_enter")]: Running,
///             #[state(minimal)]: Paused,
///         },
///         transitions: {
///             Idle => Running : event(Go),
///             Running <=> Paused : event(Go),
///             0 <=> 2 : guard("can_toggle"),
///         },
///         :|entity_mut, ids| {
///             info!("Graph spawned with {} states", ids.len());
///         }
///     );
///     commands.spawn(graph);
/// }
/// ```
#[proc_macro]
#[cfg(feature = "fsm")]
pub fn fsm_graph(item: TokenStream) -> TokenStream {
    fsm_graph::fsm_graph_impl(item)
}

// ── fsm! ─────────────────────────────────────────────────────────────────

/// # 有限状态机\Finite State Machine
///
/// 声明式构建一个完整的有限状态机 (FSM)。在 `fsm_graph!` 的基础上增加了
/// `init(...)` 机器配置、`components: { ... }` 自由组件块和 `:config_fn` 回调。
///
/// Declaratively constructs a complete Finite State Machine (FSM). Extends
/// `fsm_graph!` with `init(...)` machine config, a `components: { ... }`
/// free-component block, and a `:config_fn` callback.
///
/// 宏展开后生成一个 [`SpawnStateMachine`] 组件，可通过 `commands.spawn(fsm!(...))`
/// 直接生成状态机实体。
///
/// The macro expands to a [`SpawnStateMachine`] component; use
/// `commands.spawn(fsm!(...))` to spawn the state machine entity.
///
/// # 逐步语法说明\Step-by-Step Syntax
///
/// ## 1. 机器配置 (可选)\Machine Config (optional)
///
/// 以 `init(...)` 开头，参数与 `hsm!` 相同。之后需要逗号 `,`。
///
/// Starts with `init(...)`, same parameters as `hsm!`. A trailing comma `,`
/// is required.
///
/// ```rust,ignore
/// fsm!(
///     init(init_state = Idle, history_capacity = 5),
///     // ...
/// )
/// ```
///
/// ## 2. 状态图\State Graph
///
/// 必须包含 `states: { ... }` 和 `transitions: { ... }` 两个块，
/// 语法与 [`fsm_graph!`] 完全一致。详见该宏的文档。
///
/// Must contain both `states: { ... }` and `transitions: { ... }` blocks.
/// The syntax is identical to [`fsm_graph!`]. See that macro for details.
///
/// ## 3. 自由组件块 (可选)\Free Components Block (optional)
///
/// `components: { ... }` 内的表达式将作为 Bevy 组件附加到**状态机实体**上。
/// 注意：与 `hsm!` 不同，`fsm!` 中的自由组件必须封装在此块中。
///
/// Expressions inside `components: { ... }` are attached as Bevy components
/// to the **state machine entity**. Unlike `hsm!`, free components in `fsm!`
/// must be wrapped in this block.
///
/// ```rust,ignore
/// fsm!(
///     states: { /* ... */ },
///     transitions: { /* ... */ },
///     components: {
///         Name::new("MyFSM"),
///         Paused,
///     }
/// )
/// ```
///
/// ## 4. 配置回调 (可选)\Config Callback (optional)
///
/// 以 `:callback_fn` 结尾，与 `hsm!` 相同。
///
/// # EBNF 语法\EBNF Grammar
///
/// ```ebnf
/// fsm ::= [ machine_config, ',' ], fsm_graph_content, [ ',', components_block ], [ ',', config_fn ] ;
///
/// components_block ::= 'components', ':', '{', [ component, { ',', component } ], '}' ;
///
/// (* machine_config, config_fn, component 与 hsm! 共享 *)
/// (* fsm_graph_content 见 fsm_graph! *)
/// ```
///
/// # 示例\Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// #[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
/// enum MyEvent { Go, Back }
///
/// fn log_enter(ctx: In<ActionContext>) { info!("Entering {:?}", ctx.state()); }
///
/// fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
///     system_registry!(<commands, action_registry>[
///         "log_enter" => log_enter,
///     ]);
///
///     commands.spawn(fsm!(
///         init(init_state = A),
///         states: {
///             #[state(after_enter = "log_enter")]: A,
///             #[state(after_enter = "log_enter")]: B,
///             #[state(minimal)]: C,
///         },
///         transitions: {
///             A <=> B : event(MyEvent::Go),
///             B => C : event(MyEvent::Back),
///             2 => 0 : event(MyEvent::Go),        // 按索引\by index
///         },
///         components: {
///             Name::new("MyFSM"),
///         },
///         :|entity_mut, ids| {
///             info!("FSM spawned with {} states", ids.len());
///         }
///     ));
/// }
/// ```
#[proc_macro]
#[cfg(feature = "fsm")]
pub fn fsm(item: TokenStream) -> TokenStream {
    fsm::fsm_impl(item)
}
