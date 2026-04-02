use std::{any::TypeId, fmt::Debug, hash::Hash, marker::PhantomData, mem::swap, sync::Arc};

use bevy::{
    app::App,
    ecs::{
        schedule::{IntoScheduleConfigs, ScheduleError, ScheduleLabel},
        world::unsafe_world_cell::UnsafeWorldCell,
    },
    platform::collections::{Equivalent, HashMap, HashSet},
    prelude::*,
};

use crate::{
    action_dispatcher::system_state_trait::ExpandScheduleLabelFunction, context::*,
    labels::SystemLabel, state_actions::*,
};

/// # 一个对状态机系统的抽象\An abstraction of a state machine system
/// * In : 输入上下文
/// - In : Input context
/// * Out : 输出上下文
/// - Out : Output context
///     * None: 下一帧将不再执行该状态
///     - None: The next frame will no longer execute this state
///     * Some: 继续执行该状态, 里面的数量为空时将视为None
///     - Some: Continue executing this state. When the quantity inside is empty, it will be treated as None
///         * 过滤条件 :
///         * Filter Condition:
///             * `OnUpdate`: 继续执行该状态
///             - `OnUpdate`: Continue executing this state
///             * `BeforeExit`  : 停止执行该状态
///             - `BeforeExit`  : Stop executing this state
pub trait IntoActionSystem<M> {
    fn into_system(self) -> impl IntoSystem<In<Vec<ActionContext>>, Option<Vec<ActionContext>>, M>;
}

impl<F, M> IntoActionSystem<M> for F
where
    F: IntoSystem<In<Vec<ActionContext>>, Option<Vec<ActionContext>>, M>,
{
    fn into_system(self) -> impl IntoSystem<In<Vec<ActionContext>>, Option<Vec<ActionContext>>, M> {
        self
    }
}

/// A trait for adding, removing, and replacing action systems in a Bevy `App` or `World`.
/// This provides a unified interface for managing action systems.
///
/// 用于在 Bevy 的 `App` 或 `World` 中添加、删除和替换动作系统的 trait。
/// 这为管理动作系统提供了一个统一的接口。
pub trait SystemState {
    /// 将一个动作系统添加至指定的 `Schedule`。
    ///
    /// # Arguments
    ///
    /// * `schedule`: 将要添加动作系统的 `Schedule`。
    /// * `action_name`: 动作系统的唯一名称。
    /// * `system`: 要添加的动作系统，必须实现 `IntoActionSystem`。
    ///
    /// Adds an action system to the specified schedule.
    ///
    /// # Arguments
    ///
    /// * `schedule`: The schedule to which the action system will be added.
    /// * `action_name`: A unique name for the action system.
    /// * `system`: The action system to add, which must implement `IntoActionSystem`.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn my_action_system(In(contexts): In<Vec<ActionContext>>) -> Option<Vec<ActionContext>> { Some(contexts) }
    /// # fn my_fn() {
    /// let mut app = App::new();
    /// app.add_plugins(StateMachinePlugin::default());
    ///
    /// app.add_action_system(Update, "my_action", my_action_system);
    /// # }
    /// ```
    ///
    fn add_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + Default,
        action_name: impl Into<SystemLabel>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self;

    /// 从指定的 `Schedule` 中移除一个动作系统。
    ///
    /// # Arguments
    ///
    /// * `schedule`: 将要移除动作系统的 `Schedule`。
    /// * `action_name`: 要移除的动作系统的名称。
    ///
    /// # 注意
    ///
    /// 当前函数功能会将所有此前状态机注册的上下文全部删除，
    /// 如果需要继承先前函数的缓冲区，请使用 [Self::replace_action_system()]。
    ///
    /// Removes an action system from the specified schedule.
    ///
    /// # Arguments
    ///
    /// * `schedule`: The schedule from which the action system will be removed.
    /// * `action_name`: The name of the action system to remove.
    ///
    /// # Note
    ///
    /// This function will delete all previously registered contexts of the state machine.
    /// If you need to inherit the buffer of the previous function, please use [Self::replace_action_system()].
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn my_action_system(In(contexts): In<Vec<ActionContext>>) -> Option<Vec<ActionContext>> { Some(contexts) }
    /// # fn my_fn() {
    /// let mut app = App::new();
    /// app.add_plugins(StateMachinePlugin::default());
    ///
    /// app.add_action_system(Update, "my_action", my_action_system);
    /// // ...
    /// app.remove_action_system(Update, "my_action");
    /// # }
    /// ```
    ///
    fn remove_action_system(
        &mut self,
        schedule: impl ScheduleLabel,
        action_name: impl Into<SystemLabel>,
    ) -> &mut Self;

    /// 在指定的 `Schedule` 中用一个新的动作系统替换现有的动作系统。
    ///
    /// # Arguments
    ///
    /// * `schedule`: 将要替换动作系统的 `Schedule`。
    /// * `action_name`: 要替换的动作系统的名称。
    /// * `system`: 新的动作系统。
    ///
    /// # 注意
    ///
    /// 当前函数功能会继承所有此前状态机注册的上下文，
    /// 如果需要删除先前函数的缓冲区，请使用 [Self::remove_action_system()]。
    ///
    /// Replaces an existing action system with a new one in the specified schedule.
    ///
    /// # Arguments
    ///
    /// * `schedule`: The schedule where the action system will be replaced.
    /// * `action_name`: The name of the action system to replace.
    /// * `system`: The new action system.
    ///
    /// # Note
    ///
    /// This function will inherit all contexts previously registered by the state machine.
    /// If you need to delete the buffer of the previous function, please use [Self::remove_action_system()].
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn my_action_system(In(contexts): In<Vec<ActionContext>>) -> Option<Vec<ActionContext>> { Some(contexts) }
    /// # fn my_new_action_system(In(contexts): In<Vec<ActionContext>>) -> Option<Vec<ActionContext>> { Some(contexts) }
    /// # fn my_fn() {
    /// let mut app = App::new();
    /// app.add_plugins(StateMachinePlugin::default());
    ///
    /// app.add_action_system(Update, "my_action", my_action_system);
    /// // ...
    /// app.replace_action_system(Update, "my_action", my_new_action_system);
    /// # }
    /// ```
    ///
    fn replace_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + Clone,
        action_name: impl Into<SystemLabel>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self;
}

impl SystemState for App {
    fn add_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + Default,
        action_name: impl Into<SystemLabel>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self {
        let world = self.world_mut();
        world.add_action_system(schedule, action_name, system);
        self
    }

    fn remove_action_system(
        &mut self,
        schedule: impl ScheduleLabel,
        action_name: impl Into<SystemLabel>,
    ) -> &mut Self {
        let world = self.world_mut();
        world.remove_action_system(schedule, action_name);
        self
    }

    fn replace_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + Clone,
        action_name: impl Into<SystemLabel>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self {
        let world = self.world_mut();
        world.replace_action_system(schedule, action_name, system);
        self
    }
}

impl SystemState for World {
    fn add_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + Default,
        action_name: impl Into<SystemLabel>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self {
        let action_name = action_name.into();

        // 注册状态系统
        schedule.add_system_info(self, action_name.clone()).unwrap();

        // 添加系统
        let system = schedule.configuration_action_system(action_name.clone(), system);
        self.resource_scope(
            |world: &mut World, mut systems: Mut<'_, ActionSystemRegistry>| {
                let index = schedule.push_system_index(action_name, &mut systems);
                world.schedule_scope(schedule, |_world: &mut World, schedule: &mut Schedule| {
                    schedule.add_systems(system.in_set(index));
                });
            },
        );
        self
    }

    fn remove_action_system(
        &mut self,
        schedule: impl ScheduleLabel,
        action_name: impl Into<SystemLabel>,
    ) -> &mut Self {
        let action_name = action_name.into();
        schedule.remove_system_info(self, &action_name).unwrap();
        schedule.remove_system(self, action_name).unwrap();
        self
    }

    fn replace_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + Clone,
        action_name: impl Into<SystemLabel>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self {
        let action_name = action_name.into();
        schedule.replace_system(self, action_name, system).unwrap();
        self
    }
}

pub type GetBufferId =
    Arc<dyn Fn(&mut World, Box<dyn FnOnce(&mut StateActionBuffer)>) + Send + Sync + 'static>;

/// 状态机系统
///
/// Status machine system
/// # 作用\Effect
/// * 用于获取对应时间点的缓存资源入口
/// - Used to get the entry point of the cache resource at a certain time
/// * Key: "`ScheduleLabel`:`action_name`"
/// * Value: 是如何通过[World]获取缓存[StateActionBuffer]的方法
/// - Value: How to get the cache resource through [World]
#[derive(Resource, Default, Clone)]
pub struct ActionDispatch(HashMap<SystemLabel, GetBufferId>);

impl ActionDispatch {
    pub(super) fn insert(&mut self, action_name: impl Into<SystemLabel>, system_id: GetBufferId) {
        let action_name = action_name.into();
        self.0.insert(action_name, system_id);
    }

    pub(super) fn remove<Q>(&mut self, action_name: &Q) -> Option<GetBufferId>
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.0.remove(action_name)
    }

    pub(super) fn get<Q>(&self, action_name: &Q) -> Option<GetBufferId>
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.0.get(action_name).cloned()
    }
}

#[derive(ScheduleLabel, Hash, Debug, PartialEq, Eq, Clone)]
pub(super) struct DefaultActionSchedule;

/// 状态机组系统缓存
///
/// Status machine system cache manager
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
pub(super) struct ScheduleActionBuffers<T: ScheduleLabel = DefaultActionSchedule> {
    buffers: HashMap<SystemLabel, StateActionBuffer>,
    _marker: PhantomData<T>,
}

impl<T: ScheduleLabel> ScheduleActionBuffers<T> {
    pub fn insert_buffer(&mut self, action_name: SystemLabel, buffer: StateActionBuffer) {
        self.buffers.insert(action_name, buffer);
    }

    pub fn get_buffer<Q>(&self, action_name: &Q) -> Option<&StateActionBuffer>
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.buffers.get(action_name)
    }

    pub fn get_buffer_mut<Q>(&mut self, action_name: &Q) -> Option<&mut StateActionBuffer>
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.buffers.get_mut(action_name)
    }

    pub fn remove_buffer<Q>(&mut self, action_name: &Q) -> Option<StateActionBuffer>
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.buffers.remove(action_name)
    }

    pub fn contains<Q>(&self, action_name: &Q) -> bool
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.buffers.contains_key(action_name)
    }
}

/// 状态机系统缓存
///
/// Status machine system cache
/// # 作用\Effect
/// * 收集当前帧触发的实体, 并且在下一帧进行系统处理
/// - Collect entities triggered by the current frame and perform system processing in the next frame
#[derive(Default, Clone, PartialEq, Eq)]
pub struct StateActionBuffer {
    /// 当前帧状态组
    ///
    /// Current frame status group
    pub curr: HashSet<ActionContext>,
    /// 下一帧状态组
    ///
    /// Next frame status group
    pub next: HashSet<ActionContext>,
    /// 过滤器: 用于筛选掉下一帧的状态
    ///
    /// Filter: used to filter out the next frame's status
    filter: HashSet<ActionContext>,
    /// 拦截器: 用于筛选掉当前帧的状态
    ///
    /// Interceptor: Use to filter out the current frame's status
    interceptor: HashSet<ActionContext>,
}

impl StateActionBuffer {
    /// 获取当前帧状态组
    ///
    /// Get the current state group
    #[inline(always)]
    pub fn current_actions(&self) -> Vec<ActionContext> {
        let mut v = Vec::with_capacity(self.curr.len());
        v.extend(self.curr.iter());
        v
    }

    /// 更新为当前状态组
    ///
    /// Update to the current state group
    #[inline(always)]
    pub fn update(&mut self) {
        let Self {
            curr, next, filter, ..
        } = self;
        swap(curr, next);

        if !filter.is_empty() {
            curr.retain(|x| !filter.contains(x));
            filter.clear();
        }

        next.clear();
    }

    /// 更新拦截器
    ///
    /// Update interceptor
    fn update_interceptor(&mut self) {
        self.interceptor.extend(self.curr.difference(&self.next));
    }

    /// 添加一个上下文
    ///
    /// Add a context
    #[inline(always)]
    pub fn add(&mut self, context: ActionContext) {
        if self.interceptor.contains(&context) {
            return;
        }
        self.next.insert(context);
    }

    /// 添加一个过滤器
    ///
    /// Add a filter
    pub fn add_filter(&mut self, context: ActionContext) {
        self.filter.insert(context);
    }

    /// 添加一个拦截器
    ///
    /// Add an interceptor
    pub fn add_interceptor(&mut self, context: ActionContext) {
        self.interceptor.insert(context);
    }

    /// 移除一个过滤器
    ///
    /// Remove a filter
    pub fn remove_filter(&mut self, context: ActionContext) {
        self.filter.remove(&context);
    }

    /// 移除一个拦截器
    ///
    /// Remove an interceptor
    pub fn remove_interceptor(&mut self, context: ActionContext) {
        self.interceptor.remove(&context);
    }

    /// 获取缓存作用域
    ///
    /// Get the buffer scope
    /// # 作用\Effect
    /// * 用于在系统中获取当前状态的缓存作用域
    /// - Used to get the cache scope of the current state
    /// * 可以在作用域中添加或修改状态上下文
    /// - Can add or modify state contexts in the scope
    /// * 作用域结束后，会自动更新缓存
    /// - The scope will automatically update the cache after ending
    pub fn buffer_scope(
        world: UnsafeWorldCell,
        state_id: Entity,
        f: impl FnOnce(&mut StateActionBuffer) + 'static,
    ) {
        // SAFETY: 该函数必须在系统中调用，并且保证在调用过程中不会有并发访问 `World` 的情况发生。
        let world = unsafe { world.world_mut() };
        let Some(on_update_system) = world.get::<OnUpdateSystem>(state_id) else {
            return;
        };
        let guard_registry = world.resource::<ActionDispatch>();
        let Some(get_buffer_scope) = guard_registry.get(on_update_system) else {
            warn!("{}", on_update_system.not_found_error(state_id));
            return;
        };

        (get_buffer_scope)(world, Box::new(f));
    }
}

impl Debug for StateActionBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StateActionBuffer[curr: {:?}, next: {:?}, filter: {:?}, interceptor: {:?} ]",
            self.curr, self.next, self.filter, self.interceptor
        )
    }
}

impl Extend<ActionContext> for StateActionBuffer {
    fn extend<T: IntoIterator<Item = ActionContext>>(&mut self, iter: T) {
        self.next
            .extend(iter.into_iter().filter(|c| !self.interceptor.contains(c)));
    }
}

/// 创建一个处理动作系统运行逻辑的闭包。
/// 这个闭包会接收来自前一个系统的 `ActionContext`，并将其添加到对应的 `StateActionBuffer` 中。
fn create_action_system_runner<T: ScheduleLabel>(
    action_name: SystemLabel,
) -> impl Fn(In<Option<Vec<ActionContext>>>, ResMut<ScheduleActionBuffers<T>>) {
    move |state_contexts: In<Option<Vec<ActionContext>>>,
          mut action_system_buffers: ResMut<ScheduleActionBuffers<T>>| {
        let Some(buffer) = action_system_buffers.get_buffer_mut(&action_name) else {
            return;
        };
        if let bevy::prelude::In(Some(state_contexts)) = state_contexts
            && !state_contexts.is_empty()
        {
            buffer.extend(state_contexts);
        }
        buffer.update_interceptor();
    }
}

/// 创建一个用于判断动作系统是否应该运行的条件闭包。
/// 只有当对应的 `StateActionBuffer` 的 `next` 缓冲区不为空时，系统才会运行。
fn create_run_condition_for_action_system<T: ScheduleLabel>(
    action_name: SystemLabel,
) -> impl Fn(Option<Res<ScheduleActionBuffers<T>>>) -> bool {
    move |action_system_buffer: Option<Res<ScheduleActionBuffers<T>>>| {
        action_system_buffer.is_some_and(|buffers| {
            buffers
                .get_buffer(&action_name)
                .is_some_and(|buffer| !buffer.next.is_empty())
        })
    }
}

/// 创建一个更新 `StateActionBuffer` 并返回当前动作的闭包。
/// 这个闭包是动作系统管道的第一个阶段，它负责准备好当前帧需要处理的 `ActionContext`。
fn create_buffer_updater_and_get_actions<T: ScheduleLabel>(
    action_name: SystemLabel,
) -> impl Fn(ResMut<ScheduleActionBuffers<T>>) -> Vec<ActionContext> {
    move |mut action_system_buffers: ResMut<ScheduleActionBuffers<T>>| -> Vec<ActionContext> {
        if let Some(buffer) = action_system_buffers.get_buffer_mut(&action_name) {
            buffer.update();
            buffer.current_actions()
        } else {
            Vec::new()
        }
    }
}

pub(super) mod system_state_trait {
    use bevy::ecs::{
        schedule::{IntoScheduleConfigs, ScheduleConfigs, ScheduleLabel},
        system::{IntoSystem, System},
        world::World,
    };

    use crate::{
        action_dispatcher::{
            ActionSystemSet, IntoActionSystem, create_action_system_runner,
            create_buffer_updater_and_get_actions, create_run_condition_for_action_system,
        },
        labels::SystemLabel,
        prelude::ActionSystemRegistry,
    };

    /// 系统状态
    pub trait ExpandScheduleLabelFunction: Send + Sync + 'static {
        fn configuration_action_system<M>(
            &self,
            action_name: SystemLabel,
            system: impl IntoActionSystem<M>,
        ) -> ScheduleConfigs<Box<dyn System<In = (), Out = ()> + 'static>>
        where
            Self: ScheduleLabel + Sized,
        {
            let action_system = create_buffer_updater_and_get_actions::<Self>(action_name.clone())
                .pipe(system.into_system())
                .pipe(create_action_system_runner::<Self>(action_name.clone()));
            action_system.run_if(create_run_condition_for_action_system::<Self>(action_name))
        }

        #[inline]
        fn push_system_index(
            &self,
            action_name: SystemLabel,
            update_systems: &mut ActionSystemRegistry,
        ) -> ActionSystemSet
        where
            Self: ScheduleLabel + Sized,
        {
            update_systems.push::<Self>(action_name)
        }

        #[inline]
        fn replace_system_index(
            &self,
            action_name: &SystemLabel,
            update_systems: &mut ActionSystemRegistry,
        ) -> Option<ActionSystemSet>
        where
            Self: ScheduleLabel + Sized,
        {
            update_systems.replace::<Self>(action_name)
        }

        fn add_system_info(
            &self,
            world: &mut World,
            action_name: SystemLabel,
        ) -> bevy::prelude::Result<()>
        where
            Self: Default;

        fn remove_system_info(
            &self,
            world: &mut World,
            action_name: &SystemLabel,
        ) -> bevy::prelude::Result<()>;

        fn remove_system(
            self,
            world: &mut World,
            action_name: SystemLabel,
        ) -> bevy::prelude::Result<()>;

        fn replace_system<M>(
            self,
            world: &mut World,
            action_name: SystemLabel,
            system: impl IntoActionSystem<M>,
        ) -> bevy::prelude::Result<()>
        where
            Self: Clone;
    }
}

impl<T: ScheduleLabel> system_state_trait::ExpandScheduleLabelFunction for T {
    #[inline]
    fn add_system_info(
        &self,
        world: &mut World,
        action_name: SystemLabel,
    ) -> bevy::prelude::Result<()>
    where
        Self: Default,
    {
        let mut buffers = world.get_resource_or_init::<ScheduleActionBuffers<T>>();
        if buffers.contains(&action_name) {
            return Err(ActionSystemError::ActionBufferAlreadyExists(
                action_name.clone(),
                std::any::type_name::<T>(),
            )
            .into());
        }
        buffers.insert_buffer(action_name.clone(), StateActionBuffer::default());

        let label = ShortName::of::<T>();
        let name = match action_name.is_empty() {
            false => format!("{}:{}", label, action_name),
            true => label.to_string(),
        };
        let get_buffer_id = move |world: &mut World, f: Box<dyn FnOnce(&mut StateActionBuffer)>| {
            let mut buffers = world.resource_mut::<ScheduleActionBuffers<T>>();
            let Some(buffer) = buffers.get_buffer_mut(&action_name) else {
                warn!("Action buffer for system label {} not found", action_name);
                return;
            };
            f(buffer);
        };

        let mut hsm_action_systems = world.get_resource_or_init::<ActionDispatch>();
        hsm_action_systems.insert(name, Arc::new(get_buffer_id));
        Ok(())
    }

    fn remove_system_info(
        &self,
        world: &mut World,
        action_name: &SystemLabel,
    ) -> bevy::prelude::Result<()> {
        let mut buffers = world.resource_mut::<ScheduleActionBuffers<T>>();
        if !buffers.contains(action_name) {
            return Err(ActionSystemError::ActionBufferNotExists(
                action_name.clone(),
                std::any::type_name::<T>(),
            )
            .into());
        }

        buffers.remove_buffer(action_name);

        let label = ShortName::of::<T>();
        let name = match action_name.is_empty() {
            false => format!("{}:{}", label, action_name),
            true => label.to_string(),
        };
        let mut hsm_action_systems = world.get_resource_or_init::<ActionDispatch>();
        hsm_action_systems.remove(name.as_str());
        Ok(())
    }

    fn remove_system(
        self,
        world: &mut World,
        action_name: SystemLabel,
    ) -> bevy::prelude::Result<()> {
        world.resource_scope(|world: &mut World, mut systems: Mut<'_, ActionSystemRegistry>| {
            let Some(index) = systems.remove::<T>(&action_name) else {
                return Err(ActionSystemError::SystemNotFound(action_name));
            };
            world.schedule_scope(self,move|world:&mut World,schedule:&mut Schedule|{
                schedule.remove_systems_in_set( index,world, bevy::ecs::schedule::ScheduleCleanupPolicy::RemoveSetAndSystemsAllowBreakages)?;
                Ok(())
            },
        )})?;
        Ok(())
    }

    fn replace_system<M>(
        self,
        world: &mut World,
        action_name: SystemLabel,
        system: impl IntoActionSystem<M>,
    ) -> bevy::prelude::Result<()>
    where
        Self: Clone,
    {
        let system = self.configuration_action_system(action_name.clone(), system);
        world.resource_scope(
                |world: &mut World, mut systems: Mut<'_, ActionSystemRegistry>| {
                    let new_index=ActionSystemSet(systems.counter);
                    let Some(index)= self.replace_system_index(&action_name,&mut systems) else {
                        return Err(ActionSystemError::SystemNotFound(action_name));
                    };
                    world.schedule_scope(self,|world: &mut World, schedule:&mut Schedule| {
                    schedule.add_systems(system.in_set(new_index));

                    schedule.remove_systems_in_set(index,world, bevy::ecs::schedule::ScheduleCleanupPolicy::RemoveSetAndSystemsAllowBreakages)?;
                    Ok(())
                },
            )
        })?;
        Ok(())
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ActionSystemRegistry {
    systems: HashMap<TypeId, HashMap<SystemLabel, ActionSystemSet>>,
    counter: usize,
}

impl ActionSystemRegistry {
    pub fn push<T: ScheduleLabel>(&mut self, action_name: SystemLabel) -> ActionSystemSet {
        let index = ActionSystemSet(self.counter);
        self.systems
            .entry(TypeId::of::<T>())
            .or_default()
            .insert(action_name, index);
        self.counter += 1;
        index
    }

    pub fn remove<T: ScheduleLabel>(
        &mut self,
        action_name: &SystemLabel,
    ) -> Option<ActionSystemSet> {
        self.systems
            .get_mut(&TypeId::of::<T>())
            .and_then(|map| map.remove(action_name))
    }

    pub fn replace<T: ScheduleLabel>(
        &mut self,
        action_name: &SystemLabel,
    ) -> Option<ActionSystemSet> {
        let new = ActionSystemSet(self.counter);
        self.counter += 1;
        self.systems.get_mut(&TypeId::of::<T>()).and_then(|map| {
            map.get_mut(action_name)
                .map(|old| std::mem::replace(old, new))
        })
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct ActionSystemSet(usize);

#[derive(Debug)]
enum ActionSystemError {
    SystemNotFound(SystemLabel),
    ActionBufferAlreadyExists(SystemLabel, &'static str),
    ActionBufferNotExists(SystemLabel, &'static str),
    ScheduleError(ScheduleError),
}

impl From<ActionSystemError> for bevy::prelude::BevyError {
    fn from(value: ActionSystemError) -> Self {
        match value {
            ActionSystemError::SystemNotFound(system_label) => {
                format!("System with label {} not found", system_label).into()
            }
            ActionSystemError::ActionBufferAlreadyExists(system_label, schedule_name) => format!(
                "The system<{}> for this ScheduleLabel<{}> already exists",
                system_label, schedule_name
            )
            .into(),
            ActionSystemError::ActionBufferNotExists(system_label, schedule_name) => format!(
                "The system<{}> for this ScheduleLabel<{}> does not exist",
                system_label, schedule_name
            )
            .into(),
            ActionSystemError::ScheduleError(schedule_error) => schedule_error.into(),
        }
    }
}

impl From<ScheduleError> for ActionSystemError {
    fn from(value: ScheduleError) -> Self {
        ActionSystemError::ScheduleError(value)
    }
}
