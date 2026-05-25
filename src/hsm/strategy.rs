use std::{any::type_name, fmt::Debug, sync::Arc};

use bevy::prelude::*;

use crate::{
    error::StateMachineError,
    hsm::{HsmState, state_lifecycle::StateLifecycle, state_tree::StateTree},
};

/// 状态转换策略，用于控制状态转换行为
///
/// State transition strategy, used to control state transition behavior
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateTransitionStrategy {
    /// 子状态嵌套转换：父状态保持激活，子状态进入和退出发生在父状态内部
    ///
    /// Sub state nested transition: The parent state remains active, and the sub state enters and exits occur within the parent state
    #[default]
    Nested,
    /// 平级转换：父状态先退出，然后子状态进入和退出，最后可能重新进入父状态
    ///
    /// Level-to-level transition: The parent state exits first, followed by the entry and exit of the child state, and finally, the parent state may be re-entered
    Parallel,
}

impl StateTransitionStrategy {
    pub fn is_nested(&self) -> bool {
        matches!(self, Self::Nested)
    }

    pub fn is_parallel(&self) -> bool {
        matches!(self, Self::Parallel)
    }
}

/// # 退出过渡状态行为\Exit Transition Behavior
///
/// * 用于定义状态在退出时的行为，包括重生、复活和死亡
/// - Used to define the behavior of a state when exiting, including rebirth, resurrection, and death
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitTransitionBehavior {
    /// # 重生\Rebirth
    ///
    /// 从sub_state退出后，重新进入super_state的enter阶段
    ///
    /// From sub_state exit, re-enter the super_state's after_enter phase
    Rebirth,
    /// # 复活\Resurrection
    ///
    /// 从sub_state退出后，进入super_state的update阶段
    ///
    /// From sub_state exit, enter the super_state's on_update phase
    #[default]
    Resurrection,
    /// # 死亡\Death
    ///
    /// 从sub_state退出后，不再进入super_state, 而是向上层状态继续判断[ExitTransitionBehavior]和[StateTransitionStrategy]
    ///
    /// From sub_state exit, do not enter super_state, but continue to judge [ExitTransitionBehavior] and [StateTransitionStrategy] to the upper state
    Death,
}

impl From<ExitTransitionBehavior> for StateLifecycle {
    fn from(value: ExitTransitionBehavior) -> Self {
        match value {
            ExitTransitionBehavior::Rebirth => StateLifecycle::Enter,
            ExitTransitionBehavior::Resurrection => StateLifecycle::Update,
            ExitTransitionBehavior::Death => StateLifecycle::Exit,
        }
    }
}

impl From<StateLifecycle> for ExitTransitionBehavior {
    fn from(value: StateLifecycle) -> Self {
        match value {
            StateLifecycle::Enter => ExitTransitionBehavior::Rebirth,
            StateLifecycle::Update => ExitTransitionBehavior::Resurrection,
            StateLifecycle::Exit => ExitTransitionBehavior::Death,
        }
    }
}

/// 一个用于定义子状态应如何遍历的 trait。
///
/// 此 trait 的实现将决定子状态在激活或其他操作中被考虑的顺序。
pub trait StateTraversalStrategy: Send + Sync + 'static {
    /// 给定一个子状态实体列表，按照期望的遍历顺序返回它们（取得所有权）。
    fn traverse(&self, world: &World, children: Vec<Entity>) -> Vec<Entity>;

    /// 返回遍历策略的名称。
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }
}

/// 一个包装结构体，用于持有动态的 `StateTraversalStrategy`。
///
/// 这允许在运行时互换使用不同的遍历策略。
pub struct TraversalStrategy(pub(crate) Arc<dyn StateTraversalStrategy>);

impl TraversalStrategy {
    /// 使用给定的实现创建一个新的 `TraversalStrategy`。
    pub fn new<T: StateTraversalStrategy>(strategy: T) -> Self {
        Self(Arc::new(strategy))
    }
}

impl Eq for TraversalStrategy {}

impl PartialEq for TraversalStrategy {
    fn eq(&self, other: &Self) -> bool {
        self.0.name() == other.0.name()
    }
}

impl Clone for TraversalStrategy {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl Default for TraversalStrategy {
    fn default() -> Self {
        static SEQUENTIAL: std::sync::LazyLock<TraversalStrategy> =
            std::sync::LazyLock::new(|| TraversalStrategy(Arc::new(SequentialTraversal)));
        SEQUENTIAL.clone()
    }
}

impl Debug for TraversalStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name())
    }
}

/// 一个基本的顺序遍历策略。
///
/// 此策略是零开销的透传——直接按提供的顺序返回子状态。
pub struct SequentialTraversal;

impl StateTraversalStrategy for SequentialTraversal {
    fn traverse(&self, _world: &World, children: Vec<Entity>) -> Vec<Entity> {
        children
    }
}

/// 一个基本的逆序遍历策略
///
/// 此策略简单地按照提供的逆序返回子状态。
pub struct ReverseTraversal;

impl StateTraversalStrategy for ReverseTraversal {
    fn traverse(&self, _world: &World, children: Vec<Entity>) -> Vec<Entity> {
        children.into_iter().rev().collect()
    }
}

pub(crate) fn get_state_tree(
    world: &World,
    state_tree_id: Entity,
) -> Result<&StateTree, StateMachineError> {
    world
        .get::<StateTree>(state_tree_id)
        .ok_or(StateMachineError::StateTreeNotFound(state_tree_id))
}

pub(crate) fn get_hsm_state(world: &World, state: Entity) -> Result<HsmState, StateMachineError> {
    world
        .get::<HsmState>(state)
        .copied()
        .ok_or(StateMachineError::HsmStateMissing(state))
}
