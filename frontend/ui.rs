//! 应用结构的声明式来源：`frontend/shell.zypui`（+ 控件组件库）的运行时入口。
//!
//! # 两条路，一份 IR（命门）
//!
//! - **release**：build.rs 用 zypuic 把 `frontend/shell.zypui` 与控件组件库
//!   一起编译成二进制，这里 `include_bytes!` + `decode` —— 运行时**零解析**。
//! - **dev**：`zypui_compile::parse_all` 直读源文件（parity 测试在用）。
//!
//! 两条路汇进同一份 [`zypui::ir::RawUi`]，实例化只有一份实现。
//!
//! # 组件只是名字
//!
//! shell.zypui 里的组件（PageGuide / Section / …）是模板：`<PageGuide/>`
//! 展开成裸元素树。Rust 侧不再有同名页面函数 —— 结构的真相只有这一份。

use std::sync::{Mutex, OnceLock};

use zypui::{Interner, Ui, UiRuntime};

static SHELL: OnceLock<Ui> = OnceLock::new();
static SHELL_RT: OnceLock<Mutex<UiRuntime>> = OnceLock::new();
/// 表达式求值的字符串去重器（动态 key / class 的 `&'static` 来源）。
///
/// 跨帧复用：每帧新建的话，同一个动态串每帧都会被 intern 成新的 'static
/// —— 那正是本仓每帧泄漏纪律要消灭的形状。UI 单线程，锁只在同帧内竞争。
static INTERNER: OnceLock<Mutex<Interner>> = OnceLock::new();

fn load_raw() -> zypui::RawUi {
    let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shell.zypui.bin"));
    zypui::decode::decode(bytes).expect("shell.zypui.bin 解码失败（产物错位？）")
}

/// 应用 UI（外壳 + 页面 + 控件组件，进程内一份，跨帧复用）。
pub fn shell() -> &'static Ui {
    SHELL.get_or_init(|| Ui::new(load_raw()).expect("组件重名"))
}

/// P1 per-element 运行时（build cache）。与 `shell()` 共享同源 IR 形态，但各自持有实例。
pub fn shell_runtime() -> &'static Mutex<UiRuntime> {
    SHELL_RT.get_or_init(|| Mutex::new(UiRuntime::new(load_raw()).expect("组件重名")))
}

/// 每帧一棵树：`Window` 页实例化（P1 per-element cache 路径）。
pub fn window<H: zypui::Host>(host: &H) -> zephyra_core::element::Element {
    let mut it = INTERNER
        .get_or_init(|| Mutex::new(Interner::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut rt = shell_runtime().lock().unwrap_or_else(|e| e.into_inner());
    let root = rt
        .resolve_page("Window", host, &mut it)
        .expect("Window 页实例化失败");
    (*root).clone()
}

/// Oracle 路径：全量构建（等价性测试/对照用）。
pub fn window_oracle<H: zypui::Host>(host: &H) -> zephyra_core::element::Element {
    let mut it = INTERNER
        .get_or_init(|| Mutex::new(Interner::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut els = shell()
        .build("Window", host, &mut it)
        .expect("Window 页实例化失败");
    assert!(
        els.len() == 1,
        "Window 页应恰好产出 1 个根元素，实际 {}",
        els.len()
    );
    els.remove(0)
}
