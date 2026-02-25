use bevy::ecs::component::Component;

pub mod event;
pub mod graph;
pub mod history;
pub mod state_machine;

#[derive(Component, Debug, PartialEq, Eq, Hash)]
pub struct FsmState;
