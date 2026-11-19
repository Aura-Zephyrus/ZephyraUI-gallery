//! 外壳**常量**：画布、标题栏、导航栏、内容卡的基准尺寸。
//!
//! # 迁移后这里没有结构
//!
//! 窗口外壳的**结构**（标题栏 + 导航 + 内容卡）住在 `frontend/shell.zypui` 的
//! `Window` 页里；这里是它引用的尺寸常量 —— 它们同时是布局几何与门禁
//! （`shell_metrics_match_stylesheet`）比对的**真相来源**：
//! 「代码里 240、样式表里 220」这种两处真相靠那条门禁挡住。
//!
//! # "置底"怎么实现（**当前没有置底项**）
//!
//! core 没有 flex-grow。RinUI 把 Settings 钉在导航栏底部，早期这里是靠一个
//! **定高占位**把 Settings 推到底 —— 占位高度是布局几何（不是外观），
//! 且有一条几何门禁 (`settings_lands_near_the_bottom`) 真去量布局后的矩形。
//!
//! ⚠ 2026-11-01 起**设置页整页撤下**，`NAV_SPACER` 与那条门禁一起删了
//! （导航栏里不再有置底项，占位没有意义）。若将来要加回置底项，
//! 定高占位 + 同款几何门禁要**一起**加回来 —— 只加占位会让导航栏底部
//! 留一块空白，而门禁不再盯着它。

/// 画布宽（RinUI gallery 的窗口宽）。
///
/// ⚠️ 它只是**基准画布**的宽度，不是窗口的宽度。窗口宽度由宿主按 `inner_size()`
/// 给（见 `Gallery::viewport`），外壳的宽度方向全部靠 `cross-align: stretch`
/// 铺满 —— 这里保留常量只为让基准尺寸有名字可引用。
pub const CANVAS_W: f32 = 1400.0;
/// 画布高。同 [`CANVAS_W`]：基准值，不是窗口高。
pub const CANVAS_H: f32 = 800.0;
/// 标题栏高（Apple HIG toolbar 高度）。
pub const TITLEBAR_H: f32 = 44.0;
/// 导航栏宽（Apple HIG 侧栏宽度）。
pub const NAV_W: f32 = 260.0;
/// **紧凑档**的导航栏宽：只留图标列（WinUI NavigationView 的 `LeftCompact`）。
///
/// 出处见 `docs/developer/responsive-and-touch.md` §6 引 `responsive-design` 页。取 56
/// 而不是更小：图标列本身是 32 宽（**4 的倍数**，官方要求基单位为 4 epx），
/// 加两侧 padding 后不能把图标压到 32 以下。
pub const NAV_W_COMPACT: f32 = 56.0;
/// 导航栏高 = 画布高 - 标题栏高。
pub const NAV_H: f32 = CANVAS_H - TITLEBAR_H;
/// 内容卡宽 = 画布宽 - 导航宽 - 1px 接缝（RinUI StackView 的 leftMargin）。
pub const CONTENT_W: f32 = CANVAS_W - NAV_W - 1.0;
/// 导航项数（= `ROUTES` 的长度）。
///
/// 显式写成一个常量而不是 `ROUTES.len()`：几何门禁要比对**算出来的数**与
/// **期望的数**，用 `.len()` 会让两边永远相等，于是改路由数量时门禁失去作用。
///
/// 2026-11-01：设置页整页撤下，导航栏不再有「置底项」，于是
/// `.nav-spacer`（把置底项推到容器底部的定高占位）也一并删掉 ——
/// 它只为置底项而存在，留着就是一块没人用的空白。
pub const NAV_ITEMS: usize = 8;
