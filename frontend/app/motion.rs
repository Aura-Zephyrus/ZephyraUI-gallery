//! 下拉菜单开合的定时补间：iOS 连续曲线 + 可中断的起跑点。
//!
//! # 为什么不是指数逼近
//!
//! 旧实现每帧 `v += (target - v) * k`。它永不真正到达目标（只能靠阈值硬夹），
//! 而且**没有曲线**：前 16 ms 就冲掉约 25%，与 Apple 菜单「快起步、长尾软」
//! 的观感对不上。指数逼近适合「跟手」的连续输入（滚动、拖拽），不适合
//! 「一次点击 → 一次开合」这种有明确起止的转场。
//!
//! # 为什么存 `from` 与 `elapsed` 而不只存当前值
//!
//! 用户可以在开合中途再点一次。只存当前值 + 目标的话，新动画会从**上一帧的
//! 位置**以固定速度重新逼近，看起来像跳了一下；存 `from`/`elapsed` 则是
//! 「从当前位置重新起跑」，曲线连续。这与 iOS/macOS 菜单被打断时的行为同源。
//!
//! # 曲线在哪
//!
//! [`zephyra_style::anim::Easing::AppleMenu`]：`cubic-bezier(0.22, 1.0, 0.36, 1.0)`。
//! 外观值（透明度档位、位移）仍由 `.zstyle` / 宿主绑定决定，本模块只产出
//! 0..1 的进度 —— 与 `docs/developer/animation.md` §3 的「代码只发进度」一致。
//!
//! # 时长
//!
//! 打开 240 ms、关闭 180 ms。退出比进入短是 Apple 的通用口径：关闭时用户
//! 已经知道自己要什么，停留越久越像卡顿。

use zephyra_style::anim::{progress, Easing};

/// 打开时长（毫秒）。
pub const OPEN_MS: f32 = 240.0;

/// 关闭时长（毫秒）。
pub const CLOSE_MS: f32 = 180.0;

/// 菜单形变档位数。
///
/// 60 档 × 240 ms = 4 ms/档，配合 iOS 连续曲线后几何变化接近连续。
/// 每档在 `.zstyle` 里给出尺寸、圆角、内边距与透明度——代码只发 `dd-morph-N`。
pub const MORPH_STEPS: u32 = 60;

/// 兼容旧名：淡入现在由 `dd-morph-N` 一并承载，档位密度不变。
pub const FADE_STEPS: u32 = MORPH_STEPS;

/// 一次开合的状态机 + 定时补间。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DropdownMotion {
    /// 目标态：true = 展开。
    open: bool,
    /// 本次补间的起点（目标翻转时取当时的 `value`）。
    from: f32,
    /// 当前进度 0..=1。
    value: f32,
    /// 本次补间已走的时间。
    elapsed_ms: f32,
}

impl DropdownMotion {
    /// 当前进度（0 = 完全收起，1 = 完全展开）。
    pub const fn value(self) -> f32 {
        self.value
    }

    /// 是否还需要下一帧。宿主用它决定继续刷新。
    pub const fn is_animating(self) -> bool {
        // 两个条件缺一不可：
        // - `value` 还没到目标（正常在跑）；
        // - 本次补间的起点与目标不同、且时间没走完 —— 覆盖「刚翻转目标、
        //   但这一帧 dt 为 0」的情形：画面没动，但下一帧必须继续要。
        //
        // 只看 `elapsed_ms < duration` 是不够的：`Default` 的 `from == value == 0`
        // 与目标一致，启动后的头 180 ms 会被误报成「还在动」，宿主白白空转。
        let target = if self.open { 1.0 } else { 0.0 };
        let d = self.value - target;
        let started = self.from - target;
        (d > 0.001 || d < -0.001)
            || ((started > 0.001 || started < -0.001)
                && self.elapsed_ms < Self::duration_ms(self.open))
    }

    /// 推进一帧。`open` 是目标态，`dt_ms` 是宿主给的帧间隔。
    ///
    /// 非有限/非正的 `dt_ms` 不推进（返回后宿主可以照常重绘）；
    /// 目标翻转时从当前值重新起跑，所以连续快速点击不会产生跳变。
    pub fn tick(&mut self, open: bool, dt_ms: f32) {
        if open != self.open {
            self.open = open;
            self.from = self.value;
            self.elapsed_ms = 0.0;
        }

        let target = if self.open { 1.0 } else { 0.0 };
        let dur = Self::duration_ms(self.open);
        if self.elapsed_ms >= dur && (self.value - target).abs() <= 0.001 {
            // 已经停稳：不重复求值，也不让 `elapsed_ms` 无限增长。
            self.value = target;
            return;
        }
        if !(dt_ms.is_finite() && dt_ms > 0.0) {
            return;
        }

        self.elapsed_ms = (self.elapsed_ms + dt_ms).min(dur);
        let p = progress(self.elapsed_ms, dur, Easing::AppleMenu);
        self.value = self.from + (target - self.from) * p;
        // 收尾夹到目标：曲线在 t=1 处本来就该到 1，浮点误差不该留下 0.9999。
        if (self.value - target).abs() <= 0.001 || self.elapsed_ms >= dur {
            self.value = target;
        }
        self.value = self.value.clamp(0.0, 1.0);
    }

    const fn duration_ms(open: bool) -> f32 {
        if open {
            OPEN_MS
        } else {
            CLOSE_MS
        }
    }
}

/// 形变档位对应的分类名，索引 = 档位 - 1。
///
/// `dd-morph-N` 在 `.zstyle` 里同时驱动 `width`/`height`/`padding`/`corner-radius`
/// 与 `opacity`，把「原方块扩大成菜单」落成离散外观。曲线已由
/// [`DropdownMotion`] 承载；档位只负责量化。
pub const MORPH_CLASSES: [&str; MORPH_STEPS as usize] = [
    "dd-morph-1",
    "dd-morph-2",
    "dd-morph-3",
    "dd-morph-4",
    "dd-morph-5",
    "dd-morph-6",
    "dd-morph-7",
    "dd-morph-8",
    "dd-morph-9",
    "dd-morph-10",
    "dd-morph-11",
    "dd-morph-12",
    "dd-morph-13",
    "dd-morph-14",
    "dd-morph-15",
    "dd-morph-16",
    "dd-morph-17",
    "dd-morph-18",
    "dd-morph-19",
    "dd-morph-20",
    "dd-morph-21",
    "dd-morph-22",
    "dd-morph-23",
    "dd-morph-24",
    "dd-morph-25",
    "dd-morph-26",
    "dd-morph-27",
    "dd-morph-28",
    "dd-morph-29",
    "dd-morph-30",
    "dd-morph-31",
    "dd-morph-32",
    "dd-morph-33",
    "dd-morph-34",
    "dd-morph-35",
    "dd-morph-36",
    "dd-morph-37",
    "dd-morph-38",
    "dd-morph-39",
    "dd-morph-40",
    "dd-morph-41",
    "dd-morph-42",
    "dd-morph-43",
    "dd-morph-44",
    "dd-morph-45",
    "dd-morph-46",
    "dd-morph-47",
    "dd-morph-48",
    "dd-morph-49",
    "dd-morph-50",
    "dd-morph-51",
    "dd-morph-52",
    "dd-morph-53",
    "dd-morph-54",
    "dd-morph-55",
    "dd-morph-56",
    "dd-morph-57",
    "dd-morph-58",
    "dd-morph-59",
    "dd-morph-60",
];

/// 兼容旧名：`dd-fade-N` 语义并入 `dd-morph-N`。
pub const FADE_CLASSES: [&str; FADE_STEPS as usize] = MORPH_CLASSES;

/// 进度 → 当前形变分类名（0 进度返回空串）。
pub fn morph_class(value: f32) -> &'static str {
    match zephyra_style::anim::step_with(value, MORPH_STEPS) {
        Some(n) => MORPH_CLASSES[(n - 1) as usize],
        None => "",
    }
}

/// 兼容旧名。
pub fn fade_class(value: f32) -> &'static str {
    morph_class(value)
}

/// 菜单项揭示阈值：盒子长到一定程度再显示内容，避免文字从小方块里溢出。
///
/// 开合双向对称：低于阈值显示按钮头（`dd-head`），高于阈值显示菜单项。
/// 这样关闭时盒子缩小时会先切回按钮头，避免菜单项从小方块溢出。
pub const REVEAL_AT: f32 = 0.55;

/// 进度 → 是否揭示菜单项。
pub fn reveal_items(value: f32, _open: bool) -> bool {
    value >= REVEAL_AT
}

/// 进度 → 菜单整体可见（用于 `open` 形态/绝对定位）。
pub fn visible(value: f32) -> bool {
    value > 0.001
}

pub const BTN_H: f32 = 36.0;
pub const MENU_H: f32 = 114.0;

/// 当前形变档位对应的高度（与 `dd-morph-N` 规则一致），用于配合 translate 锚定。
pub fn morph_height(value: f32) -> f32 {
    match zephyra_style::anim::step_with(value, MORPH_STEPS) {
        Some(n) => BTN_H + (MENU_H - BTN_H) * (n as f32 / MORPH_STEPS as f32),
        None => BTN_H,
    }
}

/// 菜单覆盖在按钮上展开：不额外上移，随高度从按钮顶边向下形变。
pub fn glass_anchor_ty(value: f32) -> f32 {
    let _ = value;
    0.0
}

/// 兼容旧 API。
pub const MENU_SLIDE: f32 = 0.0;

pub fn visible_slide(_value: f32) -> f32 {
    0.0
}
