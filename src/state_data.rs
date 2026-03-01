use bevy::{
    ecs::{
        component::ComponentId,
        entity::{EntityClonerBuilder, OptIn},
        world::DeferredWorld,
    },
    prelude::*,
};

#[derive(Component, Default, Debug, Clone, PartialEq, Eq, Hash, Deref)]
pub struct StateData(Vec<ComponentId>);

impl StateData {
    pub fn new(mut component_ids: Vec<ComponentId>) -> Self {
        component_ids.sort();
        Self(component_ids)
    }

    pub fn push(&mut self, component_id: ComponentId) {
        if let Err(index) = self.0.binary_search(&component_id) {
            self.0.insert(index, component_id);
        }
    }

    pub fn remove(&mut self, component_id: ComponentId) -> bool {
        if let Ok(index) = self.0.binary_search(&component_id) {
            self.0.remove(index);
            return true;
        }
        false
    }

    /// 克隆状态数据
    #[inline]
    pub(crate) fn clone_components(
        world: &mut DeferredWorld,
        entity: Entity,
        service_target: Entity,
    ) {
        let (entitys, mut commands) = world.entities_and_commands();
        let Ok(curr_state_ref) = entitys.get(entity) else {
            return;
        };
        let Some(state_data) = curr_state_ref.get::<StateData>().cloned() else {
            return;
        };

        commands.queue(state_data.clone_state_data_command(entity, service_target));
    }

    /// 移除状态数据
    #[inline]
    pub(crate) fn remove_components(
        world: &mut DeferredWorld,
        entity: Entity,
        service_target: Entity,
    ) {
        let (entitys, mut commands) = world.entities_and_commands();
        let Ok(curr_state_ref) = entitys.get(entity) else {
            return;
        };
        let Some(state_data) = curr_state_ref.get::<StateData>().cloned() else {
            return;
        };
        commands.queue(state_data.remove_state_data_command(service_target));
    }

    pub(crate) fn clone_state_data_command(
        self,
        entity: Entity,
        service_target: Entity,
    ) -> impl Command {
        move |world: &mut World| {
            world.entity_mut(entity).clone_with_opt_in(
                service_target,
                move |builder: &mut EntityClonerBuilder<'_, OptIn>| {
                    builder.allow_by_ids(self.as_slice());
                },
            );
        }
    }

    pub(crate) fn remove_state_data_command(self, entity: Entity) -> impl Command {
        move |world: &mut World| {
            let mut entity_mut = world.entity_mut(entity);
            entity_mut.remove_by_ids(&self);
        }
    }
}

#[cfg(all(test, any(feature = "hsm", feature = "fsm")))]
mod tests {
    use crate::StateMachinePlugin;

    use super::*;

    #[derive(Component, Debug, PartialEq, Eq, Clone)]
    struct ComponentA;

    #[derive(Component, Debug, PartialEq, Eq, Clone)]
    struct ComponentB;

    #[derive(Component, Debug, PartialEq, Eq, Clone)]
    struct ComponentC;

    #[test]
    #[cfg(feature = "hsm")]
    fn test_hsm_state_machine_state_data() {
        use crate::hsm::{HsmState, event::*, state_machine::*, state_tree::*};

        let mut app = App::new();
        app.add_plugins(StateMachinePlugin::<Last>::default());

        let world = app.world_mut();

        let state_data = StateData::new(vec![
            world.register_component::<ComponentA>(),
            world.register_component::<ComponentB>(),
        ]);

        let id1 = world
            .spawn((
                HsmState::with(
                    crate::prelude::StateTransitionStrategy::Parallel,
                    crate::prelude::ExitTransitionBehavior::Rebirth,
                ),
                Name::new("StateA"),
                state_data,
                ComponentA,
                ComponentB,
                ComponentC,
            ))
            .id();

        let id2 = world.spawn((HsmState::default(), Name::new("StateB"))).id();

        let mut state_tree = StateTree::new(id1);
        state_tree.add(id1, id2);

        let state_matchine_id = world.spawn_empty().id();
        world.entity_mut(state_matchine_id).insert((
            HsmStateMachine::new(HsmStateId::new(state_matchine_id, id1), 10),
            StateLifecycle::default(),
            state_tree,
        ));

        world.flush();
        let state_machine = world.entity(state_matchine_id);
        assert!(state_machine.contains::<ComponentA>());
        assert!(state_machine.contains::<ComponentB>());
        assert!(!state_machine.contains::<ComponentC>());

        world.trigger(HsmTrigger::with_sub(state_matchine_id, id2));

        world.flush();
        let state_machine = world.entity(state_matchine_id);
        assert!(!state_machine.contains::<ComponentA>());
        assert!(!state_machine.contains::<ComponentB>());
        assert!(!state_machine.contains::<ComponentC>());

        world.trigger(HsmTrigger::with_super(state_matchine_id));

        world.flush();
        let state_machine = world.entity(state_matchine_id);
        assert!(state_machine.contains::<ComponentA>());
        assert!(state_machine.contains::<ComponentB>());
        assert!(!state_machine.contains::<ComponentC>());
    }

    #[test]
    #[cfg(feature = "fsm")]
    fn test_fsm_state_machine_state_data() {
        use crate::fsm::{FsmState, event::*, graph::FsmGraph, state_machine::FsmStateMachine};

        let mut app = App::new();
        app.add_plugins(StateMachinePlugin::<Last>::default());

        let world = app.world_mut();

        let state_data = StateData::new(vec![
            world.register_component::<ComponentA>(),
            world.register_component::<ComponentB>(),
        ]);

        let id1 = world
            .spawn((
                FsmState::default(),
                Name::new("StateA"),
                state_data,
                ComponentA,
                ComponentB,
                ComponentC,
            ))
            .id();

        let id2 = world.spawn((FsmState::default(), Name::new("StateB"))).id();

        let mut graph = FsmGraph::new(id1);
        graph.add(id1, id2).add(id2, id1);

        let state_matchine_id = world.spawn_empty().id();
        world
            .entity_mut(state_matchine_id)
            .insert((FsmStateMachine::new(state_matchine_id, id1, 10), graph));

        world.flush();
        let state_machine = world.entity(state_matchine_id);
        assert!(state_machine.contains::<ComponentA>());
        assert!(state_machine.contains::<ComponentB>());
        assert!(!state_machine.contains::<ComponentC>());

        world.trigger(FsmTrigger::with_next(state_matchine_id, id2));

        world.flush();
        let state_machine = world.entity(state_matchine_id);
        assert!(!state_machine.contains::<ComponentA>());
        assert!(!state_machine.contains::<ComponentB>());
        assert!(!state_machine.contains::<ComponentC>());

        world.trigger(FsmTrigger::with_next(state_matchine_id, id1));

        world.flush();
        let state_machine = world.entity(state_matchine_id);
        assert!(state_machine.contains::<ComponentA>());
        assert!(state_machine.contains::<ComponentB>());
        assert!(!state_machine.contains::<ComponentC>());
    }
}
