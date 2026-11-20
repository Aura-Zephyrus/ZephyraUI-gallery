// crate 级用 `deny` 而不是 `forbid`：`alloc_probe` 那块必须豁免（理由见
// 该模块文档）。`deny` 允许按模块 `allow`，`forbid` 不允许 —— 所以这条
// 就是「unsafe 只允许出现在 alloc_probe 一处」的执行方式。
#![deny(unsafe_code)]
//! gallery —— `zephyra-ui` 控件层的参考应用。
//!
//! # 它与控件层的分工
//!
//! | | `ZephyraUI/`（`zephyra-ui`） | 本 crate |
//! |---|---|---|
//! | 回答 | 「一个 Button 长什么样」 | 「哪些 Button 摆在第几页」 |
//! | 产出 | `Element` + 分类名 | 路由、状态、宿主、呈现 |
//! | 后端 | **零依赖** | d3d12 / d3d11 / linux-vk 三选一 |
//! | 文案 | 不含（控件只发 key） | 含（三语文案表） |
//!
//! 判据一句话：**换一个应用还能用吗？** Button 能；首页那张横幅不能。
//!
//! # 架构
//!
//! ```text
//!   输入 ─▶ [`app::Gallery::on_input`]  推进时钟 / 还原 click / 记断点
//!            │
//!            ▼
//!   [`app::Gallery::view`] ─▶ [`ui::window`]   「Window」页实例化（frontend/shell.zypui）
//!            │
//!            ▼
//!     组件模板展开（外壳 + 页面 + 控件组件，全部在 .zypui）
//!            │
//!            ▼
//!                    zephyra_core::FrameCache        diff → layout → paint
//! ```
//!
//! # 模块
//!
//! | 模块 | 职责 |
//! |---|---|
//! | [`app`] | 应用状态 + [`zephyra_fx::View`] 实现 + 路由表 + zypui 宿主 |
//! | [`app::data`] | 宿主绑定的数据面（菜单、日历、语言段……全部 `&'static` 表） |
//! | [`app::shell`] | 外壳尺寸常量（与样式表门禁逐字比对） |
//! | [`app::pages`] | 页面清单（id / 英文标题 / 图标） |
//! | [`ui`] | `frontend/shell.zypui` 的运行时入口（编译期产物 + `include_bytes!`） |
//! | [`styles`] | 合并控件层样式表与本应用自己的 |

/// 每帧分配探针的**实现**。
///
/// # 为什么是实现而不是"注册全局分配器"
///
/// `#[global_allocator]` 只能存在**一个**，而且只能待在**可执行文件**里 ——
/// 放进 lib 会与 bin 里那个 `#[global_allocator] use ...` 冲突
/// （实测 `E0759`：「conflicts with global allocator in: zephyra_gallery」）。
///
/// 所以这里只放**类型与计数器**，注册由 `src/bin/main.rs` 那一行负责。
/// 于是「探针」这个能力在库层可测，而全局状态只有一份。
#[allow(unsafe_code)]
#[cfg(debug_assertions)]
pub mod alloc_probe;

#[path = "../frontend/app/mod.rs"]
pub mod app;
#[path = "../frontend/styles.rs"]
pub mod styles;
#[path = "../frontend/ui.rs"]
pub mod ui;

/// 缓动的转出口 —— 缓动住在 `zephyra_style::anim`（框架层），而应用里的
/// 入场动画天天用它。
///
/// # 为什么要 `pub use` 一层
///
/// 不转发的话每个用到的地方都得写 `zephyra_style::anim::`，而 gallery 还得
/// 在 `Cargo.toml` 里**直接**依赖 `zephyra-style` 才能这么写 —— 那是把
/// 「本 crate 用了什么」的细节漏到每个调用点。
pub use zephyra_style::anim;

/// 断点与触控判据的转出口（同 [`anim`] 的理由）。
pub use zephyra_core::responsive;
