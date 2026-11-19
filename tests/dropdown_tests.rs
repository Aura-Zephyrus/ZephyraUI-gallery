//! 下拉按钮（同一控件变形为菜单）的「进去 / 返回」回归。
//!
//! 两条路径分开钉：
//!
//! - **选项 → 收起**：Up 的 click 分派里用前缀认 `liquid:dd:*:opt*`；
//! - **点外面 → 收起**：空白处没有可交互目标，router 不产生 Click，只能靠
//!   Down + `hovered` 判归属。只测前者会漏掉「点外面关不掉」。

use zephyra_fx::{InputState, PointerInput, PointerInputPhase, PointerSource, View};

use zephyra_gallery::app::motion::{self, DropdownMotion, CLOSE_MS, FADE_STEPS, OPEN_MS};
use zephyra_gallery::app::{Gallery, LIQUID_DD_GLASS_KEY, LIQUID_DD_SOLID_KEY};

fn down_with_hover(hovered: Option<&'static str>) -> InputState {
    InputState {
        hovered: hovered.map(Into::into),
        pointer_events: vec![PointerInput {
            id: 0,
            x: 900.0,
            y: 700.0,
            phase: PointerInputPhase::Down,
            source: PointerSource::Mouse,
        }],
        ..InputState::default()
    }
}

#[test]
fn choosing_an_option_closes_the_menu_and_records_the_action() {
    let mut g = Gallery::default();
    g.click(LIQUID_DD_SOLID_KEY);
    assert!(g.liquid_dd_solid_open, "点击本体应展开");

    g.click("liquid:dd:solid:opt2");
    assert!(!g.liquid_dd_solid_open, "选中菜单项后必须收起");
    assert_eq!(g.last_action, "liquid-dd-solid");
}

#[test]
fn outside_down_closes_the_menu_without_a_click_target() {
    let mut g = Gallery::default();
    g.click(LIQUID_DD_SOLID_KEY);
    assert!(g.liquid_dd_solid_open);

    // 空白处：hovered = None，因此 router 不会产出 Click。
    g.on_input(&down_with_hover(None));
    assert!(!g.liquid_dd_solid_open, "按下空白处必须收起");
}

#[test]
fn down_on_the_dropdown_itself_keeps_it_open() {
    let mut g = Gallery::default();
    g.click(LIQUID_DD_SOLID_KEY);
    assert!(g.liquid_dd_solid_open);

    g.on_input(&down_with_hover(Some(LIQUID_DD_SOLID_KEY)));
    assert!(g.liquid_dd_solid_open, "按在本体上不该把菜单收掉");
}

#[test]
fn opening_one_dropdown_closes_the_other() {
    let mut g = Gallery::default();
    g.click(LIQUID_DD_SOLID_KEY);
    g.click(LIQUID_DD_GLASS_KEY);
    assert!(!g.liquid_dd_solid_open);
    assert!(g.liquid_dd_glass_open);

    g.click(LIQUID_DD_SOLID_KEY);
    assert!(g.liquid_dd_solid_open);
    assert!(!g.liquid_dd_glass_open);
}

// ------------------------------------------------------------ 开合动画本身

/// 静止态不能要求宿主继续刷新。
///
/// 旧实现只看「时间没走完」会在这里误报：`Default` 的进度与目标都是 0，
/// 但 `elapsed_ms < 180` 仍然成立，启动后的头 180 ms 白白空转。
#[test]
fn fresh_motion_is_idle() {
    let m = DropdownMotion::default();
    assert_eq!(m.value(), 0.0);
    assert!(!m.is_animating(), "静止态不该要求下一帧");
}

/// 打开必须单调推进、在 `OPEN_MS` 后精确停在 1，并且中途不空跳。
#[test]
fn opening_advances_monotonically_and_settles_at_one() {
    let mut m = DropdownMotion::default();
    m.tick(true, 16.0);
    assert!(
        m.value() > 0.0 && m.value() < 1.0,
        "第一帧应在中途，实际 {}",
        m.value()
    );

    let mut last = m.value();
    // 多走两帧，确保「超过时长」也停在 1 而不是越界。
    let frames = (OPEN_MS / 16.0).ceil() as u32 + 2;
    for i in 0..frames {
        m.tick(true, 16.0);
        assert!(
            m.value() >= last,
            "打开进度倒退了（第 {i} 帧）：{last} -> {}",
            m.value()
        );
        assert!(m.value() <= 1.0);
        last = m.value();
    }
    assert_eq!(m.value(), 1.0, "打开结束必须精确到位");
    assert!(!m.is_animating(), "停稳后不该继续要帧");
}

/// 中途反向：从**当前值**重新起跑，不回到 1 也不跳到 0。
#[test]
fn reversing_mid_flight_rebases_without_jumping() {
    let mut m = DropdownMotion::default();
    for _ in 0..6 {
        m.tick(true, 16.0);
    }
    let mid = m.value();
    assert!(mid > 0.0 && mid < 1.0, "6 帧后应在中途，实际 {mid}");

    // 目标翻转的那一帧宿主可能给 dt = 0：位置必须原地不动，但仍然要帧。
    m.tick(false, 0.0);
    assert!(
        (m.value() - mid).abs() < 1e-6,
        "翻转目标不许产生跳变：{mid} -> {}",
        m.value()
    );
    assert!(m.is_animating(), "刚翻转目标（dt=0）也必须继续要帧");

    let mut last = m.value();
    let frames = (CLOSE_MS / 16.0).ceil() as u32 + 2;
    for i in 0..frames {
        m.tick(false, 16.0);
        assert!(
            m.value() <= last,
            "关闭进度回升了（第 {i} 帧）：{last} -> {}",
            m.value()
        );
        last = m.value();
    }
    assert_eq!(m.value(), 0.0, "关闭结束必须精确收回");
    assert!(!m.is_animating(), "收稳后不该继续要帧");
}

/// 非法 dt 不推进，但也不破坏「下次还能正常跑」的状态。
#[test]
fn non_finite_dt_does_not_advance() {
    let mut m = DropdownMotion::default();
    m.tick(true, 16.0);
    let before = m.value();
    for bad in [0.0, -16.0, f32::NAN, f32::INFINITY] {
        m.tick(true, bad);
        assert_eq!(m.value(), before, "dt={bad} 不该推进");
    }
    m.tick(true, 16.0);
    assert!(m.value() > before, "非法帧之后仍要能继续推进");
}

/// 16 档形变分类名与进度一一对应，0 进度不发分类名。
///
/// 「原方块扩大变成菜单」的几何/透明度值住在 `.zstyle` 的 `dd-morph-N`；
/// 这里只钉住「进度 → 档位名」的映射，值本身由样式门禁+真机看。
#[test]
fn morph_class_covers_every_step() {
    assert_eq!(motion::morph_class(0.0), "");
    assert_eq!(motion::morph_class(-1.0), "");
    assert_eq!(motion::morph_class(f32::NAN), "");
    assert_eq!(motion::morph_class(1.0), "dd-morph-60", "完成态也要有规则");
    assert_eq!(motion::MORPH_CLASSES.len(), motion::MORPH_STEPS as usize);
    assert_eq!(motion::fade_class(1.0), "dd-morph-60", "兼容别名");

    let half_step = 0.5 / FADE_STEPS as f32;
    for i in 1..=FADE_STEPS {
        let p = i as f32 / FADE_STEPS as f32;
        assert_eq!(
            motion::morph_class(p - half_step),
            motion::MORPH_CLASSES[(i - 1) as usize],
            "第 {i} 档"
        );
    }
}

/// 形变用盒子几何表达扩张，不额外平移按钮（兼容 API 保持静止）。
#[test]
fn morph_uses_geometry_not_slide() {
    assert_eq!(motion::visible_slide(0.0), 0.0);
    assert_eq!(motion::visible_slide(0.5), 0.0);
    assert_eq!(motion::visible_slide(1.0), 0.0);
    assert!(motion::visible(0.01));
    assert!(!motion::visible(0.0));
}

/// 菜单项在盒子有足够空间后再揭示，开合对称，避免小方块溢出。
#[test]
fn reveal_waits_for_room_and_is_symmetric() {
    assert!(!motion::reveal_items(0.0, true));
    assert!(!motion::reveal_items(0.5, true));
    assert!(motion::reveal_items(0.6, true));
    assert!(!motion::reveal_items(0.5, false), "关闭缩小时切回按钮头");
    assert!(motion::reveal_items(0.6, false));
    assert!(!motion::reveal_items(0.0, false));
}

/// 打开菜单不得改动**其它**行的布局。
///
/// 展开时控件变成 `position: absolute` 的菜单、脱离文档流；若 `demo-slot`
/// 没有自己的尺寸，它会缩成 0×0，下一行（玻璃下拉）在打开第一帧上跳。
/// `.dd-slot` 保留 72×36（收起态按钮外盒）就是为这条判据。
#[test]
fn opening_the_menu_does_not_move_the_glass_dropdown() {
    use std::sync::Arc;

    use zephyra_core::context::Context;
    use zephyra_core::core::Rect;
    use zephyra_core::frame::FrameCache;
    use zephyra_core::node::Node;
    use zephyra_core::theme::{Theme, ThemeVariant};
    use zephyra_style::ir::StyleSheet;
    use zephyra_style::SheetStyler;

    fn merged() -> StyleSheet {
        let mut sheet = StyleSheet::new();
        for (name, css) in zephyra_gallery::styles::all() {
            let parsed = zstyle_compile::parse(name, css);
            assert!(
                parsed.is_ok(),
                "{name}.zstyle must parse: {:?}",
                parsed.errors
            );
            sheet = sheet.merge(parsed.sheet);
        }
        sheet
    }

    fn rect_of(node: &Node, key: &str) -> Option<Rect> {
        if node.key().and_then(|k| k.as_name()) == Some(key) {
            return Some(node.rect());
        }
        node.children().iter().find_map(|c| rect_of(c, key))
    }

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let bounds = (1400.0, 800.0);

    let mut cache = FrameCache::new();
    cache.set_styler(Box::new(SheetStyler::new(Arc::new(merged()))));

    let mut g = Gallery::default();
    g.route = "reference";
    // 宿主每帧都会推视口；测试不推的话 `is_compact()` 会按默认（0,0）走紧凑布局，
    // 两次测量就不在同一个坐标系里。
    let host_frame = |dt_ms: f32| InputState {
        dt_ms,
        viewport: (1400.0, 800.0),
        scale: 1.0,
        ..InputState::default()
    };
    g.on_input(&host_frame(0.0));
    let _ = cache.frame(&g.view(&ctx), ctx, bounds);
    let closed = rect_of(cache.node().expect("首帧应有树"), LIQUID_DD_GLASS_KEY)
        .expect("玻璃下拉必须在树上");

    g.click(LIQUID_DD_SOLID_KEY);
    g.on_input(&host_frame(16.0));
    let _ = cache.frame(&g.view(&ctx), ctx, bounds);
    let animating = rect_of(cache.node().expect("第二帧应有树"), LIQUID_DD_GLASS_KEY)
        .expect("玻璃下拉在展开动画中也必须在树上");

    assert_eq!(
        (closed.x, closed.y),
        (animating.x, animating.y),
        "展开动画第一帧玻璃下拉位移了：{:?} -> {:?} —— slot 没保住尺寸",
        (closed.x, closed.y),
        (animating.x, animating.y)
    );
    assert_eq!(
        (animating.width, animating.height),
        (closed.width, closed.height),
        "玻璃下拉自身尺寸不该被别的行影响"
    );
}

/// 形变必须把**左上角钉在原按钮左上角**，并且只向右/向下长。
///
/// 只测「别的行不动」不够：`demo-slot` 的 `cross-align: center` 会把
/// 36px 槽里的 114px 菜单上下均分，顶边比按钮高 39px —— 看起来就是
/// 「菜单从按钮上方冒出来」。这条逐帧读控件 rect，钉住 x/y 不变。
#[test]
fn morph_keeps_the_button_top_left_corner_fixed() {
    use std::sync::Arc;

    use zephyra_core::context::Context;
    use zephyra_core::core::Rect;
    use zephyra_core::frame::FrameCache;
    use zephyra_core::node::Node;
    use zephyra_core::theme::{Theme, ThemeVariant};
    use zephyra_style::ir::StyleSheet;
    use zephyra_style::SheetStyler;

    fn rect_of(node: &Node, key: &str) -> Option<Rect> {
        if node.key().and_then(|k| k.as_name()) == Some(key) {
            return Some(node.rect());
        }
        node.children().iter().find_map(|c| rect_of(c, key))
    }

    let mut sheet = StyleSheet::new();
    for (name, css) in zephyra_gallery::styles::all() {
        sheet = sheet.merge(zstyle_compile::parse(name, css).sheet);
    }
    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let mut cache = FrameCache::new();
    cache.set_styler(Box::new(SheetStyler::new(Arc::new(sheet))));

    let mut g = Gallery::default();
    g.route = "reference";
    let host_frame = |dt_ms: f32| InputState {
        dt_ms,
        viewport: (1400.0, 800.0),
        scale: 1.0,
        ..InputState::default()
    };
    g.on_input(&host_frame(0.0));
    let _ = cache.frame(&g.view(&ctx), ctx, (1400.0, 800.0));
    let closed = rect_of(cache.node().expect("首帧应有树"), LIQUID_DD_SOLID_KEY)
        .expect("实心下拉必须在树上");

    g.click(LIQUID_DD_SOLID_KEY);
    let mut last_w = closed.width;
    for frame in 0..20 {
        g.on_input(&host_frame(16.0));
        let _ = cache.frame(&g.view(&ctx), ctx, (1400.0, 800.0));
        let now = rect_of(cache.node().expect("动画中应有树"), LIQUID_DD_SOLID_KEY)
            .expect("实心下拉在形变中也必须在树上");
        assert_eq!(
            (now.x, now.y),
            (closed.x, closed.y),
            "第 {frame} 帧左上角偏移了：按钮 ({}, {}) -> 菜单 ({}, {})",
            closed.x,
            closed.y,
            now.x,
            now.y
        );
        assert!(
            now.width + 0.01 >= last_w,
            "第 {frame} 帧宽度倒退了：{last_w} -> {}",
            now.width
        );
        last_w = now.width;
    }
    assert!(
        (last_w - 220.0).abs() < 0.5,
        "20 帧 × 16 ms 后应接近完整菜单宽度 220，实际 {last_w}"
    );
}

/// 真实宿主路径：点击后按帧推进，`on_input` 的时钟必须真的驱动补间。
///
/// 直接测 `DropdownMotion` 只能证明算法正确；这一条证明 `on_input` 里的
/// 每帧 `tick` 真的接上了 —— 漏掉那两行，菜单仍会「唰」地出现。
#[test]
fn host_clock_drives_the_menu_motion() {
    let mut g = Gallery::default();
    g.click(LIQUID_DD_SOLID_KEY);

    let frame = |dt_ms: f32| InputState {
        dt_ms,
        ..InputState::default()
    };

    g.on_input(&frame(16.0));
    let after_one = g.liquid_dd_solid_motion.value();
    assert!(
        after_one > 0.0 && after_one < 1.0,
        "点击后第一帧应在中途，实际 {after_one}"
    );

    // 30 帧 × 16 ms = 480 ms > OPEN_MS(240 ms)。
    for _ in 0..30 {
        g.on_input(&frame(16.0));
    }
    assert_eq!(g.liquid_dd_solid_motion.value(), 1.0);
    assert!(!g.liquid_dd_solid_motion.is_animating());
}
