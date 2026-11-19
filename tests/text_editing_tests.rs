//! 文本编辑指针语义（Chrome 对齐）：单击定位 / 拖选 / 双击选词 / 三击全选 / 溢出裁剪。
//!
//! 无 GPU、无窗口：直接喂 `InputState`（含 rects + pointer_events）给 `Gallery::on_input`。

use zephyra_core::core::Rect;
use zephyra_core::id::ElementKey;
use zephyra_fx::{InputState, PointerInput, PointerInputPhase, View};

use zephyra_core::input::PointerSource;
use zephyra_gallery::app::Gallery;

const FIELD: (&str, f32, f32, f32, f32) = ("text:field", 436.0, 202.0, 240.0, 36.0);

fn field_rect() -> Rect {
    Rect::new(FIELD.1, FIELD.2, FIELD.3, FIELD.4)
}

/// 把点击 x（字段内局部像素）与阶段送进 on_input。
fn pointer_state(phase: PointerInputPhase, x: f32) -> InputState {
    InputState {
        rects: vec![(ElementKey::Name(FIELD.0), field_rect())],
        pointer_events: vec![PointerInput {
            id: 0,
            x: FIELD.1 + 10.0 + x, // +10 = padding
            y: FIELD.2 + 18.0,
            phase,
            source: PointerSource::Mouse,
        }],
        dt_ms: 16.0,
        scale: 1.0,
        ..Default::default()
    }
}

/// "hello" 的 advance 表：每个字母 7.2px（测试专用，避免依赖字体）。
fn gallery_with_hello() -> Gallery {
    let mut g = Gallery::default();
    g.route = "textinput";
    g.text_field.set_text("hello");
    *g.text_advances.borrow_mut() = vec![
        (0, 0.0),
        (1, 7.2),
        (2, 14.4),
        (3, 21.6),
        (4, 28.8),
        (5, 36.0),
    ];
    g
}

#[test]
fn click_places_caret_at_glyph_midpoint() {
    let mut g = gallery_with_hello();
    // 点击 16px：落在第 2 个字形中点（14.4..21.6 中点 18）左侧 ⇒ 光标在字节 2。
    g.on_input(&pointer_state(PointerInputPhase::Down, 16.0));
    assert_eq!(g.text_field.cursor(), 2);
    assert!(g.text_field.selection().is_none(), "单击应清掉选区");
}

#[test]
fn dragging_selects_from_anchor_to_current() {
    let mut g = gallery_with_hello();
    g.on_input(&pointer_state(PointerInputPhase::Down, 2.0)); // anchor 字节 0
    let mut move_state = pointer_state(PointerInputPhase::Move, 30.0); // 字节 4
    g.on_input(&move_state);
    assert_eq!(g.text_field.selection(), Some((0, 4)));
    // 拖回锚点：选区收拢为空（TextBuffer 把 begin==end 归一化为 None）。
    move_state.pointer_events[0].x = FIELD.1 + 10.0 + 1.0;
    g.on_input(&move_state);
    assert!(g.text_field.selection().is_none());
    g.on_input(&pointer_state(PointerInputPhase::Up, 1.0));
    assert!(g.text_drag_anchor.is_none());
}

#[test]
fn double_click_selects_word_and_triple_click_selects_all() {
    let mut g = gallery_with_hello();
    // 第一击：光标就位；第二击（500ms 内）：选词。
    g.on_input(&pointer_state(PointerInputPhase::Down, 16.0));
    g.on_input(&pointer_state(PointerInputPhase::Up, 16.0));
    g.on_input(&pointer_state(PointerInputPhase::Down, 16.0));
    assert_eq!(g.text_field.selection(), Some((0, 5)), "双击应选中 hello");
    // 第三击：全选（hello 本身即全部，换更长的词验证）。
    g.on_input(&pointer_state(PointerInputPhase::Up, 16.0));
    g.text_field.set_text("hello world");
    *g.text_advances.borrow_mut() = (0..=11).map(|i| (i, i as f32 * 7.2)).collect();
    g.on_input(&pointer_state(PointerInputPhase::Down, 16.0));
    assert_eq!(g.text_field.selection(), Some((0, 11)), "三击应全选");
}

#[test]
fn click_position_then_type_inserts_between_characters() {
    let mut g = gallery_with_hello();
    g.on_input(&pointer_state(PointerInputPhase::Down, 16.0)); // 光标字节 2
    g.on_input(&pointer_state(PointerInputPhase::Up, 16.0));
    // 焦点必须在 text:field 才会分发文本事件。
    use zephyra_core::input::text::TextEvent;
    let state = InputState {
        active_focus: Some(ElementKey::Name(FIELD.0)),
        text_events: vec![TextEvent::Char('X')],
        ..Default::default()
    };
    g.on_input(&state);
    assert_eq!(g.text_field.text(), "heXllo", "应插在两个字符之间");
    assert_eq!(state.text_events.len(), 1);
}
