use std::sync::Arc;

use bevy::{
    ecs::{component::ComponentId, world::DeferredWorld},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    scene::{ScenePatch, SpawnSceneError},
};

/// # 状态场景补丁\State Scene Patch
/// * 一个组件，包含预编译的场景补丁数据。当附加到 HSM 状态实体时，进入状态会自动应用补丁（添加组件/子实体），退出状态时会回收这些更改。
/// - A component containing pre-compiled scene patch data. When attached to an HSM state entity,
///   entering the state automatically applies the patch (adding components/children), and exiting
///   reclaims those changes.
///
/// ## Example
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn setup(world: &mut World) {
/// let patch = world
///     .create_state_scene_patch(bsn! {
///         SomeComponent
///         Children[
///             ChildComponent,
///         ]
///     })
///     .unwrap();
/// world.spawn((Name::new("MyState"), HsmState::default(), patch));
/// # }
/// ```
#[derive(Component, Clone)]
pub struct StateScenePatch(Arc<ScenePatch>);

/// # 补丁结果\Patch Result
/// * 记录场景补丁应用后新增的组件和子实体，用于退出状态时的精确回收。
/// - Records the components and child entities added by a scene patch application,
///   enabling precise reclamation on state exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchResult {
    /// 新增的组件 ID 列表\List of added component IDs
    pub components: Vec<ComponentId>,
    /// 新增的子实体列表\List of added child entities
    pub children: Vec<Entity>,
}

impl PatchResult {
    /// 回收补丁应用的所有更改：移除新增组件并销毁新增子实体。
    ///
    /// Reclaims all changes applied by the patch: removes added components and despawns added children.
    pub fn reclaim(&self, entity: &mut EntityWorldMut) {
        entity.remove_by_ids(&self.components);
        let world = unsafe { entity.world_mut() };
        self.children.iter().for_each(|child| {
            world.despawn(*child);
        });
    }

    /// 创建一个回收命令，用于延迟回收操作。
    ///
    /// Creates a reclamation command for deferred reclamation.
    pub fn reclaim_command(self, service_target: Entity) -> impl Command {
        move |world: &mut World| {
            self.reclaim(&mut world.entity_mut(service_target));
        }
    }
}

impl StateScenePatch {
    /// 在进入状态时应用场景补丁：从状态实体读取 [`StateScenePatch`] 并应用到服务目标实体。
    ///
    /// Applies the scene patch on state enter: reads [`StateScenePatch`] from the state entity
    /// and applies it to the service target entity.
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

    /// 在退出状态时回收场景补丁：从状态机实体的 [`StateSceneReclaimer`] 中移除并回收之前应用的补丁。
    ///
    /// Reclaims the scene patch on state exit: removes and reclaims a previously applied patch
    /// from the state machine entity's [`StateSceneReclaimer`].
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

    /// 从场景数据构建一个 [`StateScenePatch`]，加载并解析场景补丁。
    ///
    /// Constructs a [`StateScenePatch`] from scene data, loading and resolving the scene patch.
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

/// # 状态场景扩展\State Scene Extension
/// * 为 `World`、`EntityWorldMut`、`DeferredWorld` 提供创建 [`StateScenePatch`] 的便捷方法。
/// - Provides convenience methods for creating [`StateScenePatch`] from
///   `World`, `EntityWorldMut`, and `DeferredWorld`.
pub trait StateSceneExt {
    /// 使用 BSN 宏或场景数据创建状态场景补丁。
    ///
    /// Creates a state scene patch from BSN macro or scene data.
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
