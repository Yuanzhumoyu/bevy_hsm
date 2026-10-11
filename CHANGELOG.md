# Changelog

本文件记录 `bevy_hsm` 的重要变更。版本号与所支持的 Bevy 版本对齐：`bevy_hsm` 0.20 ⟷ Bevy 0.20。

## [0.20.0] - 2026-10

### 变更

- 依赖升级到 **Bevy 0.20**；`bevy_hsm` 与 `bevy_hsm_macros` 版本号统一为 `0.20.0`。

### 迁移指南（从 0.19 升级）

1. **依赖版本**

   ```toml
   [dependencies]
   bevy = "0.20"
   bevy_hsm = "0.20"
   ```

2. **`iter_many` / `iter_many_mut` 现在产出 `Result`**（Bevy 0.20 的破坏性变更，影响自定义守卫/动作系统）

   ```rust
   // 0.19
   for name in query.iter_many(ids.iter()) { /* ... */ }

   // 0.20：集合内实体一定存在时用 .unwrapped()，
   //       需要容忍失效实体时逐项匹配
   for name in query.iter_many(ids.iter()).unwrapped() { /* ... */ }
   for entry in query.iter_many_mut(ids.iter()) {
       let Ok(mut value) = entry else { continue; };
       // ...
   }
   ```

3. **BSN 语法**（仅使用 `state_data` 的 `state_scene = bsn!{...}` 或自行编写 `bsn!` 时受影响）
   - `Children[` 中分隔多个实体改用 `--`，不再使用 `,` 或 `( ... )` 分组：

     ```rust
     Children[ ComponentA -- ComponentB ]   // 0.19: Children[ComponentA, ComponentB,]
     ```

   - 插入「场景函数调用」需要 `@` 前缀，嵌入表达式则用 `{}`：

     ```rust
     Children [ @my_widget() ]   // 单个 Scene
     Children [ {items}      ]   // SceneList / Vec<impl Scene>
     ```

   - `template_value(x)` 可直接写成 `x`（0.20 已移除该包装的需要）。

4. **观察者事件泛型移入事件类型**：`On<Add, T>` 写作 `On<Add<T>>`，`On<Remove, T>` 写作 `On<Remove<T>>`。

5. **指针事件扁平化**：`Pointer<Click>` → `PointerClick`，坐标字段为 `pointer.position`（`Vec2`），
   需要 `Location` 时用 `pointer.location()`。

### 行为变更

- 状态机转换系统与 HSM/FSM 触发器观察者不再使用 `Populated` 作为「是否存在未暂停状态机」的门控，
  改为函数体内显式判断并早退。此前当世界中**所有**状态机都处于 `Paused` 时，
  `Populated` 校验失败会静默跳过整个观察者，导致到达暂停状态机的触发器
  （含 `Resume` / `Interrupt`）被无声丢弃。现在这类情况可在调用点观测与调试。

### 内部改进

- `StateMachinePlugin` 直接持有调度标签（`Interned<dyn ScheduleLabel>`），不再为延迟安装转换系统
  分配 `Arc<dyn Fn(&mut App)>`；`StateMachinePlugin::with_schedule` 不再要求调度标签实现 `Clone`。
  公共构造方式（`StateMachinePlugin::default()` / `StateMachinePlugin::with_schedule(Last)`）保持不变。
- 清理 4 处未使用导入，`cargo clippy -- -D warnings` 在全部 feature 组合下通过。
- 发布包不再包含 `crates/` 下不属于本库的目录。

### 已知限制

- `bevy_hsm_macros` 与 `bevy_hsm` 必须**按顺序**发布（先宏 crate），否则依赖解析会失败。

## [0.19.0]

- 迁移到 Bevy 0.19。
- 中断/恢复机制、HSM 混合架构（HSM 内嵌 FSM）、历史状态、转换策略与守卫系统完善。
- `state_data` 支持 BSN 场景补丁。
