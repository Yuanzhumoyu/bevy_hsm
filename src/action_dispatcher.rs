use std::{fmt::Debug, hash::Hash, marker::PhantomData, mem::swap, sync::Arc};

use bevy::{
    app::App,
    ecs::{
        component::ComponentId,
        schedule::{IntoScheduleConfigs, ScheduleLabel},
        world::unsafe_world_cell::UnsafeWorldCell,
    },
    platform::collections::{Equivalent, HashMap, HashSet},
    prelude::*,
};

use crate::{
    action_dispatcher::system_state_trait::ExpandScheduleLabelFunction, context::*,
    error::StateMachineError, state_actions::*,
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

pub trait SystemState {
    fn add_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + ExpandScheduleLabelFunction + Default,
        action_name: impl Into<String>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self;
}

impl SystemState for App {
    fn add_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + ExpandScheduleLabelFunction + Default,
        action_name: impl Into<String>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self {
        let world = self.world_mut();
        let action_name = Arc::new(action_name.into());

        // 注册状态系统
        if let Err(e) = schedule.add_system_info(world, action_name.clone()) {
            warn!("{}", e);
            return self;
        }

        // 添加系统
        let mut schedules = world.resource_mut::<Schedules>();
        schedule.add_system(&mut schedules, action_name, system);
        self
    }
}

pub type GetBufferId = Arc<
    dyn Fn(&mut World, Box<dyn FnOnce(&mut World, &mut StateActionBuffer)>) + Send + Sync + 'static,
>;

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
pub struct ActionDispatch(HashMap<String, GetBufferId>);

impl ActionDispatch {
    pub(super) fn insert(&mut self, action_name: impl Into<String>, system_id: GetBufferId) {
        let action_name = action_name.into();
        self.0.insert(action_name, system_id);
    }

    pub(super) fn get<Q>(&self, action_name: &Q) -> Option<GetBufferId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get(action_name).cloned()
    }
}

/// 状态机系统缓存管理器
///
/// Status machine system cache manager
/// * Key: `ScheduleLabel`
/// * Value: `ScheduleActionBuffers<T: ScheduleLabel>` 的 `ComponentId`
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq, Deref, DerefMut)]
pub(super) struct ScheduleBufferIndex(HashMap<String, ComponentId>);

#[derive(ScheduleLabel, Hash, Debug, PartialEq, Eq, Clone)]
pub(super) struct DefaultActionSchedule;

/// 状态机组系统缓存
///
/// Status machine system cache manager
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
pub(super) struct ScheduleActionBuffers<T: ScheduleLabel = DefaultActionSchedule> {
    buffers: HashMap<String, StateActionBuffer>,
    _marker: PhantomData<T>,
}

impl<T: ScheduleLabel> ScheduleActionBuffers<T> {
    pub fn insert_buffer(&mut self, action_name: String, buffer: StateActionBuffer) {
        self.buffers.insert(action_name, buffer);
    }

    pub fn get_buffer<Q>(&self, action_name: &Q) -> Option<&StateActionBuffer>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.buffers.get(action_name)
    }

    pub fn get_buffer_mut<Q>(&mut self, action_name: &Q) -> Option<&mut StateActionBuffer>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.buffers.get_mut(action_name)
    }

    pub fn contains<Q>(&self, action_name: &Q) -> bool
    where
        Q: Hash + Equivalent<String> + ?Sized,
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
        self.curr.iter().copied().collect()
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
        f: impl FnOnce(&mut World, &mut StateActionBuffer) + 'static,
    ) {
        let world = unsafe { world.world_mut() };
        let Some(on_update_system) = world.get::<OnUpdateSystem>(state_id) else {
            return;
        };
        let guard_registry = world.resource::<ActionDispatch>();
        let Some(get_buffer_scope) = guard_registry.get(on_update_system.as_str()) else {
            warn!(
                "{}",
                StateMachineError::SystemNotFound {
                    system_name: on_update_system.as_str().to_string(),
                    state: state_id
                }
            );
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
    action_name: Arc<String>,
) -> impl Fn(In<Option<Vec<ActionContext>>>, ResMut<ScheduleActionBuffers<T>>) {
    move |state_contexts: In<Option<Vec<ActionContext>>>,
          mut action_system_buffers: ResMut<ScheduleActionBuffers<T>>| {
        let Some(buffer) = action_system_buffers.get_buffer_mut(action_name.as_str()) else {
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
    action_name: Arc<String>,
) -> impl Fn(Option<Res<ScheduleActionBuffers<T>>>) -> bool {
    move |action_system_buffer: Option<Res<ScheduleActionBuffers<T>>>| {
        action_system_buffer.is_some_and(|buffers| {
            buffers
                .get_buffer(action_name.as_str())
                .is_some_and(|buffer| !buffer.next.is_empty())
        })
    }
}

/// 创建一个更新 `StateActionBuffer` 并返回当前动作的闭包。
/// 这个闭包是动作系统管道的第一个阶段，它负责准备好当前帧需要处理的 `ActionContext`。
fn create_buffer_updater_and_get_actions<T: ScheduleLabel>(
    action_name: Arc<String>,
) -> impl Fn(ResMut<ScheduleActionBuffers<T>>) -> Vec<ActionContext> {
    move |mut action_system_buffers: ResMut<ScheduleActionBuffers<T>>| -> Vec<ActionContext> {
        if let Some(buffer) = action_system_buffers.get_buffer_mut(action_name.as_str()) {
            buffer.update();
            buffer.current_actions()
        } else {
            Vec::new()
        }
    }
}

pub(super) mod system_state_trait {
    use std::sync::Arc;

    use bevy::ecs::{schedule::Schedules, world::World};

    use crate::action_dispatcher::IntoActionSystem;

    /// 系统状态
    pub trait ExpandScheduleLabelFunction: Send + Sync + 'static {
        fn add_system_info(
            &self,
            world: &mut World,
            action_name: Arc<String>,
        ) -> Result<(), String>;

        fn add_system<M>(
            self,
            schedules: &mut Schedules,
            action_name: Arc<String>,
            system: impl IntoActionSystem<M>,
        );
    }
}

impl<T: ScheduleLabel + Default> system_state_trait::ExpandScheduleLabelFunction for T {
    #[inline]
    fn add_system_info(&self, world: &mut World, action_name: Arc<String>) -> Result<(), String> {
        let mut buffers = world.get_resource_or_init::<ScheduleActionBuffers<T>>();
        if buffers.contains(action_name.as_str()) {
            return Err(format!(
                "The system<{}> for this ScheduleLabel<{:?}> already exists",
                action_name, self
            ));
        }

        buffers.insert_buffer(action_name.to_string(), StateActionBuffer::default());

        let buffers_id = world.register_resource::<ScheduleActionBuffers<T>>();
        let mut hsm_action_systems = world.get_resource_or_init::<ActionDispatch>();
        let label = ShortName::of::<T>();
        let name = match action_name.is_empty() {
            false => format!("{}:{}", label, action_name),
            true => label.to_string(),
        };

        let get_buffer_id =
            move |world: &mut World, f: Box<dyn FnOnce(&mut World, &mut StateActionBuffer)>| {
                world.resource_scope::<ScheduleActionBuffers<T>, ()>(|world, mut buffers| {
                    let Some(buffer) = buffers.get_buffer_mut(action_name.as_str()) else {
                        warn!(
                            "Buffer not found in buffers map, action_name: {}",
                            action_name
                        );
                        return;
                    };
                    f(world, buffer)
                });
            };
        hsm_action_systems.insert(name, Arc::new(get_buffer_id));

        let mut hsm_action_system_buffer_manager =
            world.get_resource_or_init::<ScheduleBufferIndex>();
        hsm_action_system_buffer_manager
            .entry(label.to_string())
            .or_insert(buffers_id);
        Ok(())
    }

    #[inline]
    fn add_system<M>(
        self,
        schedules: &mut Schedules,
        action_name: Arc<String>,
        system: impl IntoActionSystem<M>,
    ) {
        let action_system = create_buffer_updater_and_get_actions::<T>(action_name.clone())
            .pipe(system.into_system())
            .pipe(create_action_system_runner::<T>(action_name.clone()));

        let system = action_system.run_if(create_run_condition_for_action_system::<T>(
            action_name.clone(),
        ));

        schedules.add_systems(self, system);
    }
}
