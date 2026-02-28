use bevy::ecs::component::Component;

pub mod event;
pub mod graph;
#[cfg(feature = "history")]
pub mod history;
pub mod state_machine;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FsmState;
