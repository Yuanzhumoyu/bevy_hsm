use std::{collections::VecDeque, fmt::Debug};

use bevy::prelude::*;

use crate::{
    context::TransitionRelationship,
    hsm::{state_lifecycle::StateLifecycle, strategy::ExitTransitionBehavior},
};

/// # 状态转换\State Transition
/// * 状态转换的枚举，包含下一个状态的ID和OnState
/// - The enum of state transitions, including the ID of the next state and OnState
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Enter(Entity),
    Update(Entity),
    Exit(Entity),
    Start,
    End,
}

impl Transition {
    pub const fn to(self) -> Option<(Entity, StateLifecycle)> {
        match self {
            Transition::Enter(id) => Some((id, StateLifecycle::Enter)),
            Transition::Update(id) => Some((id, StateLifecycle::Update)),
            Transition::Exit(id) => Some((id, StateLifecycle::Exit)),
            Transition::Start | Transition::End => None,
        }
    }

    pub fn to_transition(self, next: Self) -> Option<TransitionRelationship> {
        use Transition::*;
        match (self, next) {
            (Start, Enter(to)) | (Start, Update(to)) | (Start, Exit(to)) => {
                Some(TransitionRelationship::Final(to))
            }

            (Enter(from), End) | (Update(from), End) | (Exit(from), End) => {
                Some(TransitionRelationship::Initial(from))
            }

            (Enter(from), Enter(to))
            | (Enter(from), Update(to))
            | (Enter(from), Exit(to))
            | (Update(from), Enter(to))
            | (Update(from), Update(to))
            | (Update(from), Exit(to))
            | (Exit(from), Enter(to))
            | (Exit(from), Update(to))
            | (Exit(from), Exit(to)) => Some(TransitionRelationship::Transition(from, to)),

            _ => {
                error!("Invalid state transition pair: {:?} -> {:?}", self, next);
                None
            }
        }
    }

    /// 根据 [`ExitTransitionBehavior`] 创建对应的转换。
    /// - `Rebirth` → `Enter`, `Resurrection` → `Update`, `Death` → `Exit`
    ///
    /// Creates a transition corresponding to the given [`ExitTransitionBehavior`].
    pub const fn with_behavior(state_id: Entity, behavior: ExitTransitionBehavior) -> Self {
        match behavior {
            ExitTransitionBehavior::Rebirth => Self::Enter(state_id),
            ExitTransitionBehavior::Resurrection => Self::Update(state_id),
            ExitTransitionBehavior::Death => Self::Exit(state_id),
        }
    }

    /// 根据 [`StateLifecycle`] 创建对应的转换。
    ///
    /// Creates a transition corresponding to the given [`StateLifecycle`].
    pub const fn with_lifecycle(state_id: Entity, lifecycle: StateLifecycle) -> Self {
        match lifecycle {
            StateLifecycle::Enter => Self::Enter(state_id),
            StateLifecycle::Update => Self::Update(state_id),
            StateLifecycle::Exit => Self::Exit(state_id),
        }
    }

    /// 获取转换关联的状态实体 ID。
    ///
    /// Returns the state entity ID associated with this transition.
    pub const fn get_state_id(&self) -> Option<Entity> {
        match self {
            Self::Enter(id) | Self::Update(id) | Self::Exit(id) => Some(*id),
            Self::Start | Self::End => None,
        }
    }

    /// 获取转换对应的生命周期阶段。
    ///
    /// Returns the lifecycle phase corresponding to this transition.
    pub const fn get_lifecyle(&self) -> Option<StateLifecycle> {
        match self {
            Transition::Enter(_) => Some(StateLifecycle::Enter),
            Transition::Update(_) => Some(StateLifecycle::Update),
            Transition::Exit(_) => Some(StateLifecycle::Exit),
            Transition::Start | Transition::End => None,
        }
    }
}

impl Debug for Transition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enter(id) => write!(f, "Enter({})", id),
            Self::Update(id) => write!(f, "Update({})", id),
            Self::Exit(id) => write!(f, "Exit({})", id),
            Self::Start => write!(f, "Start"),
            Self::End => write!(f, "End"),
        }
    }
}

/// # 转换队列\Transition Queue
/// * 保存状态转换序列的内部队列，同时追踪前一个转换以建立转换关系。
/// - An internal queue that holds a sequence of state transitions while tracking
///   the previous transition to establish transition relationships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitionQueue {
    prev_transition: Transition,
    next_transitions: VecDeque<Transition>,
}

impl TransitionQueue {
    /// 向队列尾部添加一个转换。
    ///
    /// Pushes a transition to the back of the queue.
    pub fn push(&mut self, transition: Transition) {
        self.next_transitions.push_back(transition);
    }

    /// 从队列头部弹出一个转换，队列为空时返回 [`Transition::End`]。
    ///
    /// Pops a transition from the front of the queue, returns [`Transition::End`] when empty.
    pub fn pop(&mut self) -> Transition {
        self.next_transitions.pop_front().unwrap_or(Transition::End)
    }

    /// 查看队列头部的下一个转换而不弹出它。
    ///
    /// Peeks at the next transition at the front of the queue without popping.
    pub fn next(&self) -> Transition {
        self.next_transitions
            .front()
            .copied()
            .unwrap_or(Transition::End)
    }

    /// 替换前一个转换并返回旧值。用于在进入新状态时更新 prev，以便计算转换关系。
    ///
    /// Replaces the previous transition and returns the old value.
    pub fn replace_prev(&mut self, transition: Transition) -> Transition {
        std::mem::replace(&mut self.prev_transition, transition)
    }

    /// 清空转换队列。
    ///
    /// Clears the transition queue.
    pub fn clear(&mut self) {
        self.next_transitions.clear();
    }

    /// 返回队列中待处理的转换数量。
    ///
    /// Returns the number of pending transitions in the queue.
    pub fn len(&self) -> usize {
        self.next_transitions.len()
    }

    /// 检查队列是否为空。
    ///
    /// Checks whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.next_transitions.is_empty()
    }
}

impl Default for TransitionQueue {
    fn default() -> Self {
        Self {
            prev_transition: Transition::Start,
            next_transitions: VecDeque::new(),
        }
    }
}

impl Extend<Transition> for TransitionQueue {
    fn extend<T: IntoIterator<Item = Transition>>(&mut self, iter: T) {
        self.next_transitions.extend(iter);
    }
}
