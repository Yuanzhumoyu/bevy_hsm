use bevy::{
    ecs::{
        component::ComponentId,
        entity::{EntityClonerBuilder, OptIn},
        lifecycle::HookContext,
        world::DeferredWorld,
    },
    prelude::*,
};

/// # 状态数据
/// * 持有一个与特定状态相关联的组件类型（`ComponentId`）列表。
///
/// 当状态机进入一个带有 `StateData` 的状态时，这里定义的组件会被添加到目标实体上。
/// 当退出该状态时，这些组件会被移除。
/// 这允许我们管理仅在特定状态下才需要存在的数据。
///
/// # State Data
/// * Holds a list of component types (`ComponentId`) that are associated with a specific state.
///
/// When a state machine enters a state with `StateData`, the components defined here
/// are added to the target entity. When the state is exited, these components are removed.
/// This allows for managing data that is specific to a particular state.
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
            warn!(
                "Attempted to clone components from a non-existent entity: {:?}",
                entity
            );
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
            warn!(
                "Attempted to remove components from a non-existent entity: {:?}",
                entity
            );
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
            entity_mut.remove_by_ids(&self.0);
        }
    }
}

/// # 状态数据包
/// * 一个一次性的“安装器”组件，用于将一个 `Bundle` 动态地添加到实体上。
///
/// 当 `StateDataBundle` 被添加到实体时，它的 `on_insert` 钩子会立即触发。
/// 这个钩子会取出内部包装的 `Bundle`，并将其组件添加到同一个实体上。
/// 完成后，`StateDataBundle` 自身会被移除。
///
/// # State Data Bundle
/// * A one-time "installer" component used to dynamically add a `Bundle` to an entity.
///
/// When a `StateDataBundle` is added to an entity, its `on_insert` hook is immediately triggered.
/// This hook takes the inner wrapped `Bundle` and adds its components to the same entity.
/// After completion, the `StateDataBundle` itself is removed.
#[derive(Component)]
#[component(on_insert = Self::on_insert)]
pub struct StateDataBundle<T: Bundle>(Option<T>);

impl<T> StateDataBundle<T>
where
    T: Bundle,
{
    pub const fn new(bundle: T) -> Self {
        Self(Some(bundle))
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        world.commands().queue(move |world: &mut World| {
            let component_ids = world
                .register_bundle::<T>()
                .contributed_components()
                .to_vec();
            let mut e = world.entity_mut(entity);
            if let Some(bundle) = e.get_mut::<Self>().and_then(|mut e| e.0.take()) {
                e.insert((StateData::new(component_ids), bundle));
            }
            e.remove::<Self>();
        });
    }
}

#[cfg(test)]
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
    fn test_state_data_logic() {
        let mut world = World::new();
        let comp_a_id = world.register_component::<ComponentA>();
        let comp_b_id = world.register_component::<ComponentB>();
        let comp_c_id = world.register_component::<ComponentC>();
        // Test `new()` and sorting
        let mut state_data = StateData::new(vec![comp_c_id, comp_a_id]);
        assert_eq!(*state_data, vec![comp_a_id, comp_c_id]);
        // Test `push()` for a new component
        state_data.push(comp_b_id);
        assert_eq!(*state_data, vec![comp_a_id, comp_b_id, comp_c_id]);
        // Test `push()` for a duplicate component (should not be added again)
        state_data.push(comp_b_id);
        assert_eq!(*state_data, vec![comp_a_id, comp_b_id, comp_c_id]);
        // Test `remove()` for an existing component
        assert!(state_data.remove(comp_b_id));
        assert_eq!(*state_data, vec![comp_a_id, comp_c_id]);
        // Test `remove()` for a non-existent component
        assert!(!state_data.remove(comp_b_id));
        assert_eq!(*state_data, vec![comp_a_id, comp_c_id]);
    }

    #[test]
    #[cfg(feature = "hsm")]
    fn test_hsm_state_machine_state_data() {
        use crate::hsm::{HsmState, event::*, state_machine::*, state_tree::*};

        let mut app = App::new();
        app.add_plugins(StateMachinePlugin::default());

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
        use crate::{
            fsm::{FsmState, event::*, graph::FsmGraph, state_machine::FsmStateMachine},
            guards::GuardRegistry,
            prelude::{ActionDispatch, StateActionRegistry},
        };

        let mut app = App::new();
        app.init_resource::<StateActionRegistry>();
        app.init_resource::<ActionDispatch>();
        app.init_resource::<GuardRegistry>();

        app.add_observer(FsmStateMachine::handle_fsm_trigger);

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
