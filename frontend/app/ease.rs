//! 预计算缓动查表。
//!
//! 每帧动态求三次贝塞尔需要牛顿迭代（`Bezier::at` 里最多 8 次迭代、带
//! `powf` 相关运算），对「每帧都要推进位移」的控件来说是浪费，也是
//! 手感「卡」的来源之一。本模块把曲线在 `[0, dur_ms]` 上等间距采样一次，
//! 运行时按 `elapsed_ms` 索引表 + 线性插值，**零迭代、零 `powf`**。
//!
//! 开关与切换按钮组共用 `Easing::Direct`（`cubic-bezier(0,0,0,1)`，先快后慢，
//! `t=0.5` 处 ≈ 0.89），两者视觉一致。

use std::sync::OnceLock;
use zephyra_style::anim::Easing;

/// 预计算的缓动曲线查表。
pub struct EaseTable {
    /// 表长（点数），≥ 2。
    len: usize,
    /// 表对应的总时长（ms）。
    dur_ms: f32,
    /// 采样值（0..=1 的缓动进度），`table[0] == 0.0`，`table[len-1] == 1.0`。
    table: Box<[f32]>,
}

impl EaseTable {
    /// 用 `easing` 在 `[0, dur_ms]` 上等间距采样 `samples` 点建表。
    pub fn new(dur_ms: f32, easing: Easing, samples: usize) -> Self {
        assert!(dur_ms > 0.0 && samples >= 2);
        let len = samples;
        let mut table = Vec::with_capacity(len);
        for i in 0..len {
            let t = if len == 1 {
                0.0
            } else {
                i as f32 / (len - 1) as f32
            };
            let eased = easing.at(t).clamp(0.0, 1.0);
            table.push(eased);
        }
        Self {
            len,
            dur_ms,
            table: table.into_boxed_slice(),
        }
    }

    /// 查表：`elapsed_ms` 会被夹到 `[0, dur_ms]`，返回 0..=1 的缓动进度。
    /// 相邻采样点之间做线性插值，比直接取整更平滑。
    pub fn progress(&self, elapsed_ms: f32) -> f32 {
        if elapsed_ms <= 0.0 {
            return 0.0;
        }
        if elapsed_ms >= self.dur_ms {
            return 1.0;
        }
        let t = (elapsed_ms / self.dur_ms).clamp(0.0, 1.0);
        let idx_f = t * (self.len - 1) as f32;
        let idx0 = idx_f.floor() as usize;
        let idx1 = (idx0 + 1).min(self.len - 1);
        let frac = idx_f - idx0 as f32;
        self.table[idx0] * (1.0 - frac) + self.table[idx1] * frac
    }
}

/// 250 ms「快入」缓动表（`cubic-bezier(0,0,0,1)`，即 `Easing::Direct`）。
///
/// 每毫秒一个采样点（251 点），表建一次，运行时只做索引+线性插值。
static EASE_DIRECT_250: OnceLock<EaseTable> = OnceLock::new();

/// 250 ms「快入」预计算缓动表的单例。
pub fn ease_direct_250() -> &'static EaseTable {
    EASE_DIRECT_250.get_or_init(|| EaseTable::new(250.0, Easing::Direct, 251))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_table_start_and_end_are_exact() {
        let t = EaseTable::new(250.0, Easing::Direct, 9);
        assert!((t.progress(0.0) - 0.0).abs() < 1e-6);
        assert!((t.progress(250.0) - 1.0).abs() < 1e-6);
        assert!((t.progress(500.0) - 1.0).abs() < 1e-6);
        assert!((t.progress(-100.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn ease_table_is_monotonic() {
        let t = EaseTable::new(250.0, Easing::Direct, 251);
        let mut prev = -1.0f32;
        for i in 0..t.len {
            let ms = i as f32 / (t.len - 1) as f32 * 250.0;
            let v = t.progress(ms);
            assert!(v >= prev - 1e-6, "非单调 at {ms}ms: {prev} -> {v}");
            prev = v;
        }
    }
}
