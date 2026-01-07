use std::{fmt::Debug, hash::Hash, marker::PhantomData, mem::swap, sync::Arc};

use bevy::{
    app::App,
    ecs::{
        component::ComponentId,
        schedule::{IntoScheduleConfigs, ScheduleLabel},
    },
    platform::collections::{Equivalent, HashMap, HashSet},
    prelude::*,
};

use crate::{
    hook_system::HsmStateContext,
    state::{HsmOnState, HsmOnUpdateSystem},
    system_state::system_state_trait::ExpandScheduleLabelFuction,
};

/// # 一个对状态机系统的抽象\An abstraction of a state machine system
/// * In : 输入上下文
/// - In : Input context
/// * Out :
///     * None: 下一帧将不再执行该状态
///     - None: The next frame will no longer execute this state
///     * Some: 继续执行该状态, 里面的数量为空时将视为None
///     - Some: Continue executing this state. When the quantity inside is empty, it will be treated as None
///         * 过滤条件 :
///         * Filter Condition:
///             * `OnUpdate`: 继续执行该状态
///             - `OnUpdate`: Continue executing this state
///             * `OnExit`  : 停止执行该状态
///             - `OnExit`  : Stop executing this state
pub trait IntoActionSystem<M> {
    fn into_system(
        self,
    ) -> impl IntoSystem<In<Vec<HsmStateContext>>, Option<Vec<HsmStateContext>>, M>;
}

impl<F, M> IntoActionSystem<M> for F
where
    F: IntoSystem<In<Vec<HsmStateContext>>, Option<Vec<HsmStateContext>>, M>,
{
    fn into_system(
        self,
    ) -> impl IntoSystem<In<Vec<HsmStateContext>>, Option<Vec<HsmStateContext>>, M> {
        self
    }
}

pub trait SystemState {
    fn add_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + ExpandScheduleLabelFuction + Default,
        action_name: impl Into<String>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self;

    fn add_action_system_anchor_point(
        &mut self,
        schedule: impl ScheduleLabel + ExpandScheduleLabelFuction + Default,
    ) -> &mut Self;
}

impl SystemState for App {
    fn add_action_system<M>(
        &mut self,
        schedule: impl ScheduleLabel + ExpandScheduleLabelFuction + Default,
        action_name: impl Into<String>,
        system: impl IntoActionSystem<M>,
    ) -> &mut Self {
        let world = self.world_mut();
        let action_name = Arc::new(action_name.into());

        // 注册状态系统
        schedule.add_system_info(world, action_name.clone());

        // 添加系统
        let mut schedules = world.resource_mut::<Schedules>();
        schedule.add_system(&mut schedules, action_name, system);
        self
    }

    fn add_action_system_anchor_point(
        &mut self,
        schedule: impl ScheduleLabel + ExpandScheduleLabelFuction + Default,
    ) -> &mut Self {
        let world = self.world_mut();
        let action_name = Arc::new("".to_string());
        // 注册状态系统
        schedule.add_system_info(world, action_name.clone());

        let mut schedules = world.resource_mut::<Schedules>();
        schedule.add_system_anchor_point(&mut schedules, action_name);
        self
    }
}

pub type GetBufferId = Arc<
    dyn Fn(&mut World, Box<dyn FnOnce(&mut World, &mut HsmActionSystemBuffer)>)
        + Send
        + Sync
        + 'static,
>;

/// 状态机系统
///
/// Status machine system
/// # 作用\Effect
/// * 用于获取对应时间点的缓存资源入口
/// - Used to get the entry point of the cache resource at a certain time
/// * Key: "`ScheduleLabel`:`action_name`"
/// * Value: 是如何通过[World]获取缓存[HsmActionSystemBuffer]的方法
/// - Value: How to get the cache resource through [World]
#[derive(Resource, Default, Clone)]
pub struct HsmActionSystems(HashMap<String, GetBufferId>);

impl HsmActionSystems {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

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
/// * Value: `HsmActionSystemBuffers<T: ScheduleLabel>` 的 `ComponentId`
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq, Deref, DerefMut)]
pub(super) struct HsmActionSystemBuffersManager(HashMap<String, ComponentId>);

/// 状态机组系统缓存
///
/// Status machine system cache manager
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
pub(super) struct HsmActionSystemBuffers<T: ScheduleLabel = HsmOnState> {
    buffers: HashMap<String, HsmActionSystemBuffer>,
    _marker: PhantomData<T>,
}

impl<T: ScheduleLabel> HsmActionSystemBuffers<T> {
    pub fn insert_buffer(&mut self, action_name: String, buffer: HsmActionSystemBuffer) {
        self.buffers.insert(action_name, buffer);
    }

    pub fn get_buffer<Q>(&self, action_name: &Q) -> Option<&HsmActionSystemBuffer>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.buffers.get(action_name)
    }

    pub fn get_buffer_mut<Q>(&mut self, action_name: &Q) -> Option<&mut HsmActionSystemBuffer>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.buffers.get_mut(action_name)
    }
}

/// 状态机系统缓存
///
/// Status machine system cache
/// # 作用\Effect
/// * 收集当前帧触发的实体, 并且在下一帧进行系统处理
/// - Collect entities triggered by the current frame and perform system processing in the next frame
#[derive(Default, Clone, PartialEq, Eq)]
pub struct HsmActionSystemBuffer {
    /// 当前帧状态组
    ///
    /// Current frame status group
    pub curr: Vec<HsmStateContext>,
    /// 下一帧状态组
    ///
    /// Next frame status group
    pub next: Vec<HsmStateContext>,
    /// 过滤器: 用于筛选掉下一帧的状态
    ///
    /// Filter: used to filter out the next frame's status
    filter: HashSet<HsmStateContext>,
    /// 拦截器: 用于筛选掉当前帧的状态
    ///
    /// Interceptor: Use to filter out the current frame's status
    interceptor: HashSet<HsmStateContext>,
}

impl HsmActionSystemBuffer {
    /// 获取当前帧状态组
    ///
    /// Get the current state group
    #[inline(always)]
    pub fn get_curr(&self) -> Vec<HsmStateContext> {
        self.curr.clone()
    }

    /// 更新为下一个状态组
    ///
    /// Update to the next state group
    #[inline(always)]
    pub fn update(&mut self) {
        let Self {
            curr, next, filter, ..
        } = self;
        swap(curr, next);

        if !filter.is_empty() {
            let old_curr = std::mem::take(curr);
            *curr = old_curr
                .into_iter()
                .filter(|x| !filter.contains(x))
                .collect::<Vec<_>>();
            filter.clear();
        }

        next.clear();
    }

    /// 更新拦截器
    ///
    /// Update interceptor
    fn update_interceptor(&mut self) {
        if self.curr == self.next {
            return;
        }
        if self.next.is_empty() {
            self.interceptor.extend(self.curr.iter());
            return;
        }
        let iter = self.next.iter().filter(|x| !self.curr.contains(x));
        self.interceptor.extend(iter);
    }

    /// 添加一个上下文
    ///
    /// Add a context
    #[inline(always)]
    pub fn add(&mut self, context: HsmStateContext) {
        if self.interceptor.contains(&context) {
            return;
        }
        self.next.push(context);
    }

    /// 添加多个上下文
    ///
    /// Add multiple contexts
    #[inline(always)]
    pub fn adds<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = HsmStateContext>,
    {
        self.next
            .extend(iter.into_iter().filter(|c| !self.interceptor.contains(c)));
    }

    /// 添加一个过滤器
    ///
    /// Add a filter
    pub fn add_filter(&mut self, context: HsmStateContext) {
        self.filter.insert(context);
    }

    /// 添加一个拦截器
    ///
    /// Add an interceptor
    pub fn add_interceptor(&mut self, context: HsmStateContext) {
        self.interceptor.insert(context);
    }

    /// 移除一个拦截器
    ///
    /// Remove an interceptor
    pub fn remove_interceptor(&mut self, context: HsmStateContext) {
        self.interceptor.remove(&context);
    }

    /// 将当前帧添加到下一帧
    ///
    /// Add the current frame to the next frame
    pub fn reflow(&mut self) {
        self.next.extend(self.curr.iter());
    }

    pub fn is_empty(&self) -> bool {
        self.curr.is_empty()
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
        world: &mut World,
        state_id: Entity,
        f: impl FnOnce(&mut World, &mut HsmActionSystemBuffer) + 'static,
    ) {
        let Some(on_update_system) = world.get::<HsmOnUpdateSystem>(state_id) else {
            return;
        };
        let action_systems = world.resource::<HsmActionSystems>();
        let Some(get_buffer_scope) = action_systems.get(on_update_system.as_str()) else {
            warn!("未找到系统: {}", on_update_system.as_str());
            return;
        };

        (get_buffer_scope)(
            unsafe { world.as_unsafe_world_cell().world_mut() },
            Box::new(f),
        );
    }
}

impl Debug for HsmActionSystemBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HsmActionSystemBuffer[curr: {:?}, next: {:?}, filter: {:?}]",
            self.curr, self.next, self.filter
        )
    }
}

/// 状态机系统运行模式
///
/// State machine system run mode
fn action_system_run_mode<T: ScheduleLabel>(
    action_name: Arc<String>,
) -> impl Fn(In<Option<Vec<HsmStateContext>>>, ResMut<HsmActionSystemBuffers<T>>) {
    move |state_contexts: In<Option<Vec<HsmStateContext>>>,
          mut action_system_buffers: ResMut<HsmActionSystemBuffers<T>>| {
        let Some(buffer) = action_system_buffers.get_buffer_mut(action_name.as_str()) else {
            return;
        };
        if let bevy::prelude::In(Some(state_contexts)) = state_contexts
            && !state_contexts.is_empty()
        {
            buffer.adds(state_contexts);
        }
        buffer.update_interceptor();
    }
}

fn handle_on_update_anchor<T: ScheduleLabel>(
    action_name: Arc<String>,
) -> impl Fn(ResMut<HsmActionSystemBuffers<T>>) {
    move |mut action_system_buffers: ResMut<HsmActionSystemBuffers<T>>| {
        if let Some(buffer) = action_system_buffers.get_buffer_mut(action_name.as_str()) {
            buffer.reflow();
        }
    }
}

/// 运行动作系统的条件
///
/// Run action system condition
fn run_action_system_condition<T: ScheduleLabel>(
    action_name: Arc<String>,
) -> impl Fn(Option<Res<HsmActionSystemBuffers<T>>>) -> bool {
    move |action_system_buffer: Option<Res<HsmActionSystemBuffers<T>>>| {
        action_system_buffer.is_some_and(|buffers| {
            buffers
                .get_buffer(action_name.as_str())
                .is_some_and(|buffer| !buffer.is_empty())
        })
    }
}

/// 更新状态机系统缓存
///
/// Update state machine system cache
fn update_buffer<T: ScheduleLabel>(
    action_name: Arc<String>,
) -> impl Fn(ResMut<HsmActionSystemBuffers<T>>) {
    move |mut action_system_buffers: ResMut<HsmActionSystemBuffers<T>>| {
        if let Some(buffer) = action_system_buffers.get_buffer_mut(action_name.as_str()) {
            buffer.update();
        }
    }
}

/// 获取状态机系统缓存
///
/// Get state machine system cache
fn buffer_input<T: ScheduleLabel>(
    action_name: Arc<String>,
) -> impl Fn(Res<HsmActionSystemBuffers<T>>) -> Vec<HsmStateContext> {
    move |action_system_buffer: Res<HsmActionSystemBuffers<T>>| -> Vec<HsmStateContext> {
        action_system_buffer
            .get_buffer(action_name.as_str())
            .map_or(vec![], |buffer| buffer.get_curr())
    }
}

pub(super) mod system_state_trait {
    use std::sync::Arc;

    use bevy::ecs::{schedule::Schedules, world::World};

    use crate::system_state::IntoActionSystem;

    /// 系统状态
    pub trait ExpandScheduleLabelFuction: Send + Sync + 'static {
        fn add_system_info(&self, world: &mut World, action_name: Arc<String>);

        fn add_system<M>(
            self,
            schedules: &mut Schedules,
            action_name: Arc<String>,
            system: impl IntoActionSystem<M>,
        );

        fn add_system_anchor_point(self, schedules: &mut Schedules, action_name: Arc<String>);
    }
}

impl<T: ScheduleLabel + Default> system_state_trait::ExpandScheduleLabelFuction for T {
    #[inline]
    fn add_system_info(&self, world: &mut World, action_name: Arc<String>) {
        let mut buffers = world.get_resource_or_init::<HsmActionSystemBuffers<T>>();
        buffers.insert_buffer(action_name.to_string(), HsmActionSystemBuffer::default());

        let buffers_id = world.register_resource::<HsmActionSystemBuffers<T>>();
        let mut hsm_action_systems = world.get_resource_or_init::<HsmActionSystems>();
        let label = ShortName::of::<T>();
        let name = match action_name.is_empty() {
            false => format!("{}:{}", label, action_name),
            true => label.to_string(),
        };

        let get_buffer_id =
            move |world: &mut World, f: Box<dyn FnOnce(&mut World, &mut HsmActionSystemBuffer)>| {
                world.resource_scope::<HsmActionSystemBuffers<T>, ()>(|world, mut buffers| {
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
            world.get_resource_or_init::<HsmActionSystemBuffersManager>();
        hsm_action_system_buffer_manager
            .entry(label.to_string())
            .or_insert(buffers_id);
    }

    #[inline]
    fn add_system<M>(
        self,
        schedules: &mut Schedules,
        action_name: Arc<String>,
        system: impl IntoActionSystem<M>,
    ) {
        let action_system = buffer_input::<T>(action_name.clone())
            .pipe(system.into_system())
            .pipe(action_system_run_mode::<T>(action_name.clone()));

        let system = (
            action_system.run_if(run_action_system_condition::<T>(action_name.clone())),
            update_buffer::<T>(action_name),
        )
            .chain();

        schedules.add_systems(self, system);
    }

    fn add_system_anchor_point(self, schedules: &mut Schedules, action_name: Arc<String>) {
        let system = (
            handle_on_update_anchor::<T>(action_name.clone())
                .run_if(run_action_system_condition::<T>(action_name.clone())),
            update_buffer::<T>(action_name),
        )
            .chain();

        schedules.add_systems(self, system);
    }
}
