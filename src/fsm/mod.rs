use bevy::ecs::component::Component;

pub mod event;
pub mod graph;
#[cfg(feature = "history")]
pub mod history;
pub mod state_machine;

/// # FSM 状态
/// * 一个标记组件，用于将一个实体标识为有限状态机（FSM）中的一个状态。
///
/// 这个组件本身不包含任何数据，它仅用作识别和查询状态实体的标签。
/// 状态实体应该被添加到 [`FsmGraph`] 中来定义状态机的结构。
///
/// # FSM State
/// * A marker component used to identify an entity as a state within a Finite State Machine (FSM).
///
/// This component itself contains no data; it serves only as a tag for identifying and querying
/// state entities. State entities should be added to an [`FsmGraph`] to define the state
/// machine's structure.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FsmState;
