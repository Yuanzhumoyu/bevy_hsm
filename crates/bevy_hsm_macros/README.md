# bevy_hsm_macros

`bevy_hsm` 的过程宏实现 crate。

**一般不需要直接依赖它**：请依赖 [`bevy_hsm`](https://crates.io/crates/bevy_hsm)，它的 `hsm` /
`fsm` / `hybrid` / `history` / `state_data` feature 会自动转发到本 crate 对应的 feature。

```toml
[dependencies]
bevy_hsm = "0.20"
```

本 crate 提供以下宏（均由 `bevy_hsm` 的 prelude 重新导出）：

| 宏 | 说明 | 需要的 feature |
| ---------- | --------- | --------- |
| `hsm!` | HSM 状态树声明式定义 | `hsm`（转发） |
| `hsm_tree!` | 状态树字面量 | `hsm`（转发） |
| `fsm!` | FSM 图声明式定义 | `fsm`（转发） |
| `fsm_graph!` | FS 图字面量 | `fsm`（转发） |
| `combination_condition!` | 守卫组合条件 | `hsm` / `fsm` |
| `guard_condition!` | 守卫条件字面量 | `hsm` / `fsm` |

## Features

`hsm` / `fsm` / `hybrid` / `history` / `state_data`，与 `bevy_hsm` 同名 feature 一一对应；
单独使用时默认关闭全部 feature。

## 版本约定

本 crate 与 `bevy_hsm` 版本号保持一致，并且和所支持的 Bevy 版本号对齐
（例如 `0.20.x` 同时表示 `bevy_hsm` 0.20 与 Bevy 0.20）。

跟随 Bevy 的**已发布版本**（不跟踪 Bevy `main` 分支）：

| Bevy | bevy_hsm_macros |
| ----------- | ----------- |
| 0.20 | 0.20 |
| 0.19 | 0.19 |

## 平台支持

仅支持 `std`（未做 `no_std` 适配）；生成代码依赖的 `bevy` API 由使用方的 `bevy` 版本提供。

## 许可

MIT OR Apache-2.0，详见仓库根目录的 `LICENSE-MIT.txt` 与 `LICENSE-APACHE.txt`。
