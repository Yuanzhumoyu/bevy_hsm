//! # Bevy HSM: 一个为 Bevy 设计的强大且灵活的状态机库
//!
//! `bevy_hsm` 为 [Bevy 引擎](https://bevyengine.org/) 提供了一套完整且功能丰富的状态机解决方案。
//! 它不仅支持传统的有限状态机 (FSM)，还实现了强大的分层状态机 (HSM)，让您能够以声明式、可组合的方式构建复杂的游戏逻辑和 AI 行为。
//!
//! ## 核心理念
//!
//! - **一切皆为系统**: 状态的生命周期（进入、更新、退出）和转换守卫（Guard）都被实现为标准的 Bevy 系统。这使得状态机可以无缝地与 Bevy 的 ECS 世界交互，访问资源、查询实体、发送事件。
//! - **事件驱动**: 状态转换由事件（Triggers）驱动，完全兼容 Bevy 的事件系统。
//! - **分层结构 (HSM)**: 通过父子状态关系，您可以构建出清晰、可维护的复杂行为树，避免“状态爆炸”问题。
//! - **声明式宏**: 提供易于使用的过程宏，让您可以直观地定义状态机的结构和转换关系。
//!
//! ## 主要功能
//!
//! - **双模式支持**: 同时支持有限状态机 (`FSM`) 和分层状态机 (`HSM`)。
//! - **生命周期动作**: 为每个状态定义 `BeforeEnter`, `AfterEnter`, `OnUpdate`, `BeforeExit`, `AfterExit` 动作，它们都是 Bevy 系统。
//! - **转换守卫 (Guards)**: 使用 Bevy 系统作为守卫条件，以编程方式决定是否允许状态转换。
//! - **状态数据 (`StateData`)**: 自动在进入/退出状态时为实体添加/移除指定的组件。
//! - **历史状态 (History)**: HSM 支持历史状态，可以轻松返回到之前的活动子状态。
//! - **灵活的转换策略**: HSM 支持多种进入/退出策略（例如 `Nested`, `Parallel`），以控制父子状态的行为。
//! - **强大的宏支持**: 使用 `fsm!`, `hsm!`, `fsm_graph!`, `hsm_tree!` 等宏来简化状态机定义。
//! - **高度可定制**: 可以轻松配置状态机系统在 Bevy 的哪个调度阶段 (Schedule) 中运行。
//!
//! -------------------------------------------------------
//!
//! # Bevy HSM: A Powerful and Flexible State Machine Library for Bevy
//!
//! `bevy_hsm` provides a complete and feature-rich state machine solution for the [Bevy Engine](https://bevyengine.org/).
//! It supports not only traditional Finite State Machines (FSM) but also implements powerful Hierarchical State Machines (HSM),
//! allowing you to build complex game logic and AI behaviors in a declarative and composable way.
//!
//! ## Core Concepts
//!
//! - **Everything is a System**: State lifecycles (Enter, Update, Exit) and transition Guards are implemented as standard Bevy systems. This allows the state machine to seamlessly interact with Bevy's ECS world, accessing resources, querying entities, and sending events.
//! - **Event-Driven**: State transitions are driven by events (Triggers), fully compatible with Bevy's event system.
//! - **Hierarchical Structure (HSM)**: Through parent-child state relationships, you can build clear, maintainable, and complex behavior trees, avoiding the "state explosion" problem.
//! - **Declarative Macros**: Provides easy-to-use procedural macros that let you intuitively define the structure and transitions of your state machines.
//!
//! ## Features
//!
//! - **Dual-Mode Support**: Supports both Finite State Machines (`FSM`) and Hierarchical State Machines (`HSM`).
//! - **Lifecycle Actions**: Define `BeforeEnter`, `AfterEnter`, `OnUpdate`, `BeforeExit`, and `AfterExit` actions for each state, all of which are Bevy systems.
//! - **Transition Guards**: Use Bevy systems as guard conditions to programmatically decide whether a state transition is allowed.
//! - **State Data**: Automatically add or remove specified components from an entity upon entering/exiting a state.
//! - **History States**: HSMs support history states, making it easy to return to a previous active sub-state.
//! - **Flexible Transition Strategies**: HSM supports various entry/exit strategies (e.g., `Nested`, `Parallel`) to control the behavior of parent and child states.
//! - **Powerful Macro Support**: Use macros like `fsm!`, `hsm!`, `fsm_graph!`, and `hsm_tree!` to simplify state machine definitions.
//! - **Highly Customizable**: Easily configure which schedule the state machine systems run in.
//!
pub mod action_dispatcher;
pub mod context;
mod error;
#[cfg(feature = "fsm")]
pub mod fsm;
pub mod guards;
#[cfg(feature = "hsm")]
pub mod hsm;
pub mod labels;
pub mod markers;
pub mod state_actions;
#[cfg(feature = "state_data")]
pub mod state_data;

#[cfg(feature = "hsm")]
use std::sync::Arc;

#[cfg(feature = "hsm")]
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use crate::action_dispatcher::ActionDispatch;
use crate::guards::GuardRegistry;
use crate::prelude::TransitionRegistry;
use crate::state_actions::ActionRegistry;

/// Bevy 插件，用于初始化状态机所需的所有资源和系统。
///
/// A Bevy plugin that initializes all the resources and systems required for the state machine.
///
/// ## Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(StateMachinePlugin::default())
///     .run();
/// ```
pub struct StateMachinePlugin {
    #[cfg(feature = "hsm")]
    transition_system: Arc<dyn for<'a> Fn(&'a mut App) + Send + Sync>,
}

#[cfg(feature = "hsm")]
impl StateMachinePlugin {
    /// 创建一个新的 `StateMachinePlugin`，并指定 HSM 的转换系统在哪个调度阶段运行。
    /// 默认情况下，系统在 `Last` 调度中运行。
    ///
    /// Creates a new `StateMachinePlugin` and specifies in which schedule the HSM's transition systems should run.
    /// By default, the systems run in the `Last` schedule.
    pub fn with_schedule<T: ScheduleLabel + Clone>(schedule: T) -> Self {
        let f = move |app: &mut App| {
            crate::hsm::transition_strategy::install_transition_systems(app, schedule.clone());
        };
        StateMachinePlugin {
            transition_system: Arc::new(f),
        }
    }
}

impl Plugin for StateMachinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionDispatch>();
        app.init_resource::<ActionRegistry>();
        app.init_resource::<GuardRegistry>();
        app.init_resource::<TransitionRegistry>();

        #[cfg(feature = "hsm")]
        {
            use crate::hsm::{
                guards::{GuardEnterCache, GuardExitCache},
                transition_strategy::CheckOnTransitionStates,
            };

            app.init_resource::<CheckOnTransitionStates>();
            app.init_resource::<GuardEnterCache>();
            app.init_resource::<GuardExitCache>();
            app.init_resource::<prelude::ActionSystemRegistry>();

            (self.transition_system)(app);

            app.add_observer(hsm::state_machine::HsmStateMachine::handle_hsm_trigger);
        }

        #[cfg(feature = "fsm")]
        app.add_observer(fsm::state_machine::FsmStateMachine::handle_fsm_trigger);
    }
}

impl Default for StateMachinePlugin {
    fn default() -> Self {
        Self {
            #[cfg(feature = "hsm")]
            transition_system: Arc::new(|app: &mut App| {
                crate::hsm::transition_strategy::install_transition_systems(app, Last);
            }),
        }
    }
}

/// A macro to simplify the registration of multiple systems into a registry.
///
/// This macro is a convenient way to register multiple Bevy systems (like actions or guards)
/// into their respective registries (`ActionRegistry`, `GuardRegistry`) at once.
/// It takes a source for system registration (usually `Commands`),
/// the registry resource, and a list of name-system pairs.
///
/// # Arguments
///
/// * `$source`: The identifier for the system registration source, typically `commands` of type `Commands`.
/// * `$system_registry`: The identifier for the registry resource, e.g., `action_registry` of type `ResMut<ActionRegistry>`.
/// * `[$($system_name:expr => $system:expr),*]`: A comma-separated list of pairs, where:
///     * `$system_name`: A string literal representing the name to associate with the system.
///     * `$system`: The system (e.g., a function identifier) to be registered.
///
/// # Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// fn my_action(context: In<ActionContext>) { /* ... */ }
/// fn another_action(context: In<ActionContext>) { /* ... */ }
///
/// fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
///     system_registry!(<commands, action_registry>[
///         "action1" => my_action,
///         "action2" => another_action,
///     ]);
/// }
/// ```
#[macro_export]
macro_rules! system_registry {
    (<$source:ident, $system_registry:ident>[$($system_name:expr => $system:expr),*$(,)?]) => {
        $system_registry.extend([$(($system_name, $source.register_system($system))),*].into_iter());
    };
}

pub mod prelude {
    pub use crate::{
        StateMachinePlugin, action_dispatcher::*, context::*, guards::*, markers::*,
        state_actions::*,
    };

    #[cfg(feature = "state_data")]
    pub use crate::state_data::*;

    #[cfg(feature = "hsm")]
    pub use crate::hsm::{
        HsmState, event::*, guards::*, state_lifecycle::*, state_machine::*, state_tree::*,
        transition_strategy::*,
    };

    #[cfg(feature = "hsm")]
    pub use bevy_hsm_macros::{hsm, hsm_tree};

    #[cfg(feature = "fsm")]
    pub use crate::fsm::{FsmState, event::*, graph::*, state_machine::*};

    #[cfg(feature = "fsm")]
    pub use bevy_hsm_macros::{fsm, fsm_graph};

    pub use bevy_hsm_macros::combination_condition;
}
