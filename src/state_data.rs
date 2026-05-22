use std::sync::Arc;

use bevy::{
    ecs::{component::ComponentId, world::DeferredWorld},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    scene::{ScenePatch, SpawnSceneError},
};

#[derive(Component, Clone)]
pub struct StateScenePatch(Arc<ScenePatch>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchResult {
    pub components: Vec<ComponentId>,
    pub children: Vec<Entity>,
}

impl PatchResult {
    pub fn reclaim(&self, entity: &mut EntityWorldMut) {
        entity.remove_by_ids(&self.components);
        let world = unsafe { entity.world_mut() };
        self.children.iter().for_each(|child| {
            world.despawn(*child);
        });
    }

    pub fn reclaim_command(self, service_target: Entity) -> impl Command {
        move |world: &mut World| {
            self.reclaim(&mut world.entity_mut(service_target));
        }
    }
}

impl StateScenePatch {
    pub fn spawn_state_scene(
        world: &mut DeferredWorld,
        state: Entity,
        state_machine: Entity,
        service_target: Entity,
    ) {
        let Some(state_scene_patch) = world.get::<Self>(state).cloned() else {
            return;
        };
        world
            .commands()
            .queue(state_scene_patch.apply_state_scene_command(
                state,
                state_machine,
                service_target,
            ));
    }

    pub fn reclaim_state_scene(
        world: &mut DeferredWorld,
        state: Entity,
        state_machine: Entity,
        service_target: Entity,
    ) {
        let Some(mut reclaimer) = world.get_mut::<StateSceneReclaimer>(state_machine) else {
            return;
        };
        let Some(patch_result) = reclaimer.remove(state) else {
            return;
        };
        world.commands().queue(move |world: &mut World| {
            patch_result.reclaim(&mut world.entity_mut(service_target));
        });
    }

    pub fn apply_state_scene_command(
        self,
        state: Entity,
        state_machine: Entity,
        service_target: Entity,
    ) -> impl Command {
        move |world: &mut World| -> Result<()> {
            let mut entity = world.entity_mut(service_target);
            let patch_result = self.apply(&mut entity)?;

            let mut state_machine = world.entity_mut(state_machine);
            let Some(mut reclaimer) = state_machine.get_mut::<StateSceneReclaimer>() else {
                let mut reclaimer = StateSceneReclaimer::default();
                reclaimer.insert(state, patch_result);
                state_machine.insert(reclaimer);
                return Ok(());
            };
            reclaimer.insert(state, patch_result);
            Ok(())
        }
    }

    pub fn reclaim_state_scene_command(
        state: Entity,
        state_machine: Entity,
        service_target: Entity,
    ) -> impl Command {
        move |world: &mut World| {
            let Some(mut reclaimer) = world.get_mut::<StateSceneReclaimer>(state_machine) else {
                return;
            };
            let Some(patch_result) = reclaimer.remove(state) else {
                return;
            };
            patch_result.reclaim(&mut world.entity_mut(service_target));
        }
    }

    #[inline]
    pub fn with<T: Scene>(
        assets: &AssetServer,
        patches: &Assets<ScenePatch>,
        scene: T,
    ) -> Result<Self> {
        let mut patch = ScenePatch::load(assets, scene);
        patch.resolve(assets, patches)?;
        Ok(StateScenePatch(Arc::new(patch)))
    }

    pub fn apply(&self, entity: &mut EntityWorldMut) -> Result<PatchResult, SpawnSceneError> {
        let old_components =
            HashSet::<ComponentId>::from_iter(entity.archetype().components().iter().copied());

        let old_children = entity
            .get::<Children>()
            .map(|c| c.into_iter().copied().collect::<HashSet<Entity>>())
            .unwrap_or_default();

        self.0.as_ref().apply(entity)?;

        let new_components =
            HashSet::<ComponentId>::from_iter(entity.archetype().components().iter().copied());
        let added_components = new_components
            .difference(&old_components)
            .copied()
            .collect();

        let new_children = entity
            .get::<Children>()
            .map(|c| c.into_iter().copied().collect::<HashSet<Entity>>())
            .unwrap_or_default();
        let added_children = new_children.difference(&old_children).copied().collect();

        Ok(PatchResult {
            components: added_components,
            children: added_children,
        })
    }
}

#[derive(Component, Default)]
pub(crate) struct StateSceneReclaimer(HashMap<Entity, PatchResult>);

impl StateSceneReclaimer {
    pub fn insert(&mut self, entity: Entity, reclaim_data: PatchResult) -> Option<PatchResult> {
        self.0.insert(entity, reclaim_data)
    }

    pub fn remove(&mut self, entity: Entity) -> Option<PatchResult> {
        self.0.remove(&entity)
    }
}

pub trait StateSceneExt {
    fn create_state_scene_patch<T: Scene>(&self, scene: T) -> Result<StateScenePatch>;
}

macro_rules! impl_state_scene_ext {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl StateSceneExt for $ty {
                fn create_state_scene_patch<T: Scene>(&self, scene: T) -> Result<StateScenePatch> {
                    let assets = self.resource::<AssetServer>();
                    let patches = self.resource::<Assets<ScenePatch>>();
                    StateScenePatch::with(assets, patches, scene)
                }
            }
        )+
    };
}

impl_state_scene_ext!(World, EntityWorldMut<'_>, DeferredWorld<'_>);

#[cfg(test)]
mod tests {
    use bevy::scene::ScenePlugin;

    use super::*;

    #[derive(Component, Debug, PartialEq, Eq, Clone, Default)]
    struct ComponentA;

    #[derive(Component, Debug, PartialEq, Eq, Clone, Default)]
    struct ComponentB;

    #[derive(Component, Debug, PartialEq, Eq, Clone, Default)]
    struct ComponentC;

    #[test]
    fn test_state_scene() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(ScenePlugin);

        let world = app.world_mut();
        let state_scene_patch = world
            .create_state_scene_patch(bsn! {
                ComponentB
                Children[
                    ComponentC,
                    ComponentC,
                ]
            })
            .unwrap();
        let mut entity_mut = world.spawn(ComponentA);
        let patch_result = state_scene_patch.apply(&mut entity_mut).unwrap();

        // 检查根实体
        assert!(entity_mut.contains::<ComponentA>()); // 原始组件应保留
        assert!(entity_mut.contains::<ComponentB>()); // 新组件已添加
        assert!(!entity_mut.contains::<ComponentC>()); // 子实体的组件不应在根上

        // 检查子实体
        let world = unsafe { entity_mut.world_mut() };
        let mut query = world.query::<&ComponentC>();
        let component_c_count = query.iter(world).count();
        assert_eq!(component_c_count, 2); // 确认子实体已创建

        // 模拟状态退出和清理
        patch_result.reclaim(&mut entity_mut);

        // 检查清理结果
        assert!(entity_mut.contains::<ComponentA>()); // 原始组件应保留
        assert!(!entity_mut.contains::<ComponentB>()); // 新组件已移除

        let world = unsafe { entity_mut.world_mut() };
        let mut query = world.query::<&ComponentC>();
        let component_c_count = query.iter(world).count();
        assert_eq!(component_c_count, 0); // 确认子实体已被销毁
    }
}
