//! 构建期：把 `frontend/shell.zypui`（引用 `ZephyraUI/ui/controls.zypui` 的组件）
//! 编译成一份二进制（OUT_DIR），运行时零解析。
//!
//! 两个文件**一起**编译成一个工件：shell.zypui 的 `<Button/>` 等组件引用
//! 与 controls.zypui 的定义必须在同一张组件表里解析。运行时侧
//! `frontend/ui.rs` 的 `include_bytes!` 与这里的产物一一对应。

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // 组件库（控件层）与应用外壳。组件库属于 ZephyraUI crate —— 结构的
    // 真相只有一份，gallery 不复制它。
    let controls = manifest.join("../ZephyraUI/ZephyraUI/ui/controls.zypui");
    let shell = manifest.join("frontend/shell.zypui");
    println!("cargo:rerun-if-changed={}", controls.display());
    println!("cargo:rerun-if-changed={}", shell.display());

    let controls_src = std::fs::read_to_string(&controls).expect("读 ZephyraUI/ui/controls.zypui");
    let shell_src = std::fs::read_to_string(&shell).expect("读 frontend/shell.zypui");
    // 名字泄漏成 'static：zypuic 的诊断/IR 全程 &'static。
    let controls_name: &'static str = Box::leak(controls.display().to_string().into_boxed_str());
    let shell_name: &'static str = Box::leak(shell.display().to_string().into_boxed_str());

    let bytes = match zypui_compile::compile(&[
        (controls_name, controls_src.as_str()),
        (shell_name, shell_src.as_str()),
    ]) {
        Ok(bytes) => bytes,
        Err(diags) => {
            let mut msg = String::from("frontend/*.zypui 编译失败：\n");
            for d in &diags {
                msg.push_str(&d.render());
                msg.push('\n');
            }
            panic!("{msg}");
        }
    };

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("shell.zypui.bin");
    std::fs::write(&out, &bytes).expect("写编译产物");
}
