//! 树级门禁。全部按**整棵树**判，不按函数判 —— 逐函数查只能查到
//! 「我记得去查的那个」。（与早期那条逐函数查的门禁同一理由。）
//!
//! # 恒真防御
//!
//! 每条涉及遍历的断言都先确认「树不是空的」（节点数下界），否则树造失败时
//! 断言会**恒真**地绿 —— 这个项目已经因为恒真式吃过两次亏。

use zephyra_core::context::Context;
use zephyra_core::core::Rect;
use zephyra_core::id::NodeTag;
use zephyra_core::node::Node;
use zephyra_core::theme::{Theme, ThemeVariant};
use zephyra_fx::View;
use zephyra_style::ir::StyleSheet;

use zephyra_gallery::app::pages::PAGES;
use zephyra_gallery::app::{Gallery, BASE_COLUMNS, REF_FAMILIES, ROUTES, SWITCH_DEMO_KEY};
use zephyra_ui::tokens::ALL as TOKENS;

// ---------------------------------------------------------------- 公共帮手

/// 合并全部样式表 —— **直接用 [`zephyra_gallery::styles::all`]**，
/// 与 `src/bin/main.rs::build_sheet` 同一份输入。
///
/// # 为什么不再 `read_dir`
///
/// 旧版本扫 crate 目录。控件层拆出去之后那条路有两个问题：一是它只能看见
/// gallery 自己的表，看不见控件层的；二是「图对、门禁绿」要求门禁吃**同一份**
/// 表，而现在那份表是 `include_str!` 内嵌的 —— 门禁再去磁盘读一遍就成了
/// **两次真相**：改了 `styles.rs` 忘了重编，门禁照样绿。
///
/// 走同一个函数就没有这个问题：内嵌的源是唯一定义处。
fn merged_sheet() -> StyleSheet {
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

/// 造一棵**应用了样式表并布局过**的树。
///
/// # 文本引擎是真的（与 `--dump-cpu` 同口径）
///
/// 曾经用 `Context::new(&theme)`（`engine: None`）—— 文本按**兜底度量**排版，
/// 行高比真实字体小得多。于是「一屏装不下、最下面几行被裁掉」这类问题在
/// 门禁里**完全看不见**，而 `--dump-cpu` 的图上它就在那里：一次实测中
/// basics 族的色板在真实字体下掉出客户区 ~110px，测试里的几何却说它稳稳在框内。
///
/// 判据必须与出图同源，所以这里用 [`zephyra_fx::from_system_font`]（就是
/// `CpuHost` 用的那个）。取不到系统字体时它内部有兜底，不会 panic。
fn laid_out(route: &'static str) -> Node {
    laid_out_from(route, 0)
}

/// [`laid_out`] 的**带族**版本：路由 + 参考族（非 reference 路由时族无影响）。
fn laid_out_from(route: &'static str, family: usize) -> Node {
    laid_out_tweaked(route, |_| {}, family)
}

/// [`laid_out`] 的**可改初值**版本：先把 `tweak` 应用到 `Gallery` 再出树。
///
/// 用于「同一个路由、不同运行期状态」的门禁 —— 玻璃的便宜 / 物理两种算法
/// 各自长出**不同的面板**（两个旋钮 vs 四个），而两条都要钉。
fn laid_out_tweaked(route: &'static str, tweak: impl FnOnce(&mut Gallery), family: usize) -> Node {
    let theme = Theme::builtin(ThemeVariant::Dark);
    let (engine, _note) = zephyra_fx::from_system_font();
    let engine = std::cell::RefCell::new(engine);
    let ctx = Context::with_engine(&theme, &engine);
    let mut g = Gallery::default();
    g.route = route;
    g.ref_family = family;
    tweak(&mut g);
    let element = g.view(&ctx);
    let mut root = Node::from_element(&element);
    merged_sheet().apply_all(&mut root);
    zephyra_core::layout::layout(&mut root, Rect::new(0.0, 0.0, 1400.0, 800.0), ctx);
    root
}

/// 材质页的树 —— 一等路由 `materials`（材质专区与液态玻璃合并在一页）。
fn laid_out_materials() -> Node {
    laid_out("materials")
}

/// 材质页的树，玻璃强制走**物理折射**（面板上有四个旋钮）。
fn laid_out_materials_physical() -> Node {
    laid_out_tweaked("materials", |g| g.glass.mode = GlassMode::Physical, 0)
}

/// 覆盖「会改外观的状态」的门禁脚本表 —— `interaction_leaves_no_stale_appearance`
/// 与 `every_stylesheet_rule_matches_something` 共用。
///
/// # 为什么要提成两处共用
///
/// 后者要回答「样式表里每条规则都选得中东西吗」，而「有东西可被选中」依赖
/// **交互态**：`.pressed` 规则只有在按钮被按住时才选得中元素。若只跑默认状态的
/// 树，它会把所有 `.pressed` / `.checked` 规则误报成死规则 —— 于是唯一的选择
/// 就是「把死规则检查关掉」，而那正是这条检查要防的事。
///
/// ⚠ 往表里加脚本时同步问一句：新状态会不会带出新的 `.class` 规则？会的话
/// 不加进这里就会出现假阳性，然后有人会去**删规则**而不是加脚本 —— 那比没有
/// 这条门禁更坏。
type GateScripts = Vec<(&'static str, fn(&mut Gallery))>;

/// 见 [`GateScripts`]。
fn interaction_scripts() -> GateScripts {
    vec![
        ("切到 guide", |g| g.route = "guide"),
        // ⚠ 复选框组的状态**只在控件层的模型里**（`zephyra_ui::CheckboxGroup`），
        // 门禁脚本不能直接改数组 —— 那样就绕过了「组头从子项派生」这条规则，
        // 而那条规则正是本轮要钉的东西。
        ("勾上 alpha", |g| {
            g.check_group
                .set_item_state("check:alpha", zephyra_ui::CheckState::Checked);
        }),
        ("再勾 beta 与 gamma", |g| {
            g.check_group
                .set_item_state("check:beta", zephyra_ui::CheckState::Checked);
            g.check_group
                .set_item_state("check:gamma", zephyra_ui::CheckState::Checked);
        }),
        ("全部取消", |g| {
            for key in ["check:alpha", "check:beta", "check:gamma"] {
                g.check_group
                    .set_item_state(key, zephyra_ui::CheckState::Unchecked);
            }
        }),
        // 组头点一下 = 全选 / 全部不选。半选时点它 → 全选。
        ("组头全选", |g| g.check_group.toggle_all()),
        ("切到 reference", |g| g.route = "reference"),
        ("切到 styling", |g| g.route = "styling"),
        ("切到 layout", |g| g.route = "layout"),
        ("切回 guide", |g| g.route = "guide"),
        // ⚠ **每一族一个脚本**：族内容只在对应分段被选中时出现在树上，
        // 而 `every_stylesheet_rule_matches_something` 按全部脚本扫形状 ——
        // 少一个族的脚本，那一族的分类名（`.page-checkbox` 等）就会被误判成
        // 死规则，然后有人会去删规则。第一次就踩了这个坑。
        //
        // 2026-11-01 起参考页是三族（Button 族、复选框族与开关族）。
        ("参考页切到按钮族", |g| {
            g.route = "reference";
            g.ref_family = 0;
        }),
        ("参考页切到复选框族", |g| {
            g.route = "reference";
            g.ref_family = 1;
        }),
        ("参考页切到开关族", |g| {
            g.route = "reference";
            g.ref_family = 2;
        }),
        ("折射参数加一档", |g| {
            g.glass.nudge(zephyra_gallery::app::glass::Param::Ior, 1);
        }),
        ("折射参数减一档", |g| {
            g.glass
                .nudge(zephyra_gallery::app::glass::Param::Dispersion, -1);
        }),
    ]
}

/// 递归遍历：父先于子。
fn walk(node: &Node, f: &mut impl FnMut(&Node)) {
    f(node);
    for c in node.children() {
        walk(c, f);
    }
}

/// 数一幌之：确认树不是空的。
fn count_nodes(root: &Node) -> usize {
    let mut n = 0;
    walk(root, &mut |_| n += 1);
    n
}

/// 按 key 找节点（深度优先，第一个命中）。
fn find_by_key<'a>(root: &'a Node, key: &str) -> Option<&'a Node> {
    if root.key().and_then(|k| k.as_name()) == Some(key) {
        return Some(root);
    }
    for c in root.children() {
        if let Some(n) = find_by_key(c, key) {
            return Some(n);
        }
    }
    None
}

/// 内嵌的 `shell.zstyle` 源（不是磁盘）。
///
/// # 为什么必须有这个 helper 而不是直接 include_str!
///
/// 那两条门禁（`shell_metrics_match_stylesheet` / `root_container_writes_no_fixed_size`）
/// 要在**源码文本**里按行找 `height:` / `width:`，而不是走 StyleSheet —— 它们查的是
/// 「这几行字还在不在」，那需要文本，不是 IR。而文本现在内嵌在
/// `zephyra_gallery::styles` 里，所以从那里取，与运行时用的是同一份。
fn shell_css() -> &'static str {
    zephyra_gallery::styles::app_only()
        .iter()
        .find(|(n, _)| *n == "shell")
        .map(|(_, src)| *src)
        .expect("shell.zstyle 应当在内嵌样式表里")
}

// ------------------------------------------------------- 门禁 1：源码无外观

/// 源码里不许有样式 —— 全树 × 全部页面 × 全部交互态。
///
/// 判据是 `iter_local()`（Local 层）而不是 `lookup()`：后者会一路回落到
/// 样式值，看不出这一层有没有被源码写过。与 demo 同款。
#[test]
fn the_whole_tree_writes_no_appearance_in_source() {
    const APPEARANCE: &[&zephyra_core::property::PropertyKey] = &[
        &zephyra_core::props::WIDTH,
        &zephyra_core::props::HEIGHT,
        &zephyra_core::props::CORNER_RADIUS,
        &zephyra_core::props::PADDING,
        &zephyra_core::props::SPACING,
        &zephyra_core::props::DIRECTION,
        &zephyra_core::props::CROSS_ALIGN,
        &zephyra_core::props::MAIN_ALIGN,
        &zephyra_core::props::BACKGROUND,
        &zephyra_core::props::BORDER_COLOR,
        &zephyra_core::props::BORDER_WIDTH,
        &zephyra_core::props::TEXT_SIZE,
        &zephyra_core::props::TEXT_COLOR,
        &zephyra_core::props::TEXT_LINE_HEIGHT,
        &zephyra_core::props::TEXT_MAX_WIDTH,
        &zephyra_core::props::ICON_COLOR,
        &zephyra_core::props::IMAGE_SIZE,
        &zephyra_core::props::IMAGE_TINT,
        &zephyra_core::props::OPACITY,
    ];

    let routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();
    let mut seen_nodes = 0;
    for route in routes {
        // hover / pressed 各造一次：某些外观只在某个状态下才泄漏。
        for (hovered, pressed) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut g = Gallery::default();
            g.route = route;
            if hovered {
                g.hovered = Some(zephyra_core::id::ElementKey::from("guide"));
            }
            if pressed {
                g.pressed = Some(zephyra_core::id::ElementKey::from("guide"));
            }
            let theme = Theme::builtin(ThemeVariant::Dark);
            let ctx = Context::new(&theme);
            let element = g.view(&ctx);
            let mut root = Node::from_element(&element);
            merged_sheet().apply_all(&mut root);
            let mut offenders: Vec<String> = Vec::new();
            walk(&root, &mut |n: &Node| {
                seen_nodes += 1;
                for (key, _) in n.props().iter_local() {
                    if APPEARANCE.iter().any(|k| std::ptr::eq(*k, key)) {
                        // 唯一有形状的例外：`fill:*` 的填充尺寸是**内容**（见 lib.rs 第 3 条）。
                        // 横条（进度条/滑条/滚动 thumb）用 WIDTH 或 HEIGHT，都放行；
                        // 其余键（颜色/圆角/边…）一概不放行。
                        let is_fill = n
                            .key()
                            .and_then(|k| k.as_name())
                            .is_some_and(|k| k.starts_with("fill:"));
                        let is_fill_geometry = is_fill
                            && (std::ptr::eq(key, &zephyra_core::props::WIDTH)
                                || std::ptr::eq(key, &zephyra_core::props::HEIGHT));
                        // ² 第二个例外：把手与轨道的**逐帧插值色**（原版 LiquidToggle
                        //   `drawRect(lerp(...))`）。它们是绑定值（`track_bg` / `knob_bg` /
                        //   `knob_mini_color`），不是源码写入的外观 —— 与
                        //   `filter` / `decoration` 绑定同一条路（`zypui` 的 `background`
                        //   动态属性）。只放行 `track` / `knob*` key 上的 BACKGROUND，
                        //   其余外观键照旧拦。
                        let is_animated_color =
                            n.key().and_then(|k| k.as_name()).is_some_and(|k| {
                                k == "track"
                                    || k.starts_with("knob")
                                    || k == "thumb"
                                    || k.starts_with("thumb")
                                    || k.starts_with("liquid:btn:")
                                    || k.starts_with("liquid:dd:")
                            }) && std::ptr::eq(key, &zephyra_core::props::BACKGROUND);
                        if !is_fill_geometry && !is_animated_color {
                            offenders.push(format!(
                                "{} / {}",
                                n.key().map(|k| k.to_string()).unwrap_or_else(|| "?".into()),
                                key.name()
                            ));
                        }
                    }
                }
            });
            assert!(
                offenders.is_empty(),
                "源码把外观写进了 Local 层（route={route}, hovered={hovered}, pressed={pressed}）：\n  {}",
                offenders.join("\n  ")
            );
        }
    }
    assert!(
        seen_nodes > 200,
        "树遍历得太少（{seen_nodes} 个）—— 这条可能恒真，先查 walk()"
    );
}

// ------------------------------------------------------- 门禁 2：分类名存在

/// 源码发出的每个分类名，样式表里必须有规则。
///
/// 拼错分类名时**什么都不发生**（不报错、不崩、只是退回默认值），而「退回
/// 默认值」看起来就像「样式表没生效」——这是整条迁移路上最隐蔽的失败模式。
#[test]
fn every_class_name_the_source_emits_exists_in_the_stylesheet() {
    let sheet = merged_sheet();
    let known: Vec<&str> = sheet
        .rules()
        .iter()
        .flat_map(|r| r.selector.names.iter().copied())
        .collect();
    let routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();
    let mut emitted = 0;
    let mut missing: Vec<String> = Vec::new();
    for route in routes {
        let mut g = Gallery::default();
        g.route = route;
        let theme = Theme::builtin(ThemeVariant::Dark);
        let ctx = Context::new(&theme);
        let element = g.view(&ctx);
        let mut root = Node::from_element(&element);
        sheet.apply_all(&mut root);
        walk(&root, &mut |n: &Node| {
            for name in n.style_names() {
                emitted += 1;
                if !known.contains(name) {
                    missing.push(format!(
                        "{} : .{}",
                        n.key().map(|k| k.to_string()).unwrap_or_else(|| "?".into()),
                        name
                    ));
                }
            }
        });
    }
    assert!(
        emitted > 100,
        "只发出 {emitted} 个分类名 —— 树遍历可能没走完，这条测试有恒真的风险"
    );
    assert!(
        missing.is_empty(),
        "这些分类名在样式表里没有对应规则（拼错就是静默失效）：\n  {}",
        missing.join("\n  ")
    );
}

/// 下拉菜单的形变档位分类名必须**每一档**都有规则。
///
/// `dd-morph-N` 驱动「原方块扩大成菜单」的几何与透明度，只在开合过程中发，
/// 默认态不可见。直接拿 `motion::MORPH_CLASSES` 与表对，不依赖时间点。
#[test]
fn every_dropdown_morph_step_has_a_rule() {
    let sheet = merged_sheet();
    let known: Vec<&str> = sheet
        .rules()
        .iter()
        .flat_map(|r| r.selector.names.iter().copied())
        .collect();
    let classes = zephyra_gallery::app::motion::MORPH_CLASSES;
    assert!(
        classes.len() >= 8,
        "档位太少（{}）—— 短动画会看出阶梯",
        classes.len()
    );
    let mut missing: Vec<&str> = Vec::new();
    for cls in classes {
        if !known.contains(&cls) {
            missing.push(cls);
        }
    }
    assert!(
        missing.is_empty(),
        "这些形变档位在样式表里没有规则：{missing:?}"
    );
}

/// 静与动两条路径都要落到**节点**上，而不只是发对分类名。
///
/// 分类名对、样式规则也在，仍可能被更高优先级或更早的 `opacity: 1` 盖住。
/// 这一条把「点击 → 推进一帧 → 建树 → 应用样式表」整条链走完，直接读节点
/// 的最终状态：形变第一帧必须在中间档位（分类名与几何/透明度已落地）。
#[test]
fn dropdown_morph_lands_on_the_control_node() {
    let mut g = Gallery::default();
    g.route = "reference";
    g.click(zephyra_gallery::app::LIQUID_DD_SOLID_KEY);
    g.on_input(&zephyra_fx::InputState {
        dt_ms: 16.0,
        ..zephyra_fx::InputState::default()
    });
    let progress = g.liquid_dd_solid_motion.value();
    assert!(
        progress > 0.0 && progress < 1.0,
        "第一帧进度应在中途，实际 {progress}"
    );

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let mut root = Node::from_element(&g.view(&ctx));
    merged_sheet().apply_all(&mut root);

    let mut found: Option<(String, f32)> = None;
    walk(&root, &mut |n: &Node| {
        if n.key().and_then(|k| k.as_name()) == Some(zephyra_gallery::app::LIQUID_DD_SOLID_KEY) {
            found = Some((n.style_names().join(" "), n.opacity(ctx)));
        }
    });
    let (classes, opacity) = found.expect("树上找不到下拉控件 —— 路由或 key 变了");
    assert!(
        classes.split(' ').any(|c| c.starts_with("dd-morph-")),
        "控件没拿到形变档位，实际分类名：{classes}"
    );
    assert!(
        opacity > 0.0 && opacity <= 1.0,
        "形变档位没落到节点上（opacity={opacity}，分类名 {classes}）"
    );
}

// ------------------------------------------------------- 门禁 3：元素可被选中

/// 反方向：每个元素要么有分类名，要么其类型有裸规则。
///
/// 防的是「少写一个 `.style(..)`」这种最安静的删除：那一个元素谁都选不中，
/// 外观全部退回默认值，而源码侧全绿。
#[test]
fn every_element_is_reachable_by_the_stylesheet() {
    let sheet = merged_sheet();
    let bare: Vec<NodeTag> = sheet
        .rules()
        .iter()
        .filter(|r| r.selector.names.is_empty())
        .filter_map(|r| r.selector.tag)
        .collect();
    let routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();
    let mut total = 0;
    let mut unreachable: Vec<String> = Vec::new();
    for route in routes {
        let mut g = Gallery::default();
        g.route = route;
        let theme = Theme::builtin(ThemeVariant::Dark);
        let ctx = Context::new(&theme);
        let element = g.view(&ctx);
        let mut root = Node::from_element(&element);
        sheet.apply_all(&mut root);
        walk(&root, &mut |n: &Node| {
            total += 1;
            if n.style_names().is_empty() && !bare.contains(&n.tag()) {
                unreachable.push(format!(
                    "{} ({:?})",
                    n.key().map(|k| k.to_string()).unwrap_or_else(|| "?".into()),
                    n.tag()
                ));
            }
        });
    }
    assert!(total > 200, "树遍历得太少（{total} 个）");
    assert!(
        unreachable.is_empty(),
        "这些元素既没分类名、其类型也没有裸规则，样式表选不中它们：\n  {}",
        unreachable.join("\n  ")
    );
}

// ------------------------------------------------------- 门禁 4：令牌存在

/// 样式表里引用的每个 `@令牌` 都必须能在 `Theme::role()` 里查到。
///
/// 拼错令牌时 `resolve_resource` 返回 `None`、属性**静默**保持原值 ——
/// 这正是「样式全没了」类故障最安静的成因。按张量表逐个对，让它在
/// `cargo test` 阶段就红。
#[test]
fn every_token_the_stylesheet_uses_exists_in_theme() {
    let mut used: Vec<String> = Vec::new();
    // 遍历内嵌源（控件层 + 本应用），不是磁盘目录 —— 理由同 `merged_sheet`。
    for (name, css) in zephyra_gallery::styles::all() {
        // `@令牌` 只出现在值位上：抓所有 `@word`。
        for (i, _) in css.match_indices('@') {
            let rest = &css[i + 1..];
            let word: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if !word.is_empty() {
                used.push(format!("{name}.zstyle:{word}"));
            }
        }
    }
    assert!(
        !used.is_empty(),
        "一个 @令牌 都没用到 —— 这条测试有恒真的风险"
    );
    let theme = Theme::builtin(ThemeVariant::Dark);
    let light = Theme::builtin(ThemeVariant::Light);
    let mut missing: Vec<String> = Vec::new();
    for token in &used {
        let name = token.rsplit(':').next().unwrap();
        if theme.role(name).is_none() || light.role(name).is_none() {
            missing.push(token.clone());
        }
    }
    // 顺带钉住 tokens.rs 的名单与 core 的 role 表一致（防两份漂移）。
    for t in TOKENS {
        assert!(
            theme.role(t).is_some() && light.role(t).is_some(),
            "tokens.rs 的 {t} 在主题里查不到 —— 两份名单漂移了"
        );
    }
    assert!(
        missing.is_empty(),
        "这些 @令牌 在 Theme::role() 里不存在（拼错 = 静默退回默认值）：\n  {}",
        missing.join("\n  ")
    );
}

// ------------------------------------------------------- 门禁 5：页可达

/// 每个路由目标页都能造出一棵有规模的树。
#[test]
fn every_page_builds_a_real_tree() {
    let routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();
    for route in routes {
        let root = laid_out(route);
        let n = count_nodes(&root);
        assert!(
            n > 40,
            "页 {route} 只造出 {n} 个节点 —— 要么页面是空的，要么树遍历坏了"
        );
    }
}

// ------------------------------------------------------- 门禁 6：常量对齐

/// shell.rs 的外壳常量必须与 gallery.zstyle 里的值逐字相等。
///
/// 两处各写一个数的后果：窗口 1400 而卡片 1159 对不上时不会报错，
/// 只是右下缺一条 —— 那种"看起来像设计如此"的错最难查。
#[test]
fn shell_metrics_match_stylesheet() {
    let css = shell_css();
    let want = [
        (
            "box.titlebar",
            "height",
            zephyra_gallery::app::shell::TITLEBAR_H,
        ),
        ("box.nav-pane", "width", zephyra_gallery::app::shell::NAV_W),
        ("box.nav-pane", "height", zephyra_gallery::app::shell::NAV_H),
        (
            "box.content-card",
            "width",
            zephyra_gallery::app::shell::CONTENT_W,
        ),
    ];
    for (selector, prop, expect) in want {
        // 取该规则块里该属性的第一个值。
        let block = css
            .split(&format!("{selector} {{"))
            .nth(1)
            .unwrap_or_else(|| panic!("样式表里找不到 {selector} 规则"));
        let block = &block[..block.find('}').unwrap_or(block.len())];
        let line = block
            .lines()
            .map(|l| l.trim())
            .find(|l| l.starts_with(&format!("{prop}:")))
            .unwrap_or_else(|| panic!("{selector} 里找不到 {prop}"));
        let got: f32 = line
            .trim_start_matches(&format!("{prop}:"))
            .trim()
            .trim_end_matches("px;")
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("{selector} 的 {prop} 不是数字：{line}"));
        assert_eq!(
            got, expect,
            "{selector} 的 {prop}：样式表 {got} ≠ shell.rs 常量 {expect}"
        );
    }
}

/// 根容器**必须不写** `width` / `height`。
///
/// # 为什么这是动态布局的**前提**而不是细节
///
/// `arrange` 无条件把根矩形设成 `bounds`（= 窗口客户区），所以铺满窗口不需要写尺寸。
/// 而写上 `width: 1400px` 会让 `measure` 返回 1400——一切按内容算可用宽的分支
/// （列数、banner 高度、是否折叠导航）都会拿到这个"假窗口宽"。症状：
///
/// - 窗口拉到 1882 时，右侧留 386 单位**空白**（内容仍按 1400 排）；
/// - 窗口缩到 982 时，内容溢出被**裁**（同样按 1400 排）。
///
/// 两种表现看起来都像"渲染坏了"，而根因只是样式表里多了两行。
///
/// 判据是"写没写"，不是"写了多少"：数值对错还能调，**这两行存在**就让整个
/// 响应式失效，且没有任何报错。
#[test]
fn root_container_writes_no_fixed_size() {
    let css = shell_css();
    let start = css
        .split("box.window {")
        .nth(1)
        .unwrap_or_else(|| panic!("样式表里找不到 box.window"));
    let block = &start[..start.find('}').unwrap_or(start.len())];
    for prop in ["width", "height"] {
        assert!(
            !block
                .lines()
                .any(|l| l.trim().starts_with(&format!("{prop}:"))),
            "box.window 写了 {prop} —— 根容器必须由 arrange 按 bounds 铺满，\
             写死尺寸会让整个响应式失效（窗口变化时内容仍按旧尺寸排）"
        );
    }
}

/// 基准画布（1400×800）下，根矩形必须仍铺满客户区。
///
/// # 为什么去掉固定尺寸之后还要这一条
///
/// 因为"去掉显式尺寸"有另一种可能的失败：如果 `arrange` 改成按测量结果铺，
/// 根矩形会缩到内容尺寸，于是根背景不再铺满窗口 —— 右侧和底部露出未绘制区。
/// 那条路**也不报错**，只是"窗口边缘多一圈别的东西"。
///
/// 所以这条门禁把两件事一起钉住：
/// 1. 根矩形 == 客户区（铺满）；
/// 2. 基准画布下与去掉尺寸之前**无差别**（回归保护）。
#[test]
fn root_container_still_fills_client_area_at_baseline() {
    let root = laid_out("guide");
    let r = root.rect();
    let (cw, ch) = zephyra_gallery::app::BASE_VIEWPORT;
    assert_eq!(
        (r.width, r.height),
        (cw as f32, ch as f32),
        "根矩形必须铺满基准画布：{r:?}"
    );
}

// ------------------------------------------------------- 门禁 7：外壳几何

/// 真去**量布局后的矩形**：标题栏铺满画布宽、内容卡贴着导航右侧、
/// 置底的 Settings 落在画布底部附近。
///
/// core 没有 flex-grow，"铺满"靠的是固定宽高常量 —— 常量错了这里立刻红。
#[test]
fn shell_layout_geometry() {
    let root = laid_out("guide");
    assert!(count_nodes(&root) > 40, "树是空的，几何门禁恒真");

    let titlebar = find_by_key(&root, "titlebar").expect("titlebar");
    let r = titlebar.rect();
    assert!(
        (r.width - 1400.0).abs() < 1.0,
        "标题栏应铺满画布宽（1400），实测 {:.1}",
        r.width
    );
    assert!(
        (r.height - 44.0).abs() < 1.0,
        "标题栏高应是 44，实测 {:.1}",
        r.height
    );

    let content = find_by_key(&root, "content").expect("content card");
    let rc = content.rect();
    assert!(
        (rc.x - 260.0).abs() < 1.0,
        "内容卡应紧贴导航右侧（x=260），实测 {:.1}",
        rc.x
    );
    assert!(
        (rc.width - 1139.0).abs() < 1.0,
        "内容卡宽应是 1139，实测 {:.1}",
        rc.width
    );
    assert!(
        (rc.height - 756.0).abs() < 1.0,
        "内容卡高应是 756，实测 {:.1}",
        rc.height
    );

    // 导航栏的最后一项不得溢出导航容器。
    //
    // 2026-11-01 起没有「置底项」了（设置页整页撤下），所以不再断言
    // 「某一项贴在底部」—— 那是置底项特有的性质。现在只守住**不溢出**：
    // 项数变多而容器高度不变时，最后一项会掉出容器，而症状在画面上只是
    // 「导航栏最后一项看不见了」。
    let nav = find_by_key(&root, "nav").expect("nav pane");
    let nr = nav.rect();
    let last = ROUTES
        .iter()
        .map(|(id, _, _)| *id)
        .filter_map(|id| find_by_key(&root, id))
        .map(|n| n.rect())
        .max_by(|a, b| {
            (a.y + a.height)
                .partial_cmp(&(b.y + b.height))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("导航栏至少有一项");
    let last_bottom = last.y + last.height;
    assert!(
        last_bottom <= nr.y + nr.height + 0.5,
        "导航栏最后一项底部 {last_bottom:.1} 溢出了导航容器（底 {:.1}）",
        nr.y + nr.height
    );
}

// ------------------------------------------------------- 门禁 8：唯一例外

/// 源码里唯一允许写 `WIDTH` 的形态：key 以 `fill:` 开头（滑条/进度条填充）。
///
/// 黑名单会烂，形状不会 —— 这条钉的是**形状**，所以新增一处数据几何时
/// 必须显式命名，而不是悄悄加进一个清单里。
#[test]
fn only_fill_nodes_carry_local_width() {
    let routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();
    let mut bad: Vec<String> = Vec::new();
    let mut fills = 0;
    for route in routes {
        let root = laid_out(route);
        walk(&root, &mut |n: &Node| {
            let has_local_width = n
                .props()
                .iter_local()
                .any(|(k, _)| std::ptr::eq(k, &zephyra_core::props::WIDTH));
            if !has_local_width {
                return;
            }
            let is_fill = n
                .key()
                .and_then(|k| k.as_name())
                .is_some_and(|k| k.starts_with("fill:"));
            if is_fill {
                fills += 1;
            } else {
                bad.push(format!(
                    "{} ({route})",
                    n.key().map(|k| k.to_string()).unwrap_or_else(|| "?".into())
                ));
            }
        });
    }
    // 2026-11-01 起 gallery 不再演示 Slider / ProgressBar —— 那两个是
    // 唯一带 `fill:` 局部宽度的控件，于是「fill:* 至少 4 个」这条判据
    // 永远不成立（恒真 = 这条门禁失去作用）。
    //
    // 改成**反向**的更强判据：源码里**一个局部 WIDTH 都不该有**。
    // 若将来重新引入带填充的控件，把这条改回「WIDTH 只能出现在 key 以
    // `fill:` 开头的节点上」并恢复计数下界。
    assert_eq!(
        fills, 4,
        "gallery 的 fill:* 节点数变了（当前应为 4：slider solid/glass 各一枚，text field 选区块一枚，catalog 进度条一枚）。         若新增/移除带填充的控件，按实际数量更新本判据与上方注释"
    );
    assert!(
        bad.is_empty(),
        "这些节点在源码里写了 WIDTH 但 key 不是 fill:*：\n  {}",
        bad.join("\n  ")
    );
}

// ------------------------------------------------------- 门禁 9：图标已登记

/// 源码里引用的每一个图标名，都必须在内置表里**登记过**。
///
/// # 为什么这条必须住在**应用**侧而不是 core 侧
///
/// core 的 `all_builtin_icons_are_non_empty`（新加）能抓「登记了但资源是空的」，抓不了
/// 「压根没登记」—— 没登记的名字根本不会出现在那张表里，而它在本仓造成过的故障
/// 是：`DISMISS` 常量与 `dismiss.svg` 资源都在，唯独 `REGISTRY` 少一行，于是
/// `IconSet::get` 回落到 `Icon::EMPTY`，界面上的图标位**是空的**。
///
/// 症状安静到无法自证：没有报错、没有告警、节点矩形还在（那格留白），
/// 只有「图上少东西」这件事 —— 而没人会怀疑自己看错。
///
/// 所以判据必须站在**引用方**：树里出现过的名字，逐个查表。
#[test]
fn every_icon_name_the_tree_emits_is_registered() {
    let known: std::collections::HashSet<&str> = zephyra_core::icon::assets::REGISTRY
        .iter()
        .map(|(n, _)| *n)
        .collect();
    // ⚠️ 查的是**登记名**，不是资源文件名：内置表刻意用「控件的词汇」
    // （关掉 = `close`，资源文件却叫 `dismiss.svg`，见 `assets::REGISTRY` 的文档）。
    // 按文件名去查会把「用词不同」误判成「没登记」，反过来也会把真故障放过去 ——
    // 本条在落地当天就抓到过一次：应用按文件名写了 `dismiss`，而登记名是
    // `close`，于是那个图标位一直是空的。
    let routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();
    let mut used: Vec<String> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for route in routes {
        let root = laid_out(route);
        walk(&root, &mut |n: &Node| {
            for (key, value) in n.props().iter_local() {
                if !std::ptr::eq(key, &zephyra_core::props::ICON_NAME) {
                    continue;
                }
                let name = match value {
                    zephyra_core::property::PropertyValue::StaticStr(s) => *s,
                    zephyra_core::property::PropertyValue::Str(s) => s.as_ref(),
                    _ => continue,
                };
                used.push(name.to_string());
                if !known.contains(name) {
                    bad.push(format!(
                        "{} ({route}) -> {name}",
                        n.key().map(|k| k.to_string()).unwrap_or_else(|| "?".into())
                    ));
                }
            }
        });
    }
    // 十个路由里有 30+ 个图标位；少于 20 说明遍历没走完，这条可能恒真。
    assert!(
        used.len() >= 20,
        "只用到 {} 个图标名 —— 遍历可能没走完",
        used.len()
    );
    assert!(
        bad.is_empty(),
        "这些图标名不在内置表里（画面上会是空白图标位）：\n  {}",
        bad.join("\n  ")
    );
}

// -------------------------------------------------- 门禁 11：命中与绘制同一套几何

/// 命中测试选中的元素，必须**就是**那个像素上画着的东西。
///
/// # 为什么这条必须有
///
/// 命中与绘制读的是**同一棵树**（`HostWindow::route` 与 `render_pass` 都取
/// `cache.node()`），所以一旦它们对不上，症状就只有一种：**点击落在 A 上、
/// 却执行了 B 的动作** —— 界面上没有任何异常（该亮的亮了、该画的画了），
/// 只有"点哪儿不好使"。
///
/// 而这种偏移一旦存在，用截图核对是**核不出来的**：画面完全正确。
///
/// 判据取自实际像素坐标：`rect()` 是绘制用的绝对矩形，把点取在它的**中心**，
/// 处理目标必须是它自己。
///
/// 用 `HitResult::target()`（最深的**可交互**节点）而不是 `deepest()`（最深的
/// 任何节点）：命中链本来就包含图标、文本这些后代，它们不是处理目标 ——
/// `INTERACTIVE` 是**不可继承**属性（见 `props::INTERACTIVE`），只有控件本身#[test]
/// 布局页的两个 flex 演示必须**真的在分配**，而不是只在样式表里说了。
///
/// # 为什么值得一条几何门禁
///
/// flex 的激活链很长：`.zstyle` 文本 → `zstylec` 解析 → 属性键 →
/// `PropertyStore` → `Node::flex()` 的激活判断 → `measure_flex` / `arrange_flex`。
/// 任何一环断掉，症状都是「样式表写着 `flex-grow: 1`，画面上那一格纹丝不动」——
/// 不崩、不报错、门禁全绿（`every_stylesheet_rule_matches_something` 只查规则
/// 有没有选中元素，不查它有没有**效果**）。
///
/// 所以这里直接量最终几何：grow 格必须比固定格宽得多；折行演示的第四个格
/// 必须换到第二行（y 更大、x 归零）。这两条只可能由真的分配产生。
#[test]
fn the_flex_demos_on_the_layout_page_really_allocate() {
    let root = laid_out("layout");

    let mut rects: Vec<(&str, zephyra_core::core::Rect)> = Vec::new();
    walk(&root, &mut |n: &Node| {
        if let Some(name) = n.key().and_then(|k| k.as_name()) {
            if name.starts_with("fx-") {
                rects.push((name, n.rect()));
            }
        }
    });
    let rect_of = |name: &str| {
        rects
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("树上找不到 {name} —— 布局页的 flex 演示没了？"))
    };

    // grow：右格吃掉 420 − 100 − 10 = 310，固定格保持 100。
    // 若 flex 没激活，两格都是 100（按内容收窄），差值立刻归零。
    let fixed = rect_of("fx-fixed");
    let grow = rect_of("fx-grow-cell");
    assert!(
        grow.width > fixed.width + 100.0,
        "flex-grow 没生效：固定格 {:?} 宽 {}，填充格宽 {}（应接近 310）",
        fixed,
        fixed.width,
        grow.width
    );

    // 折行：340px 装得下 100+10+100+10+100 = 330，装不下第四个 →
    // 第四格必须换行（y 变大、x 回到行首）。若 flex-wrap 没生效，
    // 六格会横排成一行（y 全等），而容器宽度会被撑到 650。
    let w1 = rect_of("fx-w1");
    let w4 = rect_of("fx-w4");
    assert!(
        w4.y > w1.y + 10.0,
        "flex-wrap 没生效：第一格 y={}，第四格 y={}（应在第二行）",
        w1.y,
        w4.y
    );
    assert!(
        (w4.x - w1.x).abs() < 1.0,
        "第二行该从行首开始：第一格 x={}，第四格 x={}",
        w1.x,
        w4.x
    );
    assert!(
        w4.x < w1.x + 150.0,
        "第四格不该跟在第一格后面横排（x={}）—— 那说明压根没折行",
        w4.x
    );
}

#[test]
fn hit_test_agrees_with_the_painted_rects() {
    // ⚠ 这一条必须跑**全部**路由，不是只跑 home。
    //
    // 旧版只测 home，于是「某一页内容比内容卡高、最后几行掉到窗口外面」这件事
    // 完全没被看见：那些节点确实存在于树里、也确实可交互，只是中心落在客户区
    // 之外，命中测试按定义不会命中它们 —— 画面上就是「点不到」，且不报任何错。
    //
    // core 既没有滚动也没有裁剪溢出（`docs/dev-log/README.md` 记着「core 目前
    // 无滚动」），所以「一屏装不下」唯一的表现就是**下沿被吃掉**。要留住就得
    // 逐页量。
    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);

    let mut wrong: Vec<String> = Vec::new();
    let mut checked = 0usize;

    // 参考层两族各有各的排版，`--route reference` 只出第 0 族 ——
    // 要逐族量，否则第 1 族的掉出完全看不见。
    let mut pages: Vec<(String, Node)> = zephyra_gallery::app::list_routes()
        .into_iter()
        .map(|route| (route.to_string(), laid_out(route)))
        .collect();
    for (i, name) in REF_FAMILIES.iter().enumerate() {
        pages.push((format!("reference/{name}"), laid_out_from("reference", i)));
    }

    for (label, root) in &pages {
        let client = root.rect();

        let mut targets: Vec<(String, zephyra_core::core::Rect)> = Vec::new();
        walk(root, &mut |n: &Node| {
            if n.props()
                .lookup(&zephyra_core::props::INTERACTIVE)
                .map(|(v, _)| matches!(v, zephyra_core::property::PropertyValue::Bool(true)))
                .unwrap_or(false)
            {
                targets.push((n.key().map(|k| k.to_string()).unwrap_or_default(), n.rect()));
            }
        });

        for (key, rect) in &targets {
            // 零尺寸的节点跳过：它们在树里可交互，但布局给不出面积（父容器把它
            // 压成 0 宽/0 高），于是既画不出来也点不到。判据是**面积**，不是
            // 「有没有 key」—— 留着会让这条门禁对每一个 `.close` 按钮都误报。
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }
            // 掉出客户区的先报出来 —— 那是「看得见但点不到」的一类。
            if rect.y + rect.height > client.height + 0.5
                || rect.x + rect.width > client.width + 0.5
            {
                wrong.push(format!(
                    "{label} / {key}：矩形 {:?} 掉出客户区 {:?} —— 画面上被吃掉，点不到",
                    rect, client
                ));
                continue;
            }
            // 1px 的框在中心可能恰好落在边界外，取中心即可（矩形高宽都 >= 1）。
            let point = zephyra_core::core::Point::new(
                rect.x + rect.width * 0.5,
                rect.y + rect.height * 0.5,
            );
            let mut hit = zephyra_core::hit::HitResult::default();
            checked += 1;
            if !zephyra_core::hit::hit_test(
                root,
                zephyra_core::hit::HitTestRequest { point },
                ctx,
                &mut hit,
            ) {
                wrong.push(format!("{label} / {key}：它的中心 {point:?} 谁都没命中"));
                continue;
            }
            let Some(target) = hit.target() else {
                wrong.push(format!(
                    "{label} / {key}：中心 {point:?} 命中了某个**不可交互**的节点 —— \\
                     说明它被压在别人下面（谁盖住了它？）"
                ));
                continue;
            };
            // 目标可以是自己，**也可以是自己的后代**。
            //
            // 组合控件就是这样：`SpinBox` 本体是 INTERACTIVE（这样整块都能点），
            // 而它的 `spin:up` / `spin:down` 两个按钮叠在上面。所以 SpinBox
            // 的矩形中心落在「减号」上时，处理目标是 `spin:down` —— 那是**正确**的，
            // 因为命中链取的是最深的可交互节点。
            //
            // 只允许「自己或后代」是有意的：祖先不是。如果处理目标是祖先，
            // 说明子控件被什么东西盖住了，而盖住它的东西不在自己的子树里。
            // 目标可以是自己，**也可以是自己的后代**。
            //
            // 组合控件就是这样：`SpinBox` 本体是 INTERACTIVE（这样整块都能点），
            // 而它的 `spin:up` / `spin:down` 两个按钮叠在上面。所以 SpinBox 的
            // 矩形中心落在「减号」上时，处理目标是 `spin:down` —— 那是**正确**的，
            // 因为命中链取的是最深的可交互节点。
            //
            // 所以判据不能是「id 相等」，而要问一句更朴素的话：**这个点上有没有
            // 属于它自己子树的可交互节点？** 没有，就说明它被子树外的东西盖住了
            // （盖住它的不是祖先，因为它在自己子树里没有命中）—— 那才是真的错。
            let mut covered_by_own_subtree = false;
            walk(root, &mut |n: &Node| {
                if !covered_by_own_subtree
                    && n.key().and_then(|k| k.as_name()) == Some(key.as_str())
                {
                    covered_by_own_subtree = deepest_interactive_in(n, point).is_some();
                }
            });
            if !covered_by_own_subtree {
                wrong.push(format!(
                    "{label} / {key}：中心 {point:?} 的处理目标是 {:?}（矩形 {:?}）—— \
                     而它自己子树里没有任何可交互节点在那个点上",
                    target.id, target.rect
                ));
            }
        }
    }

    assert!(
        checked >= 150,
        "只量了 {checked} 个可交互节点 —— 遍历可能没走完，这条有恒真的风险"
    );
    assert!(
        wrong.is_empty(),
        "命中测试与绘制几何对不上（点 A 执行 B / 掉出客户区）：\n  {}",
        wrong.join("\n  ")
    );
}

// -------------------------------------------------- 门禁 10：交互不留残迹

/// 一条**交互序列**：保留式树跑过它之后，必须与「同一状态下冷启动的树」完全一致。
///
/// # 判据为什么是「与冷启动比」而不是「与上一帧比」
///
/// 交互 bug 的形状是**状态残留**：点一下之后，上一个状态的样式值赖在新树上。
/// 「和上一帧比」会把残留当成正确（上一帧确实就是那个值）。
/// 而「同一状态冷启动」是**无历史的正确答案** —— 保留式路径与它逐节点相等，
/// 就是没有残留；不等，就是有。
///
/// # 这条为什么必须有
///
/// `--dump-cpu` 只渲一帧（无输入），所以**任何交互路径的缺陷它都看不见**：
/// 单帧图永远是对的。缺陷只在「连着点几下」之后才出现，而那种缺陷恰好是
/// 最伤的一类 —— 用户看到的是「按钮点了没反应 / 点了以后别的按钮花了」。
#[test]
fn interaction_leaves_no_stale_appearance() {
    use zephyra_core::frame::FrameCache;
    use zephyra_style::SheetStyler;

    const VIEWPORT: (f32, f32) = (1400.0, 800.0);
    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let sheet = std::sync::Arc::new(merged_sheet());
    let styler =
        || Box::new(SheetStyler::new(sheet.clone())) as Box<dyn zephyra_core::styler::Styler>;

    // 脚本覆盖四类会改外观的状态：路由、勾选、开关、折叠、选中、悬停/按下。

    // 一条门禁脚本：改哪儿（名字） + 怎么改（闭包）+ 期望（由闭包自己断言）。
    let script: GateScripts = interaction_scripts();

    let mut g = Gallery::default();
    let mut warm = FrameCache::new();
    warm.set_styler(styler());
    warm.frame(&g.view(&ctx), ctx, VIEWPORT);

    let mut step_no = 0;
    for (name, step) in script {
        step_no += 1;
        step(&mut g);
        warm.frame(&g.view(&ctx), ctx, VIEWPORT);

        let mut cold = FrameCache::new();
        cold.set_styler(styler());
        cold.frame(&g.view(&ctx), ctx, VIEWPORT);

        let (Some(w), Some(c)) = (warm.node(), cold.node()) else {
            panic!("第 {step_no} 步（{name}）之后树建不起来");
        };
        let diffs = compare_trees(w, c, &mut String::new());
        assert!(
            diffs.is_empty(),
            "第 {step_no} 步「{name}」之后，保留式树与冷启动树不一致：\n  {}\n\
             （这意味着上一状态的外观值赖在新树上 —— 点击后界面花掉就是它）",
            diffs.join("\n  ")
        );
    }
}

/// `node` 的子树里，**最深**的、矩形包含 `point` 的可交互节点。
///
/// # 为什么不能直接比 id
///
/// `ElementKey` 可以带序号后缀，而控件内部的叶子节点大量复用同一个名字
/// （每个按钮的图标节点都叫 `icon`）。比 id 在嵌套同名时会给出假阳性 ——
///
/// # 为什么只需要「有没有」，不需要「是哪一个」
///
/// 门禁要回答的是「这个可交互节点被点时会不会落到它**自己人**手里」。
/// 组合控件（SpinBox / Segmented / Card-with-buttons）里，子控件叠在父控件
/// 上方是**设计如此**，命中落到子控件上是正确的。真正要抓的是相反的情况：
/// 子树里一个都没有，命中却落到了外面某个不相干的节点上 —— 那才是「点 A 执行 B」。
fn deepest_interactive_in(node: &Node, point: zephyra_core::core::Point) -> Option<&Node> {
    fn contains(r: zephyra_core::core::Rect, p: zephyra_core::core::Point) -> bool {
        p.x >= r.x && p.x <= r.x + r.width && p.y >= r.y && p.y <= r.y + r.height
    }
    fn interactive(n: &Node) -> bool {
        n.props()
            .lookup(&zephyra_core::props::INTERACTIVE)
            .map(|(v, _)| matches!(v, zephyra_core::property::PropertyValue::Bool(true)))
            .unwrap_or(false)
    }

    let mut best = (interactive(node) && contains(node.rect(), point)).then_some(node);
    for c in node.children() {
        if let Some(d) = deepest_interactive_in(c, point) {
            // 深度优先、后来者覆盖 —— 子树里越深的越「新」。
            best = Some(d);
        }
    }
    best
}

/// 逐节点逐属性对比两棵树，返回差异描述（空 = 完全一致）。
///
/// 对比的键是**视觉上看得见的那批**：颜色、圆角、边、字号、尺寸、间距、对齐、
/// 折叠、文字与图标名 —— 任何一条不一致都会在画面上留下痕迹。
fn compare_trees(warm: &Node, cold: &Node, path: &mut String) -> Vec<String> {
    const KEYS: &[&zephyra_core::property::PropertyKey] = &[
        &zephyra_core::props::BACKGROUND,
        &zephyra_core::props::BORDER_COLOR,
        &zephyra_core::props::BORDER_WIDTH,
        &zephyra_core::props::CORNER_RADIUS,
        &zephyra_core::props::TEXT_COLOR,
        &zephyra_core::props::TEXT_SIZE,
        &zephyra_core::props::ICON_COLOR,
        &zephyra_core::props::ICON_NAME,
        &zephyra_core::props::TEXT,
        &zephyra_core::props::WIDTH,
        &zephyra_core::props::HEIGHT,
        &zephyra_core::props::PADDING,
        &zephyra_core::props::SPACING,
        &zephyra_core::props::DIRECTION,
        &zephyra_core::props::MAIN_ALIGN,
        &zephyra_core::props::CROSS_ALIGN,
        &zephyra_core::props::COLLAPSED,
        &zephyra_core::props::OPACITY,
    ];
    let mut out = Vec::new();
    if warm.style_names() != cold.style_names() {
        out.push(format!(
            "{path}分类名不同：{:?} vs {:?}",
            warm.style_names(),
            cold.style_names()
        ));
    }
    for key in KEYS {
        let a = warm.props().lookup(key);
        let b = cold.props().lookup(key);
        if a != b {
            out.push(format!("{path}{}: {:?} vs {:?}", key.name(), a, b));
        }
    }
    if (warm.rect().x - cold.rect().x).abs() > 0.01
        || (warm.rect().y - cold.rect().y).abs() > 0.01
        || (warm.rect().width - cold.rect().width).abs() > 0.01
        || (warm.rect().height - cold.rect().height).abs() > 0.01
    {
        out.push(format!(
            "{path}矩形不同：{:?} vs {:?}",
            warm.rect(),
            cold.rect()
        ));
    }
    if warm.children().len() != cold.children().len() {
        out.push(format!(
            "{path}子节点数不同：{} vs {}",
            warm.children().len(),
            cold.children().len()
        ));
        return out;
    }
    for (i, (wc, cc)) in warm.children().iter().zip(cold.children()).enumerate() {
        let label = wc
            .key()
            .map(|k| k.to_string())
            .unwrap_or_else(|| format!("#{i}"));
        let saved = path.len();
        path.push_str(&format!("{label}/"));
        out.extend(compare_trees(wc, cc, path));
        path.truncate(saved);
    }
    out
}

// -------------------------------------------------- 门禁 13：半透明令牌的边界

/// 半透明令牌必须真的半透明——否则所有用它的表面全都失去层次。
///
/// # 为什么这条重要
///
/// `@surface_alt_translucent` 目前由控件层的按钮（hover 面）使用。它曾经服务
/// gallery 标题栏的 Mica 外观，DWM backdrop 移除后（2026-11，见
/// `docs/dev-log/2026-11-01-window-transparent.md`）控件侧用途仍在，令牌的
/// 「半透明」语义不能被顺手改成实色 —— 那会把按下/悬停的层次结构悄悄抹平，
/// 而这类缺陷没有像素层门禁是看不见的。
#[test]
fn the_translucent_surface_token_is_actually_translucent() {
    let dark = Theme::builtin(ThemeVariant::Dark);
    let light = Theme::builtin(ThemeVariant::Light);

    for (name, t) in [("dark", &dark), ("light", &light)] {
        let solid = t
            .role(zephyra_core::theme::roles::SURFACE_ALT)
            .expect("surface_alt 角色必须存在");
        let translucent = t
            .role(zephyra_core::theme::roles::SURFACE_ALT_TRANSLUCENT)
            .expect("surface_alt_translucent 角色必须存在");

        assert!(
            translucent.a < 1.0,
            "{name}: 半透明令牌的 alpha 应当 < 1.0，实际 {} —— \
             等于不透明的话悬停/按下的层次永远透不出来",
            translucent.a
        );
        assert!(
            translucent.a > 0.0,
            "{name}: alpha 为 0 会让标题栏文字没有任何底色"
        );
        // 颜色本身（RGB）应与实色一致 —— 半透明只改 alpha，不该顺手换色。
        // 差一点点是浮点误差，放宽到 0.02。
        assert!(
            (translucent.r - solid.r).abs() < 0.02
                && (translucent.g - solid.g).abs() < 0.02
                && (translucent.b - solid.b).abs() < 0.02,
            "{name}: 半透明令牌的 RGB 应与 surface_alt 一致（只改 alpha），\
             实得 {:?} vs {:?}",
            translucent,
            solid
        );
    }
}
// ---------------------------------- 门禁 12（重写后）：路由表与页面表必须一致

/// [`ROUTES`] 与 [`PAGES`] 的 id 集合必须**完全相同**。
///
/// # 为什么这两张表并存（而不是合并成一张）
///
/// 它们分工不同：`ROUTES` 给导航栏与 `resolve_route`（只需要 id / 标题 / 图标），
/// `PAGES` 是**全部**页面清单（含 Settings）。页面的结构住在 `frontend/shell.zypui`
/// —— 两份表漂了就是「点得进去、进去是首页」：地址栏那一瞬变了，随后一切
/// 正常，所以肉眼与截图都发现不了。
#[test]
fn route_table_and_page_table_agree() {
    // 2026-11-01 设置页整页撤下后，`ROUTES`（导航项）与 `PAGES`（全部可构造
    // 页面）重新变成**同一份清单** —— 之前是 ROUTES + Settings 与 PAGES 比。
    let from_routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();
    let from_pages: Vec<&str> = PAGES.iter().map(|p| p.0).collect();

    assert_eq!(
        from_routes, from_pages,
        "ROUTES 与 PAGES 的 id 集合或顺序不一致 —— 两份表漂了"
    );
}

/// 每个路由渲染出来的**必须就是它自己那一页**。
///
/// # 这条是为「静默回落首页」写的
///
/// `pages::page()` 对未知路由的兜底是 `home::page(g)` —— 不崩、不报错，
/// 画面上却有「标题写着 Settings、正文却是首页」这种一眼假的组合。
///
/// 而**已有的门禁一条都抓不到**：首页的树足够大（`every_page_builds_a_real_tree`
/// 过）、每个节点都在客户区内（命中几何过）、分类名齐全（样式门禁过）。
/// 它们量的是「页面非空」，不是「页面是对的」。
///
/// 判据走**页根节点的 key**：每个页面都用 `page_root(<route id>, "page-<id>")`
/// 起手，所以「这一页是这一页」这件事在树里有痕迹可查。
#[test]
fn every_route_renders_its_own_page() {
    for id in zephyra_gallery::app::list_routes() {
        let root = laid_out(id);
        let want = format!("page-{id}");

        // ⚠ 不能用 `find_by_key(root, id)`：侧栏的导航项与页根**同 key**
        // （"home" 既是路由 id 也是导航项 key），深度优先先撞上的一定是导航项。
        // 所以判据是「key 与分类名**同时**对得上」的那个节点。
        let mut hit: Option<Vec<&str>> = None;
        walk(&root, &mut |n: &Node| {
            if hit.is_none()
                && n.key().and_then(|k| k.as_name()) == Some(id)
                && n.style_names().contains(&want.as_str())
            {
                hit = Some(n.style_names().to_vec());
            }
        });
        assert!(
            hit.is_some(),
            "路由 {id} 的树上找不到「key == {id} 且带 .{want}」的节点 —— \
             大概率是 pages::page() 对未知路由静默回落到了首页"
        );
    }
}

/// 每个页面的 key 必须是**合法路由** —— 否则导航点它会静默停在当前页。
///
/// # 判据不走 UI，而走 `resolve_route`
///
/// 那是所有点击的必经之路（[`Gallery::click`] 的第一条分支就是它）。走真源
/// 就不用模拟 11 次点击，也不会因为命中测试的细节而给出假阴性。
#[test]
fn every_page_key_is_a_legal_route() {
    let bad: Vec<&str> = PAGES
        .iter()
        .filter(|p| zephyra_gallery::app::resolve_route(p.0).is_none())
        .map(|p| p.0)
        .collect();
    assert!(
        bad.is_empty(),
        "这些页面 id 解析不出路由（点导航会没反应）：\n  {}",
        bad.join("\n  ")
    );
}

/// 每个页面必须真的造出**有内容**的树 —— 不是「标题 + 什么都没有」。
///
/// # 为什么这条对重写后的结构更重要
///
/// 旧结构有 44 个控件页，任何一页塌成空壳都很难批量发现（要挨个点开 44 次）。
/// 现在只有 11 页，肉眼是能扫完的 —— 但门禁仍要留，因为**塌页不报错**：
/// 一个只含标题的 `Container` 完全合法，树建得出来、路由认得、截图也有东西。
///
/// 判据用**可交互节点数**而不是节点数：侧栏 11 项 + 页头两个工具在每一页都
/// 出现，所以「只有外壳」的页面仍然有 13 个可交互节点 —— 拿总数判的话每一页
/// 都会过，而那正是这条要挡的情况。
#[test]
fn every_page_carries_its_own_controls() {
    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);

    // 每页的外壳固定贡献 13 个可交互节点（侧栏 10 + Settings + 两个工具）。
    // 页面**自己**必须至少再贡献 1 个，否则就是「只有外壳的空页」。
    const SHELL_ONLY: usize = 13;

    let mut thin: Vec<String> = Vec::new();
    let mut total = 0usize;
    for id in zephyra_gallery::app::list_routes() {
        let mut g = Gallery::default();
        g.route = id;
        let mut root = Node::from_element(&g.view(&ctx));
        merged_sheet().apply_all(&mut root);

        let mut n = 0usize;
        walk(&root, &mut |node: &Node| {
            if node
                .props()
                .lookup(&zephyra_core::props::INTERACTIVE)
                .map(|(v, _)| matches!(v, zephyra_core::property::PropertyValue::Bool(true)))
                .unwrap_or(false)
            {
                n += 1;
            }
        });
        total += n;
        if n <= SHELL_ONLY {
            thin.push(format!("{id}: {n} 个可交互节点（外壳就占 {SHELL_ONLY}）"));
        }
    }
    assert!(
        total > SHELL_ONLY * zephyra_gallery::app::list_routes().len(),
        "总共只数到 {total} 个可交互节点 —— 遍历可能没走完，这条有恒真的风险"
    );
    assert!(
        thin.is_empty(),
        "这些页面除了外壳什么都没有：\n  {}",
        thin.join("\n  ")
    );
}

/// 语言自称（`LANG_SELFS`）必须与 `SUPPORTED` **同长**，且每门语言的条目数
/// 必须与键集合等大。
///
/// # 为什么这条值得一条门禁
///
/// 设置页的 segmented 是按下标对照两组表的：下标错位会让用户选了
/// "English" 却得到日文，而那在画面上完全看不出原因（文案是正常的、
/// 布局是正常的，只是语言不对）。这种错只有机器能查。
///
/// # 为什么只查条目数，不去枚举"表里多了一门语言"
///
/// [`Messages`] 刻意不提供枚举语言的 API（`i18n.rs` 里写明"宁可门禁抄一遍"
/// 也不加接口），所以这里用 `len_for` 反推：某门语言条目数少于键集合，
/// 就是漏翻；多于键集合，则是把同一 (locale, key) 写了两遍。
#[test]
fn every_language_name_is_counted_in_the_same_order() {
    use zephyra_gallery::app::{catalog_keys, messages, LANG_SELFS, SUPPORTED};

    assert_eq!(
        LANG_SELFS.len(),
        SUPPORTED.len(),
        "自称列表与语言表长度不一致（下标错位 = 选了 A 得到 B）"
    );
    // 键集合 = **应用的表 + 控件层的自带文案**（`messages()` 是并表后的）。
    //
    // ⚠ 2026-11-01 起控件层也带文案（复选框组组头的「全选」），所以键集合
    // 不再等于应用表的大小。少算控件层那几条 ⇒ 每门语言都少同样的数量 ⇒
    // **这条门禁会恒红**（不是恒绿，但一样是假的信号）。
    let app_keys = catalog_keys();
    let n_keys = app_keys.len() + zephyra_ui::i18n::BUILTIN_KEYS.len();

    // 撞名检查：两层用不同前缀（`zephyra.` vs 应用自己的），撞了就说明
    // 命名空间守则破了。撞名的后果是「控件层的默认值被应用的条目吃掉」，
    // 而症状只是组头那句变成了别的说法 —— 极难归因。
    for k in zephyra_ui::i18n::BUILTIN_KEYS {
        assert!(
            !app_keys.contains(k),
            "控件层 key `{k}` 与应用表撞名 —— 两层的前缀约定破了"
        );
    }

    for lang in SUPPORTED {
        assert_eq!(
            messages().len_for(lang),
            n_keys,
            "{lang} 的条目数与键集合不等大 —— 漏翻或重键"
        );
    }
}

/// 每个路由 id 在**每一门语言**下都必须有译文。
///
/// # 为什么必须逐语言查，而不是查「键集合齐全」
///
/// [`Gallery::tr`] 找不到键时回落到 `route.unknown` —— 那是个**合法**的
/// `&'static str`，画面上显示的字就是 `route.unknown` 这串字符，不崩、不空。
/// 所以「en 有 zh 没有」这类漏翻，只会在切语言的那一帧被人看见一眼就过去。
#[test]
fn every_route_id_is_translated_in_every_supported_language() {
    use zephyra_core::i18n::Preferences;
    use zephyra_gallery::app::{catalog_keys, messages, SUPPORTED};

    let keys = catalog_keys();
    let msgs = messages();
    let default = SUPPORTED[0];

    let mut problems: Vec<String> = Vec::new();
    for id in ROUTES.iter().map(|(id, _, _)| *id) {
        let key = format!("route.{id}");
        // 键本身必须存在：`route_key()` 找不到 `route.<id>` 时回落到
        // `route.unknown`，而那是个**合法**的 `&'static str` —— 不崩、只显示字面量。
        if !keys.contains(&key.as_str()) {
            problems.push(format!("{id}: 文案表里连键 {key} 都没有"));
            continue;
        }
        for lang in SUPPORTED {
            let prefs = Preferences::from_list(&[lang]);
            if msgs.select(&prefs, default, &key).is_none() {
                problems.push(format!("{id} / {lang}"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "这些路由在对应语言下查不到译文（画面上显示回落文案或键名）：\n  {}",
        problems.join("\n  ")
    );
}

/// 源码里每个 `g.tr("key")` 的键，都必须在**每一门语言**下查得到。
///
/// # 为什么这条不能由「路由译齐了」代劳
///
/// [`Gallery::tr`] 查不到键时返回**键名本身**（`messages().select(..).unwrap_or(key)`）：
/// 不崩、不空、颜色正常 —— 画面上显示的是 `ui.your-name` 这串字符。而
/// [`every_route_id_is_translated_in_every_supported_language`] 只查 `route.*`。
/// 页面里新加一句 `g.tr("ui.x")` 却忘了在表里加条目时，那条门禁全绿，
/// 画面上却是键名 —— 两件事只能分开查。
///
/// # 反向也查：表里的键不许只翻一门
///
/// 只查「用到的键都有」会漏掉「en 加了 zh 忘了」。所以这里同时拿
/// `catalog_keys()` 反查一遍 —— `Messages::missing_in` 是**精确**匹配
/// （`get` 不做回退），正合「逐语言」这个判据。
///
/// # 已知盲区
///
/// 扫的是源码文本，所以键若不是字面量（拼出来的、查表返回的）这条查不到。
/// 现状全部是字面量 —— 因为 `view()` 每帧重建整棵树，运行期拼串会泄漏
/// （见 `app::route_key` 的说明），本来就不该那么写。开头那句
/// `assert!(!used.is_empty())` 守的是「扫描方式过期了但门禁假装通过」。
#[test]
fn every_tr_key_used_in_source_exists_in_every_language() {
    use std::path::{Path, PathBuf};
    use zephyra_gallery::app::{catalog_keys, messages, SUPPORTED};

    fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in
            std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{} 可读: {e}", dir.display()))
        {
            let path = entry.expect("目录项可读").path();
            if path.is_dir() {
                collect_sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "zypui") {
                out.push(path);
            }
        }
    }

    // 结构迁到 `.zypui` 之后，tr 的调用点有两处形状：
    // - Rust 侧（数据面 / 绑定）：`g.tr("key")`；
    // - `.zypui` 模板：`tr('key')`。
    // 两处都扫 —— 键集合与写法无关，漏翻不分语言。
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let frontend = Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend");
    let mut files = Vec::new();
    collect_sources(&root, &mut files);
    collect_sources(&frontend, &mut files);

    // **跳过注释行**：这条门禁扫的是源码文本，而注释里写「源码里每个
    // `g.tr("…")`」这类句子是正当的 —— 不跳过的话，门禁会把注释里的
    // 省略号当成键名报出来（真实发生过一次：`mod.rs` 的门禁说明本身
    // 让这条门禁红了）。
    let mut used: Vec<String> = Vec::new();
    let scan = |line: &str, pattern: &str, quote: char, used: &mut Vec<String>| {
        for (i, _) in line.match_indices(pattern) {
            let rest = &line[i + pattern.len()..];
            let Some(end) = rest.find(quote) else {
                continue;
            };
            let key = &rest[..end];
            if !used.iter().any(|k| k == key) {
                used.push(key.to_string());
            }
        }
    };
    for file in &files {
        let mut text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("{} 可读: {e}", file.display()));
        // `.zypui` 的注释是 XML 块注释（`<!-- … -->`，可跨多行）—— 先整体
        // 剥掉再逐行扫；续行不以 `<!--` 开头，逐行跳过挡不住它。
        if file.extension().is_some_and(|e| e == "zypui") {
            while let Some(start) = text.find("<!--") {
                match text[start..].find("-->") {
                    Some(end) => text.replace_range(start..start + end + 3, ""),
                    None => break,
                }
            }
        }
        for line in text.lines() {
            let t = line.trim_start();
            if t.starts_with("//") || t.starts_with("<!--") || t.starts_with("-->") {
                continue;
            }
            scan(line, "g.tr(\"", '"', &mut used);
            scan(line, "tr('", '\'', &mut used);
        }
    }
    assert!(
        !used.is_empty(),
        "一个 `g.tr(\"…\")` 都没扫到 —— 取文案的写法变了，这条门禁在假装通过"
    );

    let msgs = messages();
    let mut problems: Vec<String> = Vec::new();
    for key in &used {
        for lang in SUPPORTED {
            if !msgs.missing_in(lang, &[key.as_str()]).is_empty() {
                problems.push(format!("源码用到但表里缺：{key} / {lang}"));
            }
        }
    }
    for key in catalog_keys() {
        for lang in SUPPORTED {
            if !msgs.missing_in(lang, &[key]).is_empty() {
                problems.push(format!("表内键缺译：{key} / {lang}"));
            }
        }
    }
    // 顺手拦住「把键名抄进了文案栏」：`tr()` 的回退行为会让这种抄错静默显示。
    assert!(
        used.len() <= 500,
        "扫到 {} 个 tr 键，数量对不上 —— 扫描把别处的字符串也算进来了？",
        used.len()
    );
    assert!(
        problems.is_empty(),
        "漏加条目会让画面上直接显示键名（不崩、不空，所以肉眼最容易漏）：\n  {}",
        problems.join("\n  ")
    );
}

/// 系统语言**不得**盖过用户的手动选择。
///
/// # 这条钉住的是「在设置里点中文没反应」
///
/// 宿主每帧都把系统语言偏好传进来（[`InputState::languages`]）。而协商
/// **可能得到空表** —— 宿主给的语言串不是合法 BCP 47 标签时（平台层
/// 误给 LCID 串时就是如此），`Preferences::from_owned` 会把每一项都丢掉。
///
/// 判据若写成「`prefs` 是空表 ⇒ 还没协商过」，那么**每帧**都会重跑协商，
/// 而每帧的结果是「回落 [`SUPPORTED`] 第一项」⇒ 用户刚点的语言在同一帧
/// 末尾被改回去 ⇒ 看起来「点击完全没反应」。
///
/// 症状之所以难查：点击**真的**生效了（`lang` 一度变成 1），只是同一次
/// `on_input` 调用的末尾又被改回 0 —— 于是回读状态时看到的永远是旧值。
#[test]
fn a_manual_language_choice_survives_the_system_negotiation() {
    use zephyra_core::id::ElementKey;
    use zephyra_fx::InputState;

    // 平台层误给 LCID 串时的真实形状（实测本机 `GetUserPreferredUILanguages`
    // 传错标志位就得到这两个）。它们**不是**合法 BCP 47 标签 ⇒ 协商得到空表。
    let broken_system_langs = vec!["0804".to_string(), "0409".to_string()];

    let mut g = Gallery::default();
    let frame = |pressed: Option<ElementKey>| {
        let pointers = usize::from(pressed.is_some());
        InputState {
            hovered: Some(ElementKey::Name("zh-Hans")),
            pressed,
            pointers,
            viewport: (1400.0, 800.0),
            scale: 1.0,
            dt_ms: 16.0,
            languages: broken_system_langs.clone(),
            ..InputState::default()
        }
    };

    // 首帧：协商（空表 ⇒ 回落英文）
    g.on_input(&frame(None));
    assert_eq!(g.locale(), "en", "空表时应当回落第一项");

    // 按下 → 抬起：抬起那一帧 click 生效
    g.on_input(&frame(Some(ElementKey::Name("zh-Hans"))));
    g.on_input(&frame(None));
    assert_eq!(
        g.lang, 1,
        "点「中文」后语言必须是 zh-Hans —— 被系统协商改回去了"
    );
    assert_eq!(g.locale(), "zh-Hans");

    // 后续帧继续带着系统语言：手动选择必须活下来
    for _ in 0..5 {
        g.on_input(&frame(None));
    }
    assert_eq!(
        g.lang, 1,
        "系统语言不得在后续帧把手动选择改回去 —— 协商必须只跑一次"
    );
    assert_ne!(
        g.tr("route.guide"),
        "Quick start",
        "界面文案必须真的切到中文，而不只是状态字段变了"
    );
}

/// 命令行 `--lang` 指定的语言必须**活过**系统的首次协商。
///
/// # 这条钉的是一个真实 bug
///
/// `--lang` 的文档写的是「强制语言」，但实现只是 `view.lang = idx` ——
/// 而窗口路第一帧的 `on_input` 会做**系统语言协商**，把 `lang` 改回系统
/// 那个。本机系统语言是 zh-Hans，于是 `--lang en` 也显示中文。
///
/// 症状难查的原因：**在系统语言恰好就是目标语言的机器上完全看不出来**。
/// 而 `--lang en` / `--lang ja` 的用途正是「在本机（中文）看另一种语言」
/// —— 也就是说这个 bug 恰好在它唯一有用的场景里发作。
///
/// # 为什么钉在**库**这一层而不是命令行那一层
///
/// 命令行是二进制，门禁 crate 引不到它。把「强制语言」收成
/// [`Gallery::force_language`]（它同时置「已协商」），不变量就住在库里 ——
/// 命令行只能调它，于是没有第二条路可走。
#[test]
fn a_forced_language_survives_the_system_negotiation() {
    use zephyra_fx::InputState;

    // 本机真实形状：系统语言是简体中文。
    let system = vec!["zh-Hans".to_string()];

    let mut g = Gallery::default();
    g.force_language(0); // 强制英文
    assert_eq!(g.locale(), "en", "强制语言没生效");

    // 首帧：系统语言到了。若不钉「已协商」，这里就会把英文改回中文。
    g.on_input(&InputState {
        hovered: None,
        pressed: None,
        pointers: 0,
        viewport: (1400.0, 800.0),
        scale: 1.0,
        dt_ms: 16.0,
        languages: system.clone(),
        ..InputState::default()
    });
    assert_eq!(
        g.locale(),
        "en",
        "系统协商把 `--lang` 指定的语言改回去了 —— 强制语言没生效"
    );
    assert_eq!(
        g.tr("route.guide"),
        "Quick start",
        "界面文案必须真的是英文，而不只是状态字段变了"
    );

    // 再来几帧：协商只跑一次，之后也不能翻。
    for _ in 0..3 {
        g.on_input(&InputState {
            languages: system.clone(),
            dt_ms: 16.0,
            ..InputState::default()
        });
    }
    assert_eq!(g.locale(), "en", "后续帧把强制语言翻回去了");
}

/// 系统语言**只**协商一次；协商失败也不重跑。
///
/// 与上一条互补：上一条钉「手动选择活下来」，这条钉「协商只发生一次」——
/// 后者是前者的**前提**（协商若每帧重跑，第一次失败后仍会不停重来）。
///
/// # 为什么这条要单独钉
///
/// 协商的结果被拿去做「初始语言」，而初始语言只在应用刚起来时有意义。
/// 每帧重跑它 = 每帧都在重新决定界面语言，于是任何一帧的偶然输入都可能
/// 把界面语言改回去 —— 即便协商**成功**（表非空）也一样。
#[test]
fn the_system_language_is_negotiated_at_most_once() {
    use zephyra_fx::InputState;

    let mut g = Gallery::default();
    let frame = |langs: Vec<String>| InputState {
        hovered: None,
        pressed: None,
        pointers: 0,
        viewport: (1400.0, 800.0),
        scale: 1.0,
        dt_ms: 16.0,
        languages: langs,
        ..InputState::default()
    };

    // ⚠️ 用**真机形状**：`GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, …)`
    // 交回的是**区域**标签（实测本机 `["zh-CN", "en-US"]`），而应用声明的
    // 可用语言写的是**文字**子标签（`zh-Hans`）。用 `["zh-Hans", …]` 写这条
    // 会绕过「区域标签 ↔ 文字标签」这一跳 —— 而那正是线上真实走的那一跳。
    let real_system_langs = vec!["zh-CN".to_string(), "en-US".to_string()];

    // 合法标签：协商成功 ⇒ 初始语言是中文（`zh-CN` 落到 `zh-Hans`）
    g.on_input(&frame(real_system_langs.clone()));
    assert_eq!(
        g.locale(),
        "zh-Hans",
        "系统偏好 zh-CN 时初始语言应当是 zh-Hans"
    );
    assert!(g.lang_negotiated, "协商后必须标记成「已做过」");

    // 用户切到英文
    g.click("en");
    assert_eq!(g.locale(), "en");

    // 后续帧继续带着系统语言：不得改回去
    for _ in 0..5 {
        g.on_input(&frame(real_system_langs.clone()));
    }
    assert_eq!(
        g.locale(),
        "en",
        "协商成功后，后续帧也不得重跑协商 —— 否则界面语言会被系统偏好反复改回"
    );
}

/// 控件参考页的**族切换**必须接线：两个 segmented key 都在树上，且真的点了
/// 都会改 `Gallery::ref_family`。
///
/// # 为什么取代了旧的 `segment_keys_are_all_wired`
///
/// 参考族不再是路由（参考层收进 `reference` 页内 segmented 切换），旧的
/// `seg:grid` / `seg:list` / `seg:card` 分段随 toggles 路由一起消失。
/// 新的分段是 `ref:buttons` / `ref:checkbox` 两族，走 [`crate::app::REF_FAMILIES`]
/// 查表 —— 加一族只改那张表，而这条门禁**真的点一遍**证明接线。
/// （液态玻璃 `ref:effects` 已提升为一等路由 `glass`，不在此表中。）
///
/// # 为什么不能只查 key 在树上
///
/// 「key 在树上」只证明按钮**画出来了**；分派是运行期字符串解析
/// （剥 `ref:` 前缀 → 查表），编译器看不出「面板发的 key 与查表认识的 key
/// 对不上」。所以这里真的点，然后回读 `ref_family`。
#[test]
fn reference_family_keys_all_wired() {
    let root = laid_out("reference");
    // 每个 key 都在树上（否则是「click 里有、页面上没有」，症状相反：
    // 点了对的事，但界面上没有那个东西）。
    for fam in REF_FAMILIES {
        let key = format!("ref:{fam}");
        assert!(
            find_by_key(&root, &key).is_some(),
            "参考页上找不到族切换按钮 {key} —— REF_FAMILIES 与 pages::reference 对不上"
        );
    }

    // 逐个点，回读 ref_family。
    for (i, fam) in REF_FAMILIES.iter().enumerate() {
        let mut g = Gallery::default();
        g.route = "reference";
        g.click(&format!("ref:{fam}"));
        assert_eq!(
            g.ref_family, i,
            "点 `ref:{fam}` 后 ref_family 应是 {i} —— 分派漏了这个 key 或查表错位"
        );
    }
}

/// 参考页必须**族齐全**：每一族的树里都有该族的特征控件。
///
/// # 恒真防御
///
/// 每族先断言树上确实有内容（节点数下界），否则某族 `page()` 改名后
/// 这条会因为「全部回落到 basics」而绿 —— 那是**布局回退**，不是族齐全。
#[test]
fn reference_page_has_all_families() {
    // ⚠ 这个数字是**有意写死**的：族数变化时它必须红一次，逼人回来确认
    // 「新族的内容真的塞进了一屏」（参考页无滚动，装不下就是裁切）。
    // 改成 `REF_FAMILIES.len()` 会让门禁失去作用 —— 它要防的正是「表改了、
    // 内容没跟上」。
    assert_eq!(
        REF_FAMILIES.len(),
        3,
        "参考页当前是**三族**（Button 族、复选框族与开关族）—— 加族时同步改这里的数字，\
         并确认新族一屏装得下"
    );
    for (i, fam) in REF_FAMILIES.iter().enumerate() {
        let mut g = Gallery::default();
        g.route = "reference";
        g.ref_family = i;
        let theme = Theme::builtin(ThemeVariant::Dark);
        let ctx = Context::new(&theme);
        let element = g.view(&ctx);
        let root = Node::from_element(&element);
        // 每族树里必须有该族的特征 key —— 特征 key 是各族组件里第一个
        // 可辨识控件的 key，改控件时同步改这里。
        let probe = match *fam {
            "buttons" => "act:standard",
            "checkbox" => "group:checks",
            "switches" => SWITCH_DEMO_KEY,
            _ => panic!("未知族 {fam}"),
        };
        assert!(
            find_by_key(&root, probe).is_some(),
            "参考页第 {i} 族（{fam}）的树里找不到特征控件 {probe} —— 该族内容没接进 reference"
        );
    }
}
// ------------------------------------------- 门禁 21：响应式（官方断点 + 列数）

/// 断点判据必须与官方文档的三个类别一致。
///
/// 出处：`screen-sizes-and-breakpoints-for-responsive-design` ——
/// 「小：最大可达 640 像素；中：641 - 1007px；大：1008px及以上」。
/// 数字本身在 `responsive.rs` 里有单测，这里额外钉住**汇编关系**：
/// 类别必须由宽度单调决定，不能出现「宽窗口落到更窄的档」这种倒挂。
#[test]
fn breakpoint_categories_match_doc_and_are_monotonic() {
    use zephyra_core::responsive::WidthClass;

    let mut prev = WidthClass::Small;
    for w in (0..2600).step_by(40) {
        let c = WidthClass::of_effective_width(w as f32);
        // 宽度只增，档位只能不变或更宽。
        let order = |c: WidthClass| match c {
            WidthClass::Small => 0,
            WidthClass::Medium => 1,
            WidthClass::Large => 2,
        };
        assert!(
            order(c) >= order(prev),
            "宽度 {w} 的档位 {c:?} 窄于上一档 {prev:?} —— 断点倒挂了"
        );
        prev = c;
    }
}

/// 列数必须真的随宽度变，且基准画布下仍是 5 列。
///
/// # 为什么这三条断言都要有
///
/// 1. **基准 5 列**：不能为了响应式把 1400×800 的参考外观改掉；
/// 2. **窄窗口降列**：否则窄窗口下仍排 5 列，内容溢出被裁（本轮实测的症状）；
/// 3. **宽窗口加列**：否则宽窗口下仍排 5 列，右侧留白（本轮实测的另一个症状）。
///
/// 三条一起才构成「列数是宽度的函数」。只有第 1 条时，代码完全可以
/// 退回写死 5 —— 基准图全绿，而真实窗口下症状原样存在。
#[test]
fn column_count_tracks_width_and_baseline_is_five() {
    use zephyra_core::responsive::WidthClass;
    use zephyra_gallery::app::{Gallery, BASE_VIEWPORT};

    let at = |w: u32, h: u32, scale: f32| {
        let mut g = Gallery::default();
        g.viewport = (w as f32, h as f32);
        g.scale = scale;
        (g.width_class(), g.card_columns())
    };

    // 基准：有效 1120 epx ⇒ 大档，5 列。这一条把「参考图」钉住。
    let (cls, cols) = at(BASE_VIEWPORT.0, BASE_VIEWPORT.1, 1.0);
    assert_eq!(cls, WidthClass::Large, "基准应落在官方的大档");
    assert_eq!(cols, BASE_COLUMNS, "基准画布下必须仍 5 列");

    // 窄：有效宽度 ≤ 640 ⇒ 小档，且列数必须严格少于基准。
    //
    // 取 700 物理 @1.25 = 560 epx（小）。注意这里用**物理**视口喂进去，
    // 由 Gallery 自己除 scale —— 判据必须是有效像素，官方
    // `AdaptiveTrigger.MinWindowWidth` 的定义就是 "in effective pixels"。
    let (cls, cols) = at(700, 600, 1.25);
    assert_eq!(cls, WidthClass::Small);
    assert!(
        cols < BASE_COLUMNS,
        "窄窗口下仍排 {cols} 列 —— 内容会溢出被裁"
    );
    assert!(
        gallery_at(700, 600, 1.25).is_compact(),
        "小档必须判定为紧凑"
    );

    // 宽：有效 ≥ 1008 ⇒ 大档，且列数必须多于基准。
    let (cls, cols) = at(2400, 1400, 1.0);
    assert_eq!(cls, WidthClass::Large);
    assert!(cols > BASE_COLUMNS, "宽窗口下仍排 {cols} 列 —— 右侧会留白");

    // 单调：宽度只增，列数只能不变或增。中途允许"不变"（步长小于一个卡宽），
    // 但不允许下降 —— 下降意味着列数受别的因素影响，难以归因。
    let mut prev = 0usize;
    for w in (600..2600u32).step_by(120) {
        let cols = gallery_at(w, 900, 1.0).card_columns();
        assert!(cols >= prev, "宽度 {w} 的列数 {cols} 少于上一档 {prev}");
        prev = cols;
    }
}

fn gallery_at(w: u32, h: u32, scale: f32) -> zephyra_gallery::app::Gallery {
    let mut g = zephyra_gallery::app::Gallery::default();
    g.viewport = (w as f32, h as f32);
    g.scale = scale;
    g
}

/// 内容区可用宽度必须 = 视口 − 导航 − 页边距，且紧凑档下导航归零。
///
/// 这是列数公式的输入。若它与外壳常量脱节，列数会悄悄算错 —— 而症状
/// （最后一列被裁/多出一列空白）看起来像"样式没调好"，不像算错。
#[test]
fn content_width_follows_breakpoint() {
    let wide = gallery_at(1400, 800, 1.0);
    assert_eq!(
        wide.content_width(),
        1400.0 - zephyra_gallery::app::shell::NAV_W - 28.0,
        "非紧凑档：内容区 = 视口 − 导航 − 页边距"
    );
    assert!(!wide.is_compact());

    // 折叠真的把导航宽让给内容：紧凑档的内容区 = 视口 - 页边距（导航归零），
    // 于是与非紧凑公式（视口 - 导航 - 页边距）之差恰为 NAV_W。
    //
    // 判据在**同一个**紧凑视口下取两个值：真实的一个（导航已归零），
    // 与按非紧凑公式手算的一个。不能靠"切换到另一个 viewport"来对比 ——
    // 那样差里混着窗口宽度的变化。（第一版正是这么写的，两次断言都假。）
    let mut compact = Gallery::default();
    compact.viewport = (700.0, 600.0); // 700 @1.25 = 560 epx => 官方小档
    compact.scale = 1.25;
    assert!(compact.is_compact());
    let collapsed = compact.content_width();
    let nav = zephyra_gallery::app::shell::NAV_W;
    let gutter = zephyra_gallery::app::PAGE_GUTTER;
    assert_eq!(collapsed, 700.0 - gutter, "紧凑档内容区 = 视口 - 页边距");
    assert_eq!(
        collapsed - (700.0 - nav - gutter),
        nav,
        "折叠前后的内容区差必须恰好是导航宽（折叠后更大）"
    );
    assert!(
        collapsed > 0.0 && collapsed.is_finite(),
        "内容区必须是有界正数"
    );
}

/// 触控目标必须按 scale 换算，保证视觉上达到官方的 40 epx 下限。
///
/// 出处：`touch-interactions`「命中目标」——「可触摸定义为最小 40 x 40 epx」。
/// 本仓布局单位是物理像素，所以 40 必须乘 scale；不乘会得到 40 物理 = 32 epx
/// （1.25 缩放下），**低于官方下限而代码看起来完全正确**。
#[test]
fn touch_target_converts_to_official_ephemeral_px() {
    let g = gallery_at(1400, 800, 1.25);
    assert_eq!(
        g.touch_target(zephyra_core::responsive::TOUCH_MIN_EPX),
        50.0
    );
    assert_eq!(
        g.touch_target(zephyra_core::responsive::TOUCH_OPTIMAL_EPX),
        55.0
    );
    // 100% 下恒等；退化 scale 不应 panic 或给出 0。
    let plain = gallery_at(1400, 800, 1.0);
    assert_eq!(
        plain.touch_target(zephyra_core::responsive::TOUCH_MIN_EPX),
        40.0
    );
    let bad = gallery_at(1400, 800, 0.0);
    assert!(bad.touch_target(zephyra_core::responsive::TOUCH_MIN_EPX) > 0.0);
}

// --------------------------------------------------- 门禁 26：折射参数可调

use zephyra_gallery::app::glass::{
    Glass, GlassMode, Param, BEZEL, BEZEL_TEXT, DISPERSION, DISPERSION_TEXT, IOR, IOR_TEXT,
    THICKNESS, THICKNESS_TEXT,
};

/// 收集**三块玻璃块**上的滤镜（两种算法都收）。
///
/// # 为什么只收 `glass:*` 而不是整树
///
/// 开关的玻璃把手（`LiquidToggle` 1:1 复刻）也带一条 `Refraction` 滤镜
/// （`knob_filter` 绑定，strength 随按压进度变化）—— 整树收会把把手的
/// 滤镜数进来，`pick(0, …)` 就指到把手而不是玻璃块，这条门禁原本钉的是
/// 「参数面板真的在改玻璃块」，把手是另一条通道。
///
/// # 为什么三种算法都要收
///
/// 玻璃默认跑**原版透镜**（[`GlassMode::Original`] ⇒ [`Filter::Lens`]），
/// 便宜路走 `PixelOffset`、斯涅尔路走 `Refraction`。只收其中一种的话，
/// 这条门禁在别的模式下会因为「树上一块匹配滤镜都没有」而失败 —— 而它要
/// 钉的恰恰是「每种算法都真的可调」。
///
/// # 为什么读 `iter_local` 而不是 `get`
///
/// `Node::from_element` 之后样式表还没应用时，树上的属性全是**局部**的；这里
/// 先取局部值，正是为了在样式表**不含** `filter:` 的前提下读到源码写的值。
/// 若改用 `get`（查合成后的值），这条门禁会随「样式表里又加回了一档 filter」
/// 而悄悄改成测样式表 —— 那正是它要防的回归。
fn refraction_filters(root: &Node) -> Vec<zephyra_core::filter::Filter> {
    let mut out = Vec::new();
    walk(root, &mut |n: &Node| {
        // 只看玻璃块 —— 把手（`knob`）的 Refraction 不在这个契约里。
        let is_glass_pane = n
            .key()
            .and_then(|k| k.as_name())
            .is_some_and(|k| k.starts_with("glass:"));
        if !is_glass_pane {
            return;
        }
        for (k, v) in n.props().iter_local() {
            if !std::ptr::eq(k, &zephyra_core::props::FILTER) {
                continue;
            }
            if let zephyra_core::property::PropertyValue::Filter(chain) = v {
                for f in chain.filters() {
                    if matches!(
                        f,
                        zephyra_core::filter::Filter::Refraction { .. }
                            | zephyra_core::filter::Filter::PixelOffset { .. }
                            | zephyra_core::filter::Filter::Lens { .. }
                    ) {
                        out.push(*f);
                    }
                }
            }
        }
    });
    out
}

/// 换一组参数重跑整棵树，返回该树上的折射参数。
///
/// ⚠ 走的是**真的** [`Gallery::view`] + `Node::from_element`，不是直接问
/// `Glass::filter()` —— 后者只会证明查表函数自己对自己成立，而门禁要证明的是
/// 「树上那个值」跟着参数变了。
fn filters_at(g: &Gallery) -> Vec<zephyra_core::filter::Filter> {
    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let element = g.view(&ctx);
    let root = Node::from_element(&element);
    refraction_filters(&root)
}

/// 参数的四个字段必须随 `Gallery::glass` 变 —— **这是「可调」的定义**。
///
/// # 为什么这条必须按**四张表**逐张试，而不是「改一个字段看树变没变」
///
/// 只试一个字段的话，另三个字段写死在源码里这条门禁照样绿，而画面上
/// 「IOR 那一栏的加减按钮点了没反应」。四个字段各试一次，四处死值全被覆盖。
///
/// # 恒真防御
///
/// 每种参数都要先断言**树里确实有一块玻璃**（`!got.is_empty()`），否则下面
/// 那句 `assert_ne!` 在「玻璃块根本没造出来」时会因为两边都空而绿。
#[test]
fn glass_params_are_not_hard_coded() {
    use zephyra_core::filter::Filter;

    /// 把某个 `Filter::Refraction` 的四个字段抠出来。
    fn parts(f: &Filter) -> [f32; 4] {
        match f {
            Filter::Refraction {
                strength,
                bezel,
                dispersion,
                ior,
            } => [*strength, *bezel, *dispersion, *ior],
            other => panic!("树上出现了非折射的滤镜 {other:?}"),
        }
    }

    let pick = |idx: usize, v: &Vec<Filter>| parts(&v[idx]);

    // ---- 基准树 ----
    //
    // ⚠ **四参数的契约只在物理折射那条路上成立**：便宜路（`PixelOffset`）
    // 只有 `amount` / `bezel` 两个字段，色散与折射率在那边没有对应物。
    // 所以这一段显式切到 `Physical`（默认是 `Original`）。便宜路那两个字段
    // 由 `cheap_glass_mode_moves_amount_and_bezel` 单独钉。
    let mut base = Gallery::default();
    base.route = "materials";
    base.glass.mode = GlassMode::Physical;
    let warm = filters_at(&base);
    assert!(
        warm.len() >= 3,
        "液态玻璃页上只有 {} 块玻璃带折射滤镜 —— 这条门禁可能恒真",
        warm.len()
    );
    let b = pick(0, &warm);

    // ---- 逐个参数各挪一档 ----
    for param in Param::ALL {
        let mut g = Gallery::default();
        g.route = "materials"; // 材质专区（与液态玻璃合并）是一等路由
        g.glass.mode = GlassMode::Physical;
        assert!(
            g.glass.nudge(param, 1),
            "默认档位下 `{param:?}` 加一档被 clamp 掉了 —— 表的下界取得不对"
        );
        let moved = filters_at(&g);
        assert!(
            !moved.is_empty(),
            "改了 `{param:?}` 之后玻璃块从树上消失了 —— 判据本身失效"
        );
        let m = pick(0, &moved);
        let bi = param.index();
        for (j, (x, y)) in b.iter().zip(m.iter()).enumerate() {
            if j == bi {
                assert_ne!(
                    x, y,
                    "`{param:?}` 加一档后树上的这个字段（{x} → {y}）没变 —— \
                     该参数被写死在源码里了，页面上的 [+] 是摆设"
                );
            } else {
                // 其余三个必须**原样不动**。反向判据同样必要：只断「该变的变了」
                // 挡不住 `nudge` 里错位写到别人字段上的形状，而那种形状在画面上
                // 是「点厚度、IOR 跳了」。
                assert_eq!(
                    x, y,
                    "`{param:?}` 加一档后第 {j} 个字段也跟着变了（{x} → {y}）—— \
                     nudge 写错了字段"
                );
            }
        }
        // 三块玻璃共用**除厚度以外**的参数（倒角带宽 / 色散 / 折射率必须**逐项
        // 相同**）；而**厚度**必须**各不相同**且**有序**（薄 < 常规 < 厚）——
        // 标题写着「薄 / 常规 / 厚」，三块一样厚的话那三个标题在**说谎**。
        //
        // ⚠ 这两条一起断，缺一不可：只断「厚度不同」挡不住「厚度不同、但倒角
        // 带宽各写各的」；只断「其余三项相同」挡不住「三块厚度一样」。
        let p0 = pick(0, &moved);
        for k in 1..moved.len() {
            let pk = pick(k, &moved);
            for j in 1..4 {
                assert_eq!(
                    p0[j], pk[j],
                    "`{param:?}` 改动后第 {k} 块玻璃的第 {j} 个字段与第 0 块不一致 \
                     （{} vs {}）—— 倒角带宽 / 色散 / 折射率是三块**共用**的",
                    p0[j], pk[j]
                );
            }
            assert!(
                p0[0] < pk[0],
                "第 {k} 块玻璃的厚度（{}）必须比第 0 块（{}）大 —— \
                 三块要按「薄 / 常规 / 厚」**有序**",
                pk[0],
                p0[0]
            );
        }
    }

    // ---- 步进必须**可逆**：四个各加一档再各减一档，回到基准值 ----
    //
    // 判据是「加完减完回到原处」，不是「减一档等于基准」—— 后者只在从基准
    // 减起时成立，而这里真正的风险是 `nudge` 把加减**同一个方向**实现成两次
    // 加（那种形状下每条单参数门禁都绿，而画面上 `[−]` 与 `[+]` 效果一样）。
    let mut round = Gallery::default();
    round.route = "materials"; // 材质专区（与液态玻璃合并）是一等路由
    round.glass.mode = GlassMode::Physical;
    for param in Param::ALL {
        assert!(round.glass.nudge(param, 1), "`{param:?}` 加一档被 clamp 了");
    }
    for param in Param::ALL.iter().rev() {
        assert!(
            round.glass.nudge(*param, -1),
            "`{param:?}` 加了一档却减不回来 —— 步进不可逆"
        );
    }
    assert_eq!(
        round.glass,
        Glass {
            mode: GlassMode::Physical,
            ..Glass::default()
        },
        "四个参数各加一档再各减一档后没回到默认值"
    );
    assert_eq!(
        pick(0, &filters_at(&round)),
        b,
        "加减一轮之后树上的折射参数没回到基准值"
    );
}

/// 便宜路（`Filter::PixelOffset`）的两个字段也必须随参数变。
///
/// # 为什么单列一条
///
/// [`glass_params_are_not_hard_coded`] 断的是物理折射的四参数契约，而便宜路
/// （切到它之后）树里是 `PixelOffset { amount, bezel }`。两个字段分别由
/// `Thickness` / `Bezel` 两个档位驱动（`Dispersion` / `Ior` 在便宜路上没有
/// 对应物，改了也只改树上那两个字段）。
#[test]
fn cheap_glass_mode_moves_amount_and_bezel() {
    use zephyra_core::filter::Filter;

    fn amount_bezel(f: &Filter) -> (f32, f32) {
        match f {
            Filter::PixelOffset { amount, bezel } => (*amount, *bezel),
            other => panic!("便宜路应当是像素偏移玻璃，树上却是 {other:?}"),
        }
    }

    let mut base = Gallery::default();
    base.route = "materials";
    base.glass.mode = GlassMode::Cheap;
    let warm = filters_at(&base);
    assert!(
        warm.len() >= 3,
        "液态玻璃页上只有 {} 块玻璃 —— 这条门禁可能恒真",
        warm.len()
    );
    let b = amount_bezel(&warm[0]);

    for (param, changes_amount) in [(Param::Thickness, true), (Param::Bezel, false)] {
        let mut g = Gallery::default();
        g.route = "materials";
        g.glass.mode = GlassMode::Cheap;
        assert!(g.glass.nudge(param, 1), "`{param:?}` 加一档被 clamp 了");
        let m = amount_bezel(&filters_at(&g)[0]);
        if changes_amount {
            assert_ne!(b.0, m.0, "`{param:?}` 加一档后 `amount` 没变 —— 被写死了");
            assert_eq!(b.1, m.1, "`{param:?}` 加一档后 `bezel` 也跟着变了");
        } else {
            assert_ne!(b.1, m.1, "`{param:?}` 加一档后 `bezel` 没变 —— 被写死了");
            assert_eq!(b.0, m.0, "`{param:?}` 加一档后 `amount` 也跟着变了");
        }
    }

    // 便宜路没有色散 / 折射率：那两个旋钮在树上**不该**产生第三、第四个字段，
    // 但它们也不该把已有字段改坏。
    for param in [Param::Dispersion, Param::Ior] {
        let mut g = Gallery::default();
        g.route = "materials";
        g.glass.mode = GlassMode::Cheap;
        g.glass.nudge(param, 1);
        assert_eq!(
            amount_bezel(&filters_at(&g)[0]),
            b,
            "`{param:?}` 在便宜路上没有对应物，却改了 `amount` / `bezel`"
        );
    }
}

/// 原版液态玻璃（`Filter::Lens`）的两个物理量必须随参数变，且它是默认算法。
///
/// 这条钉住用户反馈的修复：玻璃材质控件的默认观感必须和 AndroidLiquidGlass
/// 的 `lens()` 一致（背景被掰弯、角部 7 通道分色），而不是没有背景折射的
/// 像素偏移或整圈彩虹的斯涅尔均匀色散。
#[test]
fn original_glass_mode_moves_refraction_params() {
    use zephyra_core::filter::Filter;

    fn amount_height(f: &Filter) -> (f32, f32) {
        match f {
            Filter::Lens {
                refraction_height,
                refraction_amount,
            } => (*refraction_amount, *refraction_height),
            other => panic!("默认档应当是原版透镜玻璃，树上却是 {other:?}"),
        }
    }

    let mut base = Gallery::default();
    base.route = "materials";
    assert_eq!(
        base.glass.mode,
        GlassMode::Original,
        "玻璃的默认算法必须是原版液态玻璃（见 `GlassMode` 的文档）"
    );
    let warm = filters_at(&base);
    assert!(
        warm.len() >= 3,
        "液态玻璃页上只有 {} 块玻璃 —— 这条门禁可能恒真",
        warm.len()
    );
    let b = amount_height(&warm[0]);

    for (param, changes_amount) in [(Param::Thickness, true), (Param::Bezel, false)] {
        let mut g = Gallery::default();
        g.route = "materials";
        assert!(g.glass.nudge(param, 1), "`{param:?}` 加一档被 clamp 了");
        let m = amount_height(&filters_at(&g)[0]);
        if changes_amount {
            assert_ne!(b.0, m.0, "`{param:?}` 加一档后 `refraction_amount` 没变");
            assert_eq!(b.1, m.1, "`{param:?}` 加一档后 `refraction_height` 也变了");
        } else {
            assert_ne!(b.1, m.1, "`{param:?}` 加一档后 `refraction_height` 没变");
            assert_eq!(b.0, m.0, "`{param:?}` 加一档后 `refraction_amount` 也变了");
        }
    }

    // 透镜的分色强度是 shader 里按 `(cx·cy)/(hx·hy)` 写死的，没有面板旋钮。
    for param in [Param::Dispersion, Param::Ior] {
        let mut g = Gallery::default();
        g.route = "materials";
        g.glass.nudge(param, 1);
        assert_eq!(
            amount_height(&filters_at(&g)[0]),
            b,
            "`{param:?}` 在原版透镜里没有对应物，却改了折射参数"
        );
    }
}

/// 数值表与文本表必须**逐项对齐**。
///
/// # 为什么不靠 `format!` 生成文本、而是手写两张表
///
/// 因为每帧 `format!` 会走 `Box::leak`（见 `pages::basics::spin_text` 的理由），
/// 而这一页的参数每帧都要显示。手写两张表的代价就是**可能对不齐**，所以这条
/// 门禁是它的唯一保险。
///
/// # 判据为什么不是「文本能被解析回数值」
///
/// 那需要 `str::parse`，而这里要断的是**下标对应关系**：`DISPERSION_TEXT[7]`
/// 必须是 `DISPERSION[7]` 的显示，不是「某个能被解析的数」。所以逐下标比
/// `(值, 文本)` 对，并对不上的下标直接点名。
#[test]
fn glass_param_tables_line_up() {
    macro_rules! check {
        ($name:literal, $vals:expr, $texts:expr) => {{
            let v: &[f32] = &$vals;
            let t: &[&str] = &$texts;
            assert_eq!(
                v.len(),
                t.len(),
                "{}：数值表 {} 项、文本表 {} 项 —— 下标会越界或错位",
                $name,
                v.len(),
                t.len()
            );
            for (i, (val, text)) in v.iter().zip(t.iter()).enumerate() {
                assert!(
                    !text.is_empty(),
                    "{} 第 {i} 项文本为空 —— 面板上会是一个空白",
                    $name
                );
                // 文本的数值必须与表里那一项一致（允许文本多带小数位）。
                let parsed: f32 = text.parse().unwrap_or_else(|e| {
                    panic!("{} 第 {i} 项文本 {text:?} 不是一个数：{e}", $name)
                });
                let tol = if parsed.abs() >= 10.0 { 0.01 } else { 0.006 };
                assert!(
                    (parsed - val).abs() <= tol,
                    "{} 第 {i} 项：文本 {text:?} 解析成 {parsed}，表里却是 {val}",
                    $name
                );
            }
            // 必须**严格递增** —— 步进靠的是下标 +1，若有两档同值，用户连点两下
            // 会看到数值不变，症状与「按钮没接线」无法区分。
            for w in v.windows(2) {
                assert!(
                    w[1] > w[0],
                    "{}：{} 之后是 {} —— 档位没有严格递增，点两下看起来像没反应",
                    $name,
                    w[0],
                    w[1]
                );
            }
        }};
    }

    check!("THICKNESS", THICKNESS, THICKNESS_TEXT);
    check!("BEZEL", BEZEL, BEZEL_TEXT);
    check!("DISPERSION", DISPERSION, DISPERSION_TEXT);
    check!("IOR", IOR, IOR_TEXT);

    // IOR 必须含 1.00（不折射的诊断档）与 1.50（普通冕牌玻璃）。
    // 少了 1.00，「公式被改坏了吗」这个判断在页面上就没法做。
    assert!(
        IOR.contains(&1.0),
        "IOR 表里没有 1.00 —— 少了「不折射」这条诊断档"
    );
    assert!(
        IOR.contains(&1.5),
        "IOR 表里没有 1.50 —— 普通冕牌玻璃那档没了"
    );
    // 倒角下限不得为 0（除零，见 `BEZEL` 的文档）。
    //
    // ⚠ 判据写成「**每一档**都不小于 2」而不是「第 0 档不小于 2」：后者 clippy
    // 会判成恒真（编译期已知 `BEZEL[0]` 的值），而前者在中间档被改成 0 时才
    // 说得清是哪一档出的问题。
    for (i, b) in BEZEL.iter().enumerate() {
        assert!(
            *b >= 2.0,
            "BEZEL 第 {i} 档是 {b} —— 小于 2 会让着色器里的 `max(color.g, 1.0)` \
             把整块玻璃都塞进强折射区"
        );
    }
    // 默认档必须落在表内，否则 `Glass::default()` 会越界 panic。
    let d = Glass::default();
    assert!(
        d.thickness < THICKNESS.len(),
        "默认厚度档 {} 越界",
        d.thickness
    );
    assert!(d.bezel < BEZEL.len(), "默认倒角档 {} 越界", d.bezel);
    assert!(
        d.dispersion < DISPERSION.len(),
        "默认色散档 {} 越界",
        d.dispersion
    );
    assert!(d.ior < IOR.len(), "默认 IOR 档 {} 越界", d.ior);
}

/// 面板上每个 `[−] [+] 都必须在树上，且点了真的改参数。
///
/// # 为什么不能只看「key 在树上」
///
/// 「key 在树上」只保证按钮**画出来了**；而 `click()` 的分派是**运行期字符串
/// 解析**（剥 `glass-` 前缀 → 找 `Param::slug()` → 看后缀），编译器看不出
/// 「面板发的是 `glass-thickness:dec` 而 `slug()` 返回 `thickness` 但中间少个
/// 冒号」。所以这里**真的点一遍**再回读参数。
///
/// # 两种算法各测一遍
///
/// 默认（原版透镜）与便宜路都只有 Amount / Bezel 两个旋钮 —— 色散与折射率
/// 在那两条路上**没有对应物**，面板不画它们。物理折射那条才是四行八键。
/// 少测一边就会出现「切到物理之后 `[+]` 是摆设」或「两旋钮模式下画了四个、
/// 两个点了没用」。
///
/// # 恒真防御
///
/// 先断言该模式下的按钮**个数**，少画了会在这里报，而不是静默通过。
#[test]
fn glass_param_keys_all_wired() {
    // ---- 便宜路：两行四键 ----
    let root = laid_out_materials();
    let cheap = [(Param::Thickness, "Amount"), (Param::Bezel, "Bezel")];
    for (param, label) in cheap {
        for suffix in ["dec", "inc"] {
            let k = format!("glass-{}:{suffix}", param.slug());
            assert!(
                find_by_key(&root, &k).is_some(),
                "便宜模式下找不到按钮 {k}（{label} 的 {suffix}）—— \
                 Param::ALL 与 effects.rs 的面板对不上"
            );
        }
    }
    // 反向：便宜模式**不该**画色散 / 折射率。
    for param in [Param::Dispersion, Param::Ior] {
        let k = format!("glass-{}:dec", param.slug());
        assert!(
            find_by_key(&root, &k).is_none(),
            "便宜模式下画出了 {k} —— 像素偏移没有色散 / 折射率，那个旋钮是摆设"
        );
    }

    // ---- 物理折射：四行八键 ----
    let phys = laid_out_materials_physical();
    let mut keys = Vec::new();
    for param in Param::ALL {
        keys.push(format!("glass-{}:dec", param.slug()));
        keys.push(format!("glass-{}:inc", param.slug()));
    }
    assert_eq!(keys.len(), 8, "四个参数 × 增减 = 8 个按钮");
    for k in &keys {
        assert!(
            find_by_key(&phys, k).is_some(),
            "物理折射下找不到按钮 {k} —— Param::ALL 与 effects.rs 的面板对不上"
        );
    }

    // 逐个点「加」，每次都要求参数真的动了。
    let mut g = Gallery::default();
    g.route = "materials"; // 材质专区（与液态玻璃合并）是一等路由
    for param in Param::ALL {
        let before = g.glass;
        g.click(&format!("glass-{}:inc", param.slug()));
        assert_ne!(
            g.glass,
            before,
            "点 `glass-{}:inc` 之后参数没变 —— 分派漏了这个 key",
            param.slug()
        );
    }
    // 再逐个点「减」，回到默认值。
    for param in Param::ALL.iter().rev() {
        g.click(&format!("glass-{}:dec", param.slug()));
    }
    assert_eq!(g.glass, Glass::default(), "加减一轮之后没回到默认值");
}

/// 节点上某个属性的**生效值**。
///
/// # 为什么不能只看 `iter_local`
///
/// 属性有**两层存储**：本地写死的（`iter_local`）与样式表给的（`iter_styled`）。
/// 样式表走的是 `set_styled_prop`，落在**另一层** —— 而材质页的纹理名与材质
/// 种类**全部**来自样式表，只看 local 的话它们一个都读不到。
///
/// 两层都查，且**本地优先**（本地是最高优先级）。
fn prop_of<'a>(
    node: &'a Node,
    key: &'static zephyra_core::property::PropertyKey,
) -> Option<&'a zephyra_core::property::PropertyValue> {
    node.props()
        .iter_local()
        .chain(node.props().iter_styled())
        .find(|(k, _)| std::ptr::eq(*k, key))
        .map(|(_, v)| v)
}

/// 照片板必须**真的挂着那张照片**。
///
/// # 为什么这条是整页的**前提**
///
/// 折射与磨砂读的是「**已经画在屏幕上**的东西」（离屏趟里的 `g_dst`）。
/// 照片没画进去 ⇒ 玻璃块背后是纯色 ⇒ 位移纯色 ⇒ 画面上「折射没生效」。
/// 而那不是着色器的问题，是**属性没接上**的问题 —— 排查方向会整个跑偏
/// （去查 HLSL、查滤镜链、查离屏尺寸），而真正的答案在一行样式里。
///
/// # 为什么查的是**属性**而不是画面
///
/// CPU 光栅**不采样纹理**（它只画图像的形状，见 `cpu_raster::draw_batch`），
/// 所以像素级的判据在离线路径上永远是「没有照片」。属性级的判据才是
/// 跨路径成立的 —— 它断的是「照片**有没有被声明**」，而真正有没有画出来
/// 由真 GPU 截图复核。
#[test]
fn the_photo_board_carries_the_texture() {
    let root = laid_out_materials();
    let board = find_by_key(&root, "material-board").expect("要有照片板");

    let texture: Option<String> =
        prop_of(board, &zephyra_core::props::IMAGE_TEXTURE).map(|v| match v {
            zephyra_core::property::PropertyValue::StaticStr(s) => (*s).to_string(),
            zephyra_core::property::PropertyValue::Str(s) => s.as_ref().to_string(),
            other => panic!("image-texture 的值不是字符串：{other:?}"),
        });
    assert_eq!(
        texture.as_deref(),
        Some(zephyra_gallery::app::MOUNTAIN_TEXTURE),
        "照片板没挂上背景纹理 —— 玻璃块背后是纯色，位移看不出来"
    );
}

/// 照片的宽高比必须与照片板一致 —— 否则照片铺满板时会被**拉扁**。
///
/// # 为什么这条值得单列
///
/// 照片按**节点矩形**铺满（`image-size` 默认 −1），而节点的宽高比由样式表写死。
/// 两者一旦对不上，照片就被非等比拉伸 —— 山会被压扁，而画面上**看不出**
/// 「被拉扁了」，只显得「这张照片怪怪的」。
///
/// 资源是**编译期内嵌**的 JPEG，它的长宽比与板的长宽比必须一致。
/// 换图时这条门禁会当场红，而不是让人在画面里找「哪里怪」。
#[test]
fn the_photo_aspect_matches_the_board() {
    let root = laid_out_materials();
    let board = find_by_key(&root, "material-board").expect("要有照片板");
    let b = board.rect();
    let board_aspect = b.width / b.height;

    let bytes = include_bytes!("../assets/mountain.jpg");
    // `include_bytes!` 给的是 `&[u8; N]`，而 `Decoder::new` 要 `&[u8]`（它按
    // `io::Read` 收）。
    let mut dec = jpeg_decoder::Decoder::new(&bytes[..]);
    dec.read_info().expect("资源是合法 JPEG");
    let info = dec.info().expect("资源有尺寸");
    let photo_aspect = info.width as f32 / info.height as f32;

    assert!(
        (board_aspect - photo_aspect).abs() < 0.02,
        "照片板宽高比 {board_aspect:.4} 与资源宽高比 {photo_aspect:.4} 差得太多 —— \
         照片铺满板时会被拉扁（{:.1}% 的非等比拉伸）",
        (board_aspect / photo_aspect - 1.0) * 100.0
    );
}

/// 三块材质样板必须**各自声明自己的材质**。
///
/// # 为什么这条不能靠「看画面」断
///
/// `material:` 拼错时**解析就会失败**（`zstyle_compile::value::enum_value` 只认
/// `solid` / `frosted` / `glass` 三个词），于是那条规则**整块被跳过** ——
/// 而画面上那块样板只是「没有磨砂」，不报错、不空白，看不出是拼错。
///
/// 顺带断掉「三块样板声明成同一种」：那会让「材质选型」这一整个专区
/// 变成同一块板子画三遍，而画面上确实看起来「三块一模一样」。
#[test]
fn the_material_swatches_declare_their_materials() {
    use zephyra_core::filter::Material;

    let root = laid_out_materials();
    let mut got: Vec<(String, Material)> = Vec::new();
    walk(&root, &mut |n: &Node| {
        let Some(key) = n.key().and_then(|k| k.as_name()) else {
            return;
        };
        if !key.starts_with("swatch:") {
            return;
        }
        let material = match prop_of(n, &zephyra_core::props::MATERIAL) {
            Some(zephyra_core::property::PropertyValue::Material(m)) => Some(*m),
            _ => None,
        };
        got.push((key.to_string(), material.unwrap_or(Material::solid())));
    });

    assert_eq!(
        got.len(),
        3,
        "材质样板只有 {} 块 —— 专区讲的是三种表面，少一块就少一种对照",
        got.len()
    );
    let want = [
        ("swatch:solid", Material::solid()),
        ("swatch:frosted", Material::frosted()),
        ("swatch:glass", Material::glass()),
    ];
    for (key, m) in &want {
        let found = got
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("没有 {key} 这块样板"));
        assert_eq!(
            found.1, *m,
            "{key} 的材质是 {:?}，应为 {m:?} —— 材质选型这一栏就少了一种对照",
            found.1
        );
    }
}

/// 三块玻璃必须**整块落在照片板上**。
///
/// # 这条钉的是整页的**前提**
///
/// 折射采样的是「**已经画在屏幕上**的东西」。玻璃块下面没有照片 ⇒ 位移纯色
/// ⇒ 画面上「折射没生效」。而那不是着色器的问题，是**几何**的问题 —— 于是
/// 排查方向会整个跑偏（去查 HLSL、查滤镜链、查离屏尺寸）。
///
/// # 为什么判据是「整块落在板里」
///
/// 玻璃块**探出**照片板的那一角，底下是纯色板底 ⇒ 那一角读作「断裂 / 拖影」
/// （真 GPU 截图上实测出现过：薄那块底边一条橙/青色断裂带）。所以不是
/// 「玻璃块与照片有重叠」就行，而是要**整块**在照片里。
#[test]
fn the_glass_panes_sit_on_the_photo() {
    let root = laid_out_materials();
    let board = find_by_key(&root, "material-board").expect("要有照片板");
    let b = board.rect();

    for key in ["glass:a", "glass:b", "glass:c"] {
        let r = find_by_key(&root, key)
            .unwrap_or_else(|| panic!("要有 {key} 这块玻璃"))
            .rect();
        assert!(
            r.width > 0.0 && r.height > 0.0,
            "{key} 的矩形是 {r:?} —— 玻璃块量成 0，照片上什么都看不到"
        );
        // ⚠ 格式串里**不能**写 `{r.x:.1}` —— Rust 的格式串不支持字段访问，
        // 只能按位置取参。所以先把要用��提出来。
        let (rx, ry, rw, rh) = (r.x, r.y, r.width, r.height);
        assert!(
            rx >= b.x - 1.0 && rx + rw <= b.x + b.width + 1.0,
            "{key} 横向探出照片板：玻璃在 {rx:.1}..{:.1}，板在 {:.1}..{:.1} —— \
             探出去的那部分底下是纯色，读作断裂 / 拖影",
            rx + rw,
            b.x,
            b.x + b.width
        );
        assert!(
            ry >= b.y - 1.0 && ry + rh <= b.y + b.height + 1.0,
            "{key} 纵向探出照片板：玻璃在 {ry:.1}..{:.1}，板在 {:.1}..{:.1} —— \
             探出去的那部分底下是纯色，读作断裂 / 拖影",
            ry + rh,
            b.y,
            b.y + b.height
        );
    }
}

/// 照片必须**露在玻璃块外面** —— 只盖住是不够的，还得没盖满。
///
/// # 为什么这条和 `the_glass_panes_sit_on_the_photo` 是两头夹
///
/// 「玻璃块落在照片里」保证块内有东西可弯；「照片露在块外」保证**块外**还有
/// 同一段画面可看。缺了后者时真 GPU 截图上是三块**不透明的贴片**：照片只在
/// 玻璃里有，玻璃外全是纯色 —— 于是「里面那些山脊到底有没有被掰弯」在画面上
/// **无从对照**，读出来就是「一点透明度都没有」。
///
/// 这是「有没有对照」的问题，不是「折射强弱」的问题：折射参数再对，没有外部
/// 参照也读不出玻璃来。所以它单列一条，而不是并进上面那条的某个 assert。
#[test]
fn the_photo_extends_beyond_the_glass() {
    const EXPOSE: f32 = 30.0;

    let root = laid_out_materials();
    let board = find_by_key(&root, "material-board").expect("要有照片板");
    let b = board.rect();
    let pane = find_by_key(&root, "glass:a").expect("要有玻璃块").rect();

    let exposed_top = pane.y - b.y;
    let exposed_bottom = (b.y + b.height) - (pane.y + pane.height);
    assert!(
        exposed_top >= EXPOSE,
        "照片只在玻璃块上方露出 {exposed_top:.1}px（要求 ≥{EXPOSE}）—— \
         照片几乎被玻璃盖满，外面没有对照 ⇒ 玻璃读作不透明贴片"
    );
    assert!(
        exposed_bottom >= EXPOSE,
        "照片只在玻璃块下方露出 {exposed_bottom:.1}px（要求 ≥{EXPOSE}）—— \
         照片几乎被玻璃盖满，外面没有对照 ⇒ 玻璃读作不透明贴片"
    );
}

/// 玻璃块压着的那块照片**必须有结构** —— 否则「折射算对了却看不见」。
///
/// # 这条钉的是「图案侧」，不是着色器侧
///
/// 位移本身由着色器管（那侧断的是「算得对」）。这条管的是**被折射的东西**：
/// 位移要改得动画面，那一块照片里必须有**颜色边界**；一块纯色（哪怕是
/// 渐变到看不出差别）的区域，位移多大都读不出来 —— 真 GPU 截图上的观感是
/// 「折射没生效」，而着色器其实一直在算。
///
/// # 判据是**梯度能量**，不是「有没有某条线」
///
/// 旧版对着棋盘格断言「玻璃上下边往内 20px 内有横边界」—— 那依赖棋盘格
/// 的规则行距，是**图案的排版**。换成照片之后这条判据不成立了（照片没有
/// 「行」），于是换成与图案无关的量：**相邻像素亮度差的均值**。
///
/// 一块有内容的照片（山脊、石阶、花丛）在任何一段上都有可观的梯度；
/// 一块纯色或极缓渐变的图接近 0。门槛取实测值的约 1/4，留足余量。
///
/// # 这条要解一次像素
///
/// 所以它是本文件里唯一一条**读资源像素**的门禁。代价是一次 JPEG 解码
/// （约 50ms），换来的是「换一张图会当场红」—— 而那正是它要防的事。
#[test]
fn the_photo_under_the_panes_has_structure() {
    let root = laid_out_materials();
    let board = find_by_key(&root, "material-board").expect("要有照片板");
    let b = board.rect();
    let pane = find_by_key(&root, "glass:a").expect("要有玻璃块").rect();

    // 玻璃块在照片上的归一化区域 —— 照片铺满板，所以板的矩形就是照片的映射。
    let u0 = (pane.x - b.x) / b.width;
    let u1 = (pane.x + pane.width - b.x) / b.width;
    let v0 = (pane.y - b.y) / b.height;
    let v1 = (pane.y + pane.height - b.y) / b.height;

    let bytes = include_bytes!("../assets/mountain.jpg");
    // `include_bytes!` 给的是 `&[u8; N]`，而 `Decoder::new` 要 `&[u8]`（它按
    // `io::Read` 收）。
    let mut dec = jpeg_decoder::Decoder::new(&bytes[..]);
    let rgb = dec.decode().expect("资源能解开");
    let info = dec.info().expect("资源有尺寸");
    assert_eq!(
        info.pixel_format,
        jpeg_decoder::PixelFormat::RGB24,
        "资源不是 RGB24 —— 这条门禁按三通道解像素，换格式要一起改"
    );
    let (w, h) = (info.width as usize, info.height as usize);
    let x0 = (u0 * w as f32) as usize;
    let x1 = ((u1 * w as f32) as usize).min(w);
    let y0 = (v0 * h as f32) as usize;
    let y1 = ((v1 * h as f32) as usize).min(h);
    assert!(
        x1 > x0 + 8 && y1 > y0 + 8,
        "玻璃块在照片上只映射到 {x0}..{x1} × {y0}..{y1} —— 取不到内容"
    );

    let lum = |x: usize, y: usize| -> f32 {
        let i = (y * w + x) * 3;
        0.299 * rgb[i] as f32 + 0.587 * rgb[i + 1] as f32 + 0.114 * rgb[i + 2] as f32
    };
    let mut sum = 0.0f32;
    let mut n = 0.0f32;
    for y in y0..y1 {
        for x in x0..x1 {
            if x + 1 < x1 {
                sum += (lum(x, y) - lum(x + 1, y)).abs();
                n += 1.0;
            }
            if y + 1 < y1 {
                sum += (lum(x, y) - lum(x, y + 1)).abs();
                n += 1.0;
            }
        }
    }
    let mean = sum / n.max(1.0);
    assert!(
        mean > 6.0,
        "玻璃块压着的那块照片几乎没有结构（相邻像素亮度差均值 {mean:.2}）—— \
         位移再对也读不出来，画面上就是「折射没生效」"
    );
}

/// 玻璃块**不许有底色** —— 「上面盖着一层玻璃」只由折射 + 描边亮线表达。
///
/// # 这条钉的是「玻璃要是透明的，不要有底色，纯靠折射反应层级关系」
///
/// 任何底色（哪怕 15% 的一层白纱）都会在玻璃后面与真实背景之间隔上一层颜色：
/// 那读起来是**贴片**不是玻璃。前两轮把纱从 30% 调到 15%（dev-log 第八节），
/// 而这条需求把方向整个反过来 —— **不加**才是对的，纱被整个删掉。
///
/// # 判据：字符串级，不是像素级
///
/// 替代的是像素门禁 `the_glass_block_is_distinguishable_from_what_it_covers`：
/// 它的前提（「底色必须影响画面 ≥90% 的像素」）被这条需求**反转** —— 玻璃
/// 恰恰必须**不**影响画面，那条判据会恒红。反过来的「玻璃不改变画面」也做不成
/// 像素门禁：CPU 光栅不实现滤镜，两遍渲染只差一个 alpha 混合，而「有底色的
/// 那一遍」已经不存在了（唯一还能比的是把块藏起来，但那会把标签挪进取样区）。
///
/// 于是可见性（位移本身）由几何门禁（`refraction_target_*` 三条）+ 着色器侧
/// 门禁钉住，「看得见玻璃」由真 GPU 截图复核（dev-log 第九节）。
#[test]
fn the_glass_pane_has_no_fill() {
    // ---- 先剥掉 `/* … */` 注释再找规则 ----
    //
    // 规则上方的注释讨论的就是「为什么不许有 background」，里面有
    // `.glass-pane` 的字样；不剥注释的话，注释里的字就会被当成规则文本。
    fn strip_comments(css: &str) -> String {
        let mut out = String::with_capacity(css.len());
        let mut it = css.chars().peekable();
        while let Some(c) = it.next() {
            if c == '/' && it.peek() == Some(&'*') {
                it.next();
                let mut depth = 1;
                while depth > 0 {
                    match it.next() {
                        Some('/') if it.peek() == Some(&'*') => {
                            it.next();
                            depth += 1;
                        }
                        Some('*') if it.peek() == Some(&'/') => {
                            it.next();
                            depth -= 1;
                        }
                        Some(_) => {}
                        None => break,
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    for (name, css) in zephyra_gallery::styles::all() {
        let code = strip_comments(css);

        // 玻璃块那条规则里不许出现 `background`。
        if let Some(at) = code.find("box.glass-pane") {
            let rest = &code[at..];
            let body_start = rest.find('{').expect("`.glass-pane` 规则后面要跟 `{`");
            let body_end = rest[body_start..]
                .find('}')
                .expect("`.glass-pane` 规则没有闭合的 `}`");
            let body = &rest[body_start..body_start + body_end];
            assert!(
                !body.contains("background"),
                "{name}.zstyle 的 `.glass-pane` 规则里有 `background` —— \
                 玻璃必须是纯透明的，层级感只由折射 + 描边表达（贴片回来了）"
            );
        }

        // 两代白纱的颜色不许再出现在任何样式表里。
        for veil in ["#ffffff26", "#ffffff4d"] {
            assert!(
                !code.to_lowercase().contains(veil),
                "{name}.zstyle 里出现了 {veil} —— 那是被删掉的玻璃白纱\
                 （15% / 30% 白），底色不许回来"
            );
        }
    }
}

/// 边界上的 `[−] [+] 必须是**无副作用**的 no-op。
///
/// # 为什么单独一条
///
/// 「已到上限还继续加」若写成了别的形状（例如 `*slot = max` 无条件赋值），
/// 画面上仍然一切正常，但**上一条门禁的判据就不再可信** —— 因为它靠
/// 「改参数 → 树变」来证明参数没被写死。`Glass::nudge` 返回 `bool` 就是给这条
/// 门禁用的，调用方（`click`）刻意忽略它，于是只有这里能验。
#[test]
fn glass_param_steppers_are_clamped_without_side_effects() {
    let mut g = Gallery::default();
    for param in Param::ALL {
        // 一路加到顶。默认档不在 0 档，所以先一路减到底再从 0 起数 ——
        // 否则「加了几次」与「离上限还有几档」永远差一个常数，这条判据就废了。
        while g.glass.nudge(param, -1) {}
        let steps = match param {
            Param::Thickness => THICKNESS.len() - 1,
            Param::Bezel => BEZEL.len() - 1,
            Param::Dispersion => DISPERSION.len() - 1,
            Param::Ior => IOR.len() - 1,
        };
        let mut n = 0;
        while g.glass.nudge(param, 1) {
            n += 1;
            assert!(n <= steps, "步进加到了 {n} 次还不停 —— clamp 没生效");
        }
        assert_eq!(n, steps, "从 0 档加到顶应是 {steps} 次，实测 {n}");
        // 顶上一格：no-op。
        let top = g.glass;
        assert!(!g.glass.nudge(param, 1), "已到上限却报告「动了」");
        assert_eq!(g.glass, top, "已到上限再加却改变了状态");
        // 顶上加两次（防「单次 clamp、双次累积」那种形状）。
        assert!(!g.glass.nudge(param, 1));
        assert_eq!(g.glass, top);
        // 一路减到底：步进次数必须与加法对称（到 0 档的距离相同）。
        let mut k = 0;
        while g.glass.nudge(param, -1) {
            k += 1;
            assert!(k <= steps, "步进减到了 {k} 次还不停 —— clamp 没生效");
        }
        assert_eq!(k, steps, "从顶档减到底应是 {steps} 次，实测 {k}");
        let bottom = g.glass;
        assert!(!g.glass.nudge(param, -1), "已到下限却报告「动了」");
        assert_eq!(g.glass, bottom, "已到下限再减却改变了状态");
        // 步进**一个**参数不得牵动其它三个 —— `nudge` 里那四条 match 分支若写成了
        // 「回写整个结构体」，这条会绿而画面上四个值一起跳。
        let mut probe = Gallery::default();
        let others = |g: &Glass| -> Vec<usize> { vec![g.thickness, g.bezel, g.dispersion, g.ior] };
        let before = others(&probe.glass);
        for p in Param::ALL {
            probe.glass.nudge(p, 1);
        }
        let after = others(&probe.glass);
        for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
            assert_ne!(
                b, a,
                "四个参数都加了一档，但第 {i} 个没变 —— nudge 漏了某个字段"
            );
        }
    }
}

// --------------------------------------------------- 门禁 27：死规则反查

/// 样式表里每条规则都得**选得中至少一个元素** —— 反方向的门禁。
///
/// # 为什么前两条不够
///
/// `every_class_name_the_source_emits_exists_in_the_stylesheet` 与
/// `every_element_is_reachable_by_the_stylesheet` 都是**树 → 表**这一个方向：
/// 前者查「源码发的每个分类名在表里有规则吗」，后者查「每个元素有分类名或裸规则
/// 吗」。合起来仍漏掉一整类静默失效：**表里有规则，而树里没有任何元素能被它
/// 选中**。
///
/// # 实测过一次（写这条门禁的直接原因）
///
/// 给圆按钮调尺寸时写了 `control.button.round { width: 28px; height: 28px; }`。
/// `controls::round_button` 发的是 `NodeTag::Control` + 分类名 `round` ——
/// **没有** `button` 这个分类名（`button` 是别的控件用的）。于是这条规则一个
/// 元素都选不中，尺寸**纹丝不动**，而两个方向的门禁全绿：分类名 `round` 有规则、
/// 元素有分类名。这正是「门禁全绿但界面是错的」那一类。
///
/// # 交互态为什么必须跑全
///
/// `.pressed` / `.checked` / `.hovered` 规则只在对应状态下选得中元素，所以判据
/// 取**全部路由 × 全部交互脚本**的并集（[`interaction_scripts`]），而不是只看
/// 默认状态。
///
/// # 为什么不查**控件层**（那 15 张表）
///
/// 控件层是**给宿主用的样式库**，它承诺覆盖一整套状态 —— `text.focused`、
/// `box.toast.error`、`box.ring-track.paused`、`box.suggest-row.selected`
/// 都在表里，而本 gallery 的界面**根本不存在**那些状态（没有 toast、没有输入框、
/// 没有建议列表）。按全表判，这些规则会全部报成死规则，于是唯一的选择是删规则
/// —— 那是把控件层改成「只服务当前 demo」，比留几条暂时用不上的规则坏得多。
///
/// 所以判据按 [`zephyra_gallery::styles::app_only`] 取**应用层**规则：应用层
/// 描述的就是这个界面，用不上的规则就是真死规则。控件层留待有对应界面时再查。
///
/// # 恒真防御
///
/// 先断言「参与判定的元素远多于规则数」，否则树遍历没走完时这条会变成「没有规则
/// 需要检查」而恒真地绿。
#[test]
fn every_stylesheet_rule_matches_something() {
    use std::collections::HashSet;
    use zephyra_core::id::ElementKey;

    type Shapes = HashSet<(NodeTag, Vec<&'static str>)>;

    // ⚠ 判据**只取应用层**（`shell.zstyle` + `pages.zstyle`）的规则，不看控件层。
    // 见函数文档「为什么不查控件层」那一节。
    let mut rules: Vec<(usize, String, zephyra_style::ir::Selector)> = Vec::new();
    for (sheet_name, src) in zephyra_gallery::styles::app_only() {
        let parsed = zstyle_compile::parse(sheet_name, src);
        assert!(
            parsed.is_ok(),
            "{sheet_name}.zstyle must parse: {:?}",
            parsed.errors
        );
        for (i, r) in parsed.sheet.rules().iter().enumerate() {
            rules.push((i, (*sheet_name).to_string(), r.selector.clone()));
        }
    }

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);

    /// 从默认状态重造一个 `Gallery`：路由 + 脚本 + 额外改一处。
    ///
    /// # 为什么每次**重造**而不是 clone
    ///
    /// `Gallery` 里挂着 `Arc<AtomicU8>`（主题意图，宿主每帧写）与 `Preferences`，
    /// 派生 `Clone` 会让「门禁克隆了一份宿主状态」变成可能 —— 而那条路一旦走歪，
    /// 门禁验的就是它自己造的世界。重造只有一条真相来源：`Gallery::default()`。
    fn build(
        route: &'static str,
        script: fn(&mut Gallery),
        extra: impl FnOnce(&mut Gallery),
    ) -> Gallery {
        let mut g = Gallery::default();
        g.route = route;
        script(&mut g);
        extra(&mut g);
        g
    }

    /// 看一棵树：登记所有出现过的「类型 + 分类名」形状，并交回可交互的 key。
    ///
    /// ⚠ 只登记**形状**不登记节点：`Selector::matches` 看的就是这两样，而把整棵
    /// 树留在哈希表里在几千次构造下要几百 MB。
    fn absorb(
        seen: &mut Shapes,
        elements: &mut usize,
        g: &Gallery,
        ctx: &Context<'_>,
    ) -> Vec<&'static str> {
        let element = g.view(ctx);
        let root = Node::from_element(&element);
        let mut keys = Vec::new();
        walk(&root, &mut |n: &Node| {
            *elements += 1;
            seen.insert((n.tag(), n.style_names().to_vec()));
            let interactive = n
                .props()
                .iter_local()
                .any(|(k, _)| std::ptr::eq(k, &zephyra_core::props::INTERACTIVE));
            if interactive {
                if let Some(k) = n.key().and_then(|k| k.as_name()) {
                    keys.push(k);
                }
            }
        });
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    let routes: Vec<&str> = ROUTES.iter().map(|(id, _, _)| *id).collect();

    let mut seen: Shapes = HashSet::new();
    let mut elements = 0usize;

    // ⚠ 记下第一项再迭代：趟 D 要在循环里回头引用它，而 \or x in vec\ 会 move 掉 vec。
    let first_route = routes[0];
    for route in routes {
        // ---- 趟 A：全部交互脚本，只登记形状，顺便攒下这一页出现过的交互 key ----
        //
        // ⚠ 形状登记与 key 收集**必须一趟做完**：分两趟就要把树留下来，而树不留
        // （见 `absorb` 的注释），于是第二趟只能重跑一遍脚本。
        let mut keys: Vec<&'static str> = Vec::new();
        for (_, script) in interaction_scripts() {
            let g = build(route, script, |_| {});
            for k in absorb(&mut seen, &mut elements, &g, &ctx) {
                keys.push(k);
            }
        }
        keys.sort_unstable();
        keys.dedup();

        // ---- 趟 B：逐个交互 key 悬停 / 按住（用**默认**脚本造）----
        //
        // 为什么只在默认脚本下造悬停态：`hovered` / `pressed` 的**分类名**与脚本
        // 无关（`state()` 只看指针落在哪个 key 上），所以「哪些规则会因此被选中」
        // 只取决于**哪些 key 存在**，而 key 的全集刚在趟 A 里拿到了。
        //
        // ⚠ 这也是为什么不能手列 key：漏一个，那个元素身上的 `.hovered` /
        // `.pressed` 规则就会被误判成死规则，于是这条门禁的第一次失败会教人去
        // **删规则** —— 那比没有这条门禁更坏。
        for k in &keys {
            absorb(
                &mut seen,
                &mut elements,
                &build(route, |_| {}, |g| g.hovered = Some(ElementKey::Name(k))),
                &ctx,
            );
            absorb(
                &mut seen,
                &mut elements,
                &build(
                    route,
                    |_| {},
                    |g| {
                        g.pressed = Some(ElementKey::Name(k));
                        g.last_pressed = Some(ElementKey::Name(k));
                    },
                ),
                &ctx,
            );
        }

        // ---- 趟 C：断点三档（`.compact` 只在小档出现）----
        for w in [640.0_f32, 1000.0, 1400.0] {
            absorb(
                &mut seen,
                &mut elements,
                &build(route, |_| {}, |g| g.viewport = (w, 900.0)),
                &ctx,
            );
        }

        // ---- 趟 D：入场动画的 8 个档 ----
        //
        // ⚠ 采样必须**密**于档位数：`enter_class()` 走的是缓动函数
        // （`anim::progress` → `anim::step`），进度到档位边界的映射不是线性的，
        // 8 个均匀中点只落进其中 4~5 个桶 —— 于是 `.enter-3` 这类规则会被误判成
        // 死规则。取 8 倍密度的采样（64 个），代价是几十棵树，可以忽略。
        //
        // 只在**第一个路由**上跑：`.page` 与 `.enter-N` 是外壳发的，每个路由都有。
        if route == first_route {
            for i in 0..64 {
                let ms = (i as f32 + 0.5) * zephyra_style::anim::ENTER_MS / 64.0;
                absorb(
                    &mut seen,
                    &mut elements,
                    &build(route, |_| {}, |g| g.enter_ms = ms),
                    &ctx,
                );
            }
        }
    }

    assert!(
        elements > 5000,
        "只遍历了 {elements} 个元素 —— 树遍历没走完，这条门禁可能恒真"
    );
    assert!(
        rules.len() > 50,
        "应用层只有 {} 条规则 —— 判据可能没取到规则",
        rules.len()
    );

    let dead: Vec<String> = rules
        .iter()
        .filter(|(_, _, sel)| !seen.iter().any(|(tag, names)| sel.matches(*tag, names)))
        .map(|(i, sheet_name, sel)| {
            let names: Vec<String> = sel.names.iter().map(|n| format!(".{n}")).collect();
            format!(
                "{sheet_name}.zstyle 第 {i} 条 `{}{}`",
                sel.tag.map(|t| format!("{t:?}")).unwrap_or_default(),
                names.concat()
            )
        })
        .collect();

    assert!(
        dead.is_empty(),
        "样式表里这些规则一个元素都选不中（写了等于没写，而树→表那两条门禁不会报）：\n  {}",
        dead.join("\n  ")
    );
}

// ------------------------------------------- 门禁 28：交互控件必须接线

/// 树的**形状指纹**：类型 + key + 分类名 + 文本值，深度优先后拼成一个串。
///
/// # 为什么不用 `compare_trees`
///
/// 点击可能导致**结构**变化（关掉 InfoBar 少一个子树），而 `compare_trees`
/// 的路径配对假设两棵树同构。指纹串对结构差异天然鲁棒：多一个节点就多一段，
/// 两串不等。
///
/// # 为什么包含 TEXT 值
///
/// `act:` 按钮的反馈是改一个 text_label 的文本——分类名、类型、结构全都不变，
/// 只有 TEXT 变。不含它的话「点了状态变了」会被误判成「没接」。
fn shape_fingerprint(node: &Node, out: &mut String) {
    use std::fmt::Write as _;
    let text = node
        .props()
        .iter_local()
        .find(|(k, _)| std::ptr::eq(*k, &zephyra_core::props::TEXT))
        .map(|(_, v)| format!("{v:?}"))
        .unwrap_or_default();
    let _ = write!(
        out,
        "{:?}|{:?}|{:?}|{};",
        node.tag(),
        node.key(),
        node.style_names(),
        text
    );
    for c in node.children() {
        shape_fingerprint(c, out);
    }
}

/// 展示型控件白名单 —— 点击**不**改变状态是**正确的**行为。
///
/// # 为什么白名单必须短、且每类有出处
///
/// 这份名单是「这个应用里哪些可交互元素点了没反应」的**完整清单**。多进一项，
/// 就多一个骗过门禁的空壳；少进一项，门禁红（安全方向的失败）。每一项都
/// 必须能回答「为什么不接」：
///
/// | 类别 | 成员 | 为什么不接 |
/// |---|---|---|
/// | 窗口能力 | `win:min` / `win:max` / `win:close` | 最小化/关闭是宿主能力，View 拿不到 |
/// | 无拖动输入 | `slider-volume`、`state-slider` | core 的 `InputState` 没有连续指针位置，拖动无法表达 |
/// | 无文本输入 | `field-name`、`area-notes` | core 无键盘事件路由与光标 |
/// | 指示器 | `progress`、`fx-soft`、`fx-hard` | 「进行到哪」的显示件；进度值是应用传的参数，不是可点状态 |
/// | 外观展示 | `navdemo:*` | 导航件展示的是**外观**（真正的导航在 `shell`）——本页文档明说，见 `pages/navigation.rs` 页首 |
/// | 外观展示 | `round-a`、`round-b` | round_button 的「普通 / checked」两种外观并排，不是两个可切换开关 |
/// | 不可关闭 | `infobar`、`infobar-err` | `info_bar` 根节点的可点区域就是关闭钮所在行；关掉的那条由 `bar:close` 负责，根自身无动作 |
/// | 主题意图 | `theme-toggle` | 它改的是**主题**（颜色），树形状不变——`theme_switch_changes_colors_without_touching_the_tree` 专门钉住这一点 |
///
/// ⚠ 往这里加条目之前先问：**能不能接？** `slider` 类是 core 真没有拖动
/// 输入；`progress` 类本来就不是按钮。二者之外的「点了没反应」几乎都是
/// 漏接线——先接线，进不来白名单。
const DISPLAY_ONLY: &[&str] = &[
    "win:min",
    "win:max",
    "win:close",
    "slider-volume",
    "state-slider",
    "field-name",
    "area-notes",
    // SpinBox 的**根节点**是聚焦区（输入框本体）。core 没有文本输入与焦点，
    // 点它正确行为就是什么都不发生；能点的只有两枚 `spin:up/down`。
    "spin-count",
    "progress",
    "state-progress",
    "progress-p",
    "progress-e",
    "fx-soft",
    "fx-hard",
    "navdemo:home",
    "navdemo:star",
    "navdemo:archive",
    "round-a",
    "round-b",
    "infobar",
    "infobar-err",
    "theme-toggle",
    // 文本输入页的粘贴按钮：粘贴空剪贴板时状态不变，不是空壳——
    // 真剪贴板有内容时会插入文本。
    "text:paste",
    // 文本输入框本体：点击聚焦但焦点态由 InputState 决定，点击本身不改
    // 变树形状；全选空文本和复制空剪贴板也无可见变化。
    "text:field",
    "text:select-all",
    "text:copy",
    // ScrollView：点击（不拖拽）不改变状态，正确行为是无反应；滚轮/拖拽才会动。
    "scroll:demo",
    // ScrollBar 轨道：点击轨道当前未接线（应跳转到该位置），暂列白名单。
    "scroll:bar",
    // 滚动条拇指：点击不拖拽不改变状态，正确行为是无反应；拖拽才会动。
    "fill:scroll-thumb",
];

/// **每个可交互 key 必须真的改变状态，或在本文件的展示型白名单里。**
///
/// # 这是本轮最要紧的一条门禁
///
/// 「按钮只有样式、点了没反应」正是开发者对示例项目最致命的差评——它教的
/// 是「这个框架的控件是假的」。而这类失效**编译过、画面正常、其余门禁全绿**：
/// key 在树上（`every_class_name_*` 过）、元素可达（`every_element_is_*` 过）、
/// 命中测试通过（`hit_test_*` 过），唯一缺的是 `click()` 里没有对应分支。
/// 首次运行就抓出 30 个 key：`state-switch` / `lang:*` / `combo-material`
/// 是真漏接，`spin:up` 的显示表越界是真 bug，而 `feedback` / `tool:*` /
/// `round-*` 属于三种不同的「看起来能点」。
///
/// # 判据
///
/// 对全部路由 × 参考页两族构造树，收集每个 `INTERACTIVE` 节点的 key；然后
/// 对每个 key **真的点一遍**，比较点击前后树的形状指纹（类型/key/分类名/文本）。
/// 有差异 = 接线了；无差异则必须在 [`DISPLAY_ONLY`] 里。
///
/// # 为什么要「扰动」
///
/// 选中型控件（radio / segmented / list 行 / 导航项）点击**当前选中项**时
/// 状态本来就不变 —— 这是正确行为，不是空壳。而默认 Gallery 恰好停在各控件
/// 的默认项上，直接点会得到假阴性。所以每个 key 都在三个人为扰动的世界里试
/// （见 [`perturb`]）：只要有一个世界里指纹不同，就证明分派存在。
///
/// # 恒真防御
///
/// 先断言收集到的可交互 key 数量有下界（50+），否则路由表写错时收集是空的，
/// 这条会因「没有可检查的 key」而恒真地绿。
#[test]
fn every_interactive_key_is_wired_or_whitelisted() {
    use std::collections::BTreeMap;

    // 1) 全路由 × 全族扫树，收集每个 INTERACTIVE key 与它所在的（路由, 族）。
    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let mut found: BTreeMap<&'static str, (&'static str, usize, bool)> = BTreeMap::new();
    let mut interactives = 0usize;

    let mut collect =
        |g: &Gallery, found: &mut BTreeMap<&'static str, (&'static str, usize, bool)>| {
            let element = g.view(&ctx);
            let root = Node::from_element(&element);
            walk(&root, &mut |n: &Node| {
                let interactive = n
                    .props()
                    .iter_local()
                    .any(|(k, _)| std::ptr::eq(k, &zephyra_core::props::INTERACTIVE));
                if !interactive {
                    return;
                }
                interactives += 1;
                if let Some(key) = n.key().and_then(|k| k.as_name()) {
                    // 同一个 key 可能出现在多个路由（语言切换按钮在 theme/testing
                    // 两页）——任一处接线即算接线，取第一次出现的位置即可。
                    //
                    // ⚠ `disabled` 一并记下：**禁用的控件点了本就不该有反应**，
                    // 那是它的正确行为而不是「空壳」。判据因此分成两路 ——
                    // 禁用的必须**点不动**，没禁用的必须**点得动**。
                    let disabled = n.props().iter_local().any(|(k, v)| {
                        std::ptr::eq(k, &zephyra_core::props::DISABLED)
                            && v.get::<bool>().unwrap_or(false)
                    });
                    found
                        .entry(key)
                        .or_insert((g.route, g.ref_family, disabled));
                }
            });
        };

    for route in ROUTES.iter().map(|(id, _, _)| *id) {
        let mut g = Gallery::default();
        g.route = route;
        collect(&g, &mut found);
    }
    for fam in 0..REF_FAMILIES.len() {
        let mut g = Gallery::default();
        g.route = "reference";
        g.ref_family = fam;
        collect(&g, &mut found);
    }
    assert!(
        interactives >= 50,
        "只收集到 {interactives} 个可交互节点 —— 树遍历没走完，这条门禁可能恒真"
    );
    assert!(
        found.len() >= 40,
        "只有 {} 个不同的交互 key —— 判据可能没取到",
        found.len()
    );

    // 2) 逐 key 在扰动世界里真点一遍，比较点击前后的树指纹。
    let mut hollow: Vec<String> = Vec::new();
    let mut disabled_but_live: Vec<String> = Vec::new();
    for (key, (route, family, disabled)) in &found {
        let mut wired = false;
        let mut seen_anywhere = false;
        // 世界 1/2：原路由 + 两个相位；世界 3：换一条路由（外壳/导航键在
        // 任何路由上都存在，用来覆盖「点当前路由的导航项」这种天然 no-op）；
        // 世界 4/5：额外扰动参考页的族（`ref:*` 段键只在 reference 路由上，
        // 但族切换器本身常驻，所以它必须在 reference 路由上判）。
        let worlds = [
            (*route, 0usize, false),
            (*route, 1, false),
            (other_route(route), 0, false),
            (*route, 0, true),
            (*route, 1, true),
        ];
        for (world_route, phase, perturb_family) in worlds {
            let build = |k: Option<&str>| {
                let mut g = Gallery::default();
                g.route = world_route;
                g.ref_family = *family;
                perturb(&mut g, phase, perturb_family);
                if let Some(k) = k {
                    g.click(k);
                }
                let element = g.view(&ctx);
                Node::from_element(&element)
            };
            let before = build(None);
            if !tree_has_key(&before, key) {
                continue; // 这个 key 不在这个世界的树里（页面级 key + 换了路由）
            }
            seen_anywhere = true;
            let after = build(Some(key));
            let mut fp_before = String::new();
            let mut fp_after = String::new();
            shape_fingerprint(&before, &mut fp_before);
            shape_fingerprint(&after, &mut fp_after);
            if fp_before != fp_after {
                if *disabled {
                    // 禁用的控件**点了有反应** = 「禁用」没生效。
                    //
                    // 这不是空壳问题，是**反向**的问题：禁用态画成灰的、点下去
                    // 却还是能切状态 —— 用户会以为程序坏了。
                    disabled_but_live.push(format!("{key}（route {route}）"));
                }
                wired = true;
                break;
            }
        }

        if *disabled {
            // 禁用的 key 在所有世界里都点不动 = 通过（它**应该**如此）。
            // 它不进 `hollow`：`hollow` 问的是「该动的有没有动」。
            continue;
        }

        if wired || DISPLAY_ONLY.contains(key) {
            continue;
        }
        if !seen_anywhere {
            hollow.push(format!(
                "{key}（route {route}）—— 扰动后的树里找不到它，判据失效"
            ));
            continue;
        }
        hollow.push(format!(
            "{key}（route {route}）—— 三个扰动世界里点击前后树完全相同"
        ));
    }

    assert!(
        hollow.is_empty(),
        "这些可交互控件点了没有任何状态变化（空壳按钮——要么接线，要么\
         在 DISPLAY_ONLY 里注明为什么不接）：\n  {}",
        hollow.join("\n  ")
    );
    assert!(
        disabled_but_live.is_empty(),
        "这些控件标了 disabled，点了却还有反应（「禁用」没生效）：\n  {}",
        disabled_but_live.join("\n  ")
    );
}

/// 换一条路由（用于「点当前导航项」这类天然 no-op 的判定）。
fn other_route(route: &str) -> &'static str {
    ROUTES
        .iter()
        .map(|(id, _, _)| *id)
        .find(|id| *id != route)
        .unwrap_or("guide")
}

/// 把**选中型**状态挪离默认值，让「点击某一项」产生可见变化。
///
/// # 两个相位为什么够
///
/// 每个选择器只有两个候选值（相位 0 / 相位 1），而任意一个 key 至多等于
/// 其中一个候选——于是它在另一个相位里一定不是当前选中项。toggle 型字段
/// （`switch_on` 等）两个相位直接取 true/false，点击必翻转。
///
/// # 新增选中型字段时
///
/// 同步加进这里，否则该字段的默认选中项会让门禁假阴性（门禁本身仍绿——
/// 它只会漏判，不会误报，所以这条注释是唯一的提醒）。
fn perturb(g: &mut Gallery, phase: usize, perturb_family: bool) {
    let p = phase % 2;
    g.lang = if p == 0 { 1 } else { 2 };
    g.demo_check.set_state(if p == 1 {
        zephyra_ui::CheckState::Checked
    } else {
        zephyra_ui::CheckState::Unchecked
    });
    g.pill_on = p == 1;
    g.wants_dark = p == 1;
    // 开关族的两个状态：开关翻转 + 单选组换一项。
    //
    // ⚠ 新增**选中型**字段时必须加进这里 —— 否则「点了必须变」的门禁会在
    // 一个已经把状态扰到目标值的世界里恒真。
    if p == 1 {
        if !g.switch_demo.is_checked() {
            g.switch_demo.toggle();
        }
    } else {
        g.switch_demo.toggle();
    }
    let radio_target = zephyra_gallery::app::RADIO_KEYS[p];
    g.radio_group.select(radio_target);
    if perturb_family {
        // 族切换器（`ref:*`）常驻 reference 页，所以扰动族不会让它的 key
        // 消失；同时它让「点当前族」有可见变化。其它页的 key 依赖原族
        // 才存在，故由调用方传 `false` 的那几个世界先判。
        g.ref_family = (g.ref_family + 1 + p) % REF_FAMILIES.len();
    }
}

/// 树里是否还有这个 key 的可交互节点。
fn tree_has_key(root: &Node, key: &str) -> bool {
    let mut hit = false;
    walk(root, &mut |n: &Node| {
        if n.key().and_then(|k| k.as_name()) == Some(key) {
            hit = true;
        }
    });
    hit
}

// ───────────────────── 复选框：三态与组聚合 ─────────────────────

/// 节点上是否带某个分类名。
fn has_class(n: &Node, name: &str) -> bool {
    n.style_names().contains(&name)
}

/// **复选框的三态与复选框组的聚合必须与控件层的模型一致。**
///
/// # 为什么这条要有
///
/// 三态与组聚合是**画面上看不出来**的部分：
///
/// - 组头的半选**不是**用户摆出来的，是**从子项算出来**的；
/// - 子项的「选中」是模型里的 `CheckState`，过界时变成一个**分类词**
///   （`""` / `"checked"` / `"indeterminate"`）。
///
/// 任何一环对不上，画面上仍然是「一个能点的复选框」，只是状态是错的。
/// 而状态错在复选框上尤其难查：点一下它会变，只是变得不对。
///
/// # 判据
///
/// 对参考页的「选择」族（本轮把复选框组放在那里）逐项核对：
///
/// 1. 组头的分类名 == 模型算出的组状态；
/// 2. 每个子项的分类名 == 它自己的状态；
/// 3. **只有组头带半选横杠那个节点**（三态才带，两态子项不带）；
/// 4. 禁用项带 `disabled` 分类名、其余不带；
/// 5. 点组头 / 点子项之后，树上立刻跟着变（不只是模型变了）。
#[test]
fn checkbox_states_on_the_tree_match_the_model() {
    use zephyra_gallery::app::{CHECKS, CHECK_GROUP_KEY};

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);

    fn build(g: &Gallery, ctx: &Context<'_>) -> Node {
        Node::from_element(&g.view(ctx))
    }

    /// 断言一个复选框节点的状态分类名与 `word` 一致。
    fn assert_state(n: &Node, word: &str, what: &str) {
        if word.is_empty() {
            assert!(
                !has_class(n, "checked") && !has_class(n, "indeterminate"),
                "{what} 不该带任何状态分类名（模型说未选中）"
            );
        } else {
            assert!(
                has_class(n, word),
                "{what} 的状态分类名与模型不符：模型说 `{word}`，树上没有"
            );
            let other = if word == "checked" {
                "indeterminate"
            } else {
                "checked"
            };
            assert!(
                !has_class(n, other),
                "{what} 同时带了 `{other}` 与 `{word}` —— 两个状态互斥"
            );
        }
    }

    let mut g = Gallery::default();
    g.route = "reference";
    g.ref_family = 1;

    // 1) 组头 = 聚合态。
    let root = build(&g, &ctx);
    let header = find_by_key(&root, CHECK_GROUP_KEY).expect("复选框组的组头必须在树上");
    assert_state(header, g.check_group.header_state().class_word(), "组头");

    // 2) 子项 = 各自的状态。
    for key in CHECKS {
        let item = find_by_key(&root, key).unwrap_or_else(|| panic!("{key} 必须在树上"));
        let model = g.check_group.item(key).expect("模型里也要有这个子项");
        assert_state(item, model.class_word(), key);
    }

    // 3) 只有组头带半选横杠节点（三态才带）。
    assert!(
        find_by_key(header, "dash").is_some(),
        "组头是三态的，必须带半选横杠节点（dash）"
    );
    for key in CHECKS {
        let item = find_by_key(&root, key).unwrap_or_else(|| panic!("{key} 必须在树上"));
        assert!(
            find_by_key(item, "dash").is_none(),
            "{key} 是两态的，不该带半选横杠节点"
        );
    }

    // 4) 禁用项带 `disabled` 分类名，其余不带。
    for key in CHECKS {
        let item = find_by_key(&root, key).unwrap_or_else(|| panic!("{key} 必须在树上"));
        let model = g.check_group.item(key).expect("模型里也要有这个子项");
        assert_eq!(
            has_class(item, "disabled"),
            model.is_disabled(),
            "{key} 的禁用分类名与模型不符"
        );
    }

    // 5) 点组头 → 全选：树上组头变 checked、子项全变 checked。
    g.click(CHECK_GROUP_KEY);
    let root = build(&g, &ctx);
    let header = find_by_key(&root, CHECK_GROUP_KEY).expect("组头必须在树上");
    assert_state(header, "checked", "点组头后的组头");
    for key in CHECKS {
        if g.check_group.item(key).unwrap().is_disabled() {
            continue;
        }
        let item = find_by_key(&root, key).unwrap_or_else(|| panic!("{key} 必须在树上"));
        assert_state(item, "checked", &format!("点组头后的 {key}"));
    }

    // 6) 点一个子项 → 它自己变、组头变半选。
    g.click("check:alpha");
    let root = build(&g, &ctx);
    let header = find_by_key(&root, CHECK_GROUP_KEY).expect("组头必须在树上");
    assert_state(header, "indeterminate", "点一个子项后的组头");
    let alpha = find_by_key(&root, "check:alpha").expect("check:alpha 必须在树上");
    assert_state(alpha, "", "点过的 check:alpha");
    let beta = find_by_key(&root, "check:beta").expect("check:beta 必须在树上");
    assert_state(beta, "checked", "没点过的 check:beta");
}

/// **复选框组的组头必须能被键盘操作，且它的 key 在树上必须唯一。**
///
/// # 为什么这条独立于「状态与模型一致」那条
///
/// 那条查的是「画面上画出来的状态对不对」，这条查的是「按键盘有没有反应」。
/// 两类失效**互不可见**：
///
/// - 状态画错了 → 一眼能看出来；
/// - 键盘切不动 → 画面完全正常，鼠标也照常能点，只是按空格没反应。
///
/// 而「按 key 找节点」是**深度优先、第一个命中**（core 的
/// `FocusState::write_state` 与 `ActivationState::is_activatable` 都这么找）。
/// 于是只要组头那颗复选框的 key 与**别的节点**撞了（典型：组件体外层容器
/// 拿走了同一个 key），焦点就会写到那个容器上、激活也因为容器没有
/// `ACTIVATABLE` 而闩不住 —— 鼠标照常能点，键盘彻底失灵。
///
/// # 判据
///
/// 1. 组头 key 在**整棵树**上只出现一次；
/// 2. `FocusState::focus` 之后 FOCUSED 落在**带 `check-box` 分类名**的那个
///    节点上（而不是外层容器）；
/// 3. `ActivationState::key_down` 对组头 key **闩得住**（Space/Enter 生效）；
/// 4. 禁用的子项**闩不住**（禁用语义没被顺带破坏）。
#[test]
fn the_checkbox_group_header_is_keyboard_reachable() {
    use zephyra_core::id::ElementKey;
    use zephyra_core::input::activation::ActivationState;
    use zephyra_core::input::focus::FocusState;
    use zephyra_gallery::app::CHECK_GROUP_KEY;

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);

    let mut g = Gallery::default();
    g.route = "reference";
    g.ref_family = 1;
    let mut root = Node::from_element(&g.view(&ctx));

    // 1) 组头 key 在树上唯一。
    let mut hits = 0usize;
    walk(&root, &mut |n: &Node| {
        if n.key().and_then(|k| k.as_name()) == Some(CHECK_GROUP_KEY) {
            hits += 1;
        }
    });
    assert_eq!(
        hits, 1,
        "组头 key `{CHECK_GROUP_KEY}` 在树上出现了 {hits} 次 —— 按 key 找节点是 \
         深度优先第一个命中，撞 key 就会把焦点与激活引到别的节点上"
    );

    // 2) 焦点落在组头那颗复选框上。
    let mut fs = FocusState::new();
    fs.focus(&mut root, Some(&ElementKey::Name(CHECK_GROUP_KEY)), ctx);
    // 收集 (key, 分类名)：闭包不能把 `&Node` 带出去（生命周期不协变），
    // 所以取它要的**值**。
    let mut focused: Vec<(Option<String>, Vec<String>)> = Vec::new();
    walk(&root, &mut |n: &Node| {
        let f = n.props().iter_local().any(|(k, v)| {
            std::ptr::eq(k, &zephyra_core::props::FOCUSED) && v.get::<bool>().unwrap_or(false)
        });
        if f {
            focused.push((
                n.key().and_then(|k| k.as_name()).map(str::to_string),
                n.style_names().iter().map(|s| s.to_string()).collect(),
            ));
        }
    });
    assert_eq!(focused.len(), 1, "焦点应当只落在一个节点上");
    assert!(
        focused[0].1.iter().any(|c| c == "check-box"),
        "焦点落到了 {:?}（分类名 {:?}）上，而不是组头那颗复选框 —— 焦点环画不到复选框上",
        focused[0].0,
        focused[0].1
    );

    // 3) Space/Enter 闩得住组头。
    let mut act = ActivationState::new();
    assert!(
        act.key_down(&root, Some(&ElementKey::Name(CHECK_GROUP_KEY)), ctx),
        "组头按 Space/Enter 闩不住 —— 键盘切不动组头"
    );

    // 4) 禁用的子项闩不住。
    let mut act = ActivationState::new();
    assert!(
        !act.key_down(&root, Some(&ElementKey::Name("check:locked")), ctx),
        "禁用的子项竟然闩住了 —— 禁用语义被破坏了"
    );
}

/// 无障碍树必须与画面**一致** —— 画出来的复选框要被 UIA 认成复选框、
/// 名字念得出来、状态对得上、禁用项要报「不可用」。
///
/// # 为什么这条门禁在 gallery，而不是 `zephyra-uia`
///
/// `zephyra-uia` 的规则层只认**分类名**（`check-box` / `checked` / …）。
/// 而「模板有没有发出正确的分类名」只有 gallery 自己知道 —— 规则层测不出
/// 「模板忘了写 `check-box`」这类错。
///
/// 而这类错在画面上**完全看不出来**（控件照常显示、鼠标照常能点），
/// 只有屏幕阅读器念出一个「按钮」而不是「复选框」时才暴露。
///
/// # 为什么能在 Linux CI 上跑
///
/// `zephyra-uia` 的规则层是**平台中立**的（COM 那一半在 `provider` 模块，
/// `cfg(windows)`）—— 所以这条门禁在任何机器上都跑得了。
#[test]
fn the_accessibility_tree_matches_the_painted_controls() {
    use zephyra_gallery::app::{CHECKS, CHECK_GROUP_KEY, CHECK_LABEL_KEYS};
    use zephyra_uia::{Role, ToggleState, WindowInfo};

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let mut g = Gallery::default();
    g.route = "reference";
    g.ref_family = 1;
    let root = Node::from_element(&g.view(&ctx));

    let win = WindowInfo {
        name: "Zephyra Gallery".into(),
        client: Rect::new(0.0, 0.0, 1400.0, 800.0),
    };
    let snap = zephyra_uia::rules::build(&root, &win, ctx);

    // 恒真防御：树造失败时快照会只有窗口根 —— 那下面每条断言都会「因为找不到
    // 而跳过」，变成恒真绿。这条先把它钉死。
    assert!(
        snap.elements.len() > 5,
        "无障碍树只有 {} 个元素 —— 模板根本没产出控件？",
        snap.elements.len()
    );

    /// 分类词 → UIA 开关状态。
    fn toggle_of(word: &str) -> ToggleState {
        match word {
            "" => ToggleState::Off,
            "checked" => ToggleState::On,
            "indeterminate" => ToggleState::Indeterminate,
            other => panic!("未知的分类词 `{other}`"),
        }
    }

    // 1) 组头：复选框角色 + 名字 + 聚合出来的状态。
    let header = snap
        .find_by_key(CHECK_GROUP_KEY)
        .expect("组头必须在无障碍树上");
    assert_eq!(header.role, Role::CheckBox, "组头是复选框");
    // 组头的可访问名来自**控件层自带**的默认译文（`zephyra.checkbox.select-all`）——
    // 应用没写任何东西，它照样是对的。这条断言同时证明「控件层的默认文案
    // 真的并进了应用的表」（并表方向错的话这里会显示 key 本身）。
    assert_eq!(
        header.name,
        g.tr(g.check_group.header_label_key()),
        "组头的可访问名"
    );
    assert_eq!(
        header.toggle,
        Some(toggle_of(g.check_group.header_state().class_word())),
        "组头的开关状态必须与模型聚合出来的状态一致"
    );

    // 2) 子项：角色 + 名字 + 各自的状态。
    for (i, key) in CHECKS.iter().enumerate() {
        let el = snap
            .find_by_key(key)
            .unwrap_or_else(|| panic!("{key} 必须在无障碍树上"));
        assert_eq!(el.role, Role::CheckBox, "{key} 是复选框");
        // 可访问名 = **当前语言下**的译文。控件模板内部 `tr()` 出来的，
        // 所以这条断言量的是「控件层 i18n 通道真的通了」而不是「模型里存了什么」。
        assert_eq!(el.name, g.tr(CHECK_LABEL_KEYS[i]), "{key} 的可访问名");
        let model = g.check_group.item(key).expect("模型里也要有这个子项");
        assert_eq!(
            el.toggle,
            Some(toggle_of(model.class_word())),
            "{key} 的开关状态必须与模型一致"
        );
    }

    // 3) 禁用项：必须报「不可用」，否则屏幕阅读器还会念它、还会让它可操作。
    let locked = snap
        .find_by_key("check:locked")
        .expect("禁用项必须在无障碍树上");
    assert!(!locked.enabled, "禁用项必须报 IsEnabled=false");
    let alpha = snap
        .find_by_key("check:alpha")
        .expect("普通项必须在无障碍树上");
    assert!(alpha.enabled, "普通项必须报 IsEnabled=true");

    // 4) 分组容器：暴露成 Group 角色，挂着组头与子项。
    let group = snap
        .elements
        .iter()
        .find(|e| e.role == Role::Group)
        .expect("复选框组必须暴露成分组");
    assert!(
        group.children.len() >= 2,
        "分组下面至少要有组头与子项，实际 {} 个",
        group.children.len()
    );
}

// ------------------------------------------------------- 门禁：开关族的无障碍

/// 开关族（参考页第三族）必须按**正确的角色与图案**暴露。
///
/// # 为什么这条不能省
///
/// 开关与单选钮最容易错的一件事是「角色报错」：
///
/// - UIA **没有** Switch 这个 ControlType ⇒ 开关只能报 CheckBox + TogglePattern。
///   报错成 RadioButton 或 Button 时，屏读器念的是完全另一句话；
/// - 单选钮的「选中」是 **SelectionItem**，不是 Toggle。两者都报的话，
///   屏读器会既念「已勾选」又念「已选中」。
///
/// 而这两件事在画面上**完全看不出来** —— 控件照常显示、鼠标照常能点，
/// 只有开屏读器才知道坏了。所以只能在这里钉。
#[test]
fn the_switch_family_is_exposed_with_the_right_roles() {
    use zephyra_gallery::app::{RADIO_KEYS, RADIO_LABEL_KEYS, SWITCH_DEMO_KEY, SWITCH_LABEL_KEY};
    use zephyra_uia::{Role, ToggleState, WindowInfo};

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let mut g = Gallery::default();
    g.route = "reference";
    g.ref_family = 2;
    let root = Node::from_element(&g.view(&ctx));

    let win = WindowInfo {
        name: "Zephyra Gallery".into(),
        client: Rect::new(0.0, 0.0, 1400.0, 800.0),
    };
    let snap = zephyra_uia::rules::build(&root, &win, ctx);

    assert!(
        snap.elements.len() > 5,
        "无障碍树只有 {} 个元素 —— 开关族根本没产出控件？",
        snap.elements.len()
    );

    // 1) 开关：`switch` 分类名 ⇒ Role::Switch，状态落在**控件自己**身上。
    let sw = snap
        .find_by_key(SWITCH_DEMO_KEY)
        .expect("开关必须在无障碍树上");
    assert_eq!(sw.role, Role::Switch, "`switch` 必须映射成开关角色");
    assert_eq!(
        sw.name,
        g.tr(SWITCH_LABEL_KEY),
        "开关的可访问名必须来自 label_key 通道（模板内部 tr() 出来的）"
    );
    assert_eq!(
        sw.toggle,
        Some(if g.switch_demo.is_checked() {
            ToggleState::On
        } else {
            ToggleState::Off
        }),
        "开关状态必须与模型一致 —— 状态只落在子节点 `switch-track` 上时这里会一直是 Off"
    );

    // 2) 装饰子节点不许自己变成无障碍元素。
    for key in ["track", "knob", "label"] {
        let found = snap.elements.iter().any(|e| e.key == key);
        assert!(
            !found,
            "装饰/文本子节点 `{key}` 不该单独暴露 —— 无障碍树里它会长出一个假控件"
        );
    }

    // 3) 单选钮：`radio-box` ⇒ Role::RadioButton，且**只有一项**是 On。
    let mut on_count = 0;
    for (i, key) in RADIO_KEYS.iter().enumerate() {
        let el = snap
            .find_by_key(key)
            .unwrap_or_else(|| panic!("{key} 必须在无障碍树上"));
        assert_eq!(el.role, Role::RadioButton, "{key} 是单选钮");
        assert_eq!(el.name, g.tr(RADIO_LABEL_KEYS[i]), "{key} 的可访问名");
        let expected = if g.radio_group.is_selected(key) {
            on_count += 1;
            ToggleState::On
        } else {
            ToggleState::Off
        };
        assert_eq!(el.toggle, Some(expected), "{key} 的选中状态必须与组一致");
    }
    assert_eq!(
        on_count, 1,
        "单选组必须**恰好**一项选中（实际 {on_count} 项）"
    );
}

// ------------------------------------------------------- 门禁：把手与装饰的动态绑定

/// 把手位移 / 缩放是**绑定**出来的 —— 绑定断了，开关仍然能点、状态仍然翻转
/// （模型是对的），只是把手永远钉在左端：画面静默地错。所以这条直接查
/// 模板实例化之后、挂在 `knob` 上的 `translate-x` / `scale-x` / `scale-y` / `background`。
#[test]
fn the_switch_knob_travels_with_the_fraction() {
    use zephyra_gallery::app::SWITCH_DEMO_KEY;

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let mut g = Gallery::default();
    g.route = "reference";
    g.ref_family = 2;

    let knob_props = |g: &Gallery| {
        let element = g.view(&ctx);
        let root = Node::from_element(&element);
        let switch = find_by_key(&root, SWITCH_DEMO_KEY).expect("开关要在树上");
        let knob = find_by_key(switch, "knob").expect("把手要在开关子树里");
        let read = |p: &'static zephyra_core::property::PropertyKey| {
            knob.props()
                .iter_local()
                .find(|(k, _)| std::ptr::eq(*k, p))
                .and_then(|(_, v)| v.get::<f32>())
        };
        let bg_alpha = knob
            .props()
            .iter_local()
            .find(|(k, _)| std::ptr::eq(*k, &zephyra_core::props::BACKGROUND))
            .and_then(|(_, v)| v.get::<zephyra_core::core::Color>())
            .map(|c| c.a);
        (
            read(&zephyra_core::props::TRANSLATE_X),
            read(&zephyra_core::props::SCALE_X),
            read(&zephyra_core::props::SCALE_Y),
            bg_alpha,
        )
    };

    // 1) 初始（关）：位移 0、缩放 1、白罩 alpha = 1（未按住）。
    let (tx, sx, sy, bg) = knob_props(&g);
    assert_eq!(tx, Some(0.0), "关着的把手位移应为 0");
    assert_eq!(sx, Some(1.0), "关着的把手缩放应为 1");
    assert_eq!(sy, Some(1.0));
    assert_eq!(bg, Some(1.0), "未按住时白罩应不透明");

    // 2) 点一下并让动画跑完：位移 = fraction × 行程（20px）。
    g.click(SWITCH_DEMO_KEY);
    for _ in 0..600 {
        g.switch_demo.advance(16.0);
    }
    let (tx, sx, _, _) = knob_props(&g);
    assert_eq!(
        tx,
        Some(zephyra_ui::KNOB_TRAVEL),
        "开着的把手应位移一个行程"
    );
    assert_eq!(sx, Some(1.0), "没按住时缩放应为 1");

    // 3) 按住：把手放大到 1.5×（两个轴同一个值），白罩完全消失（alpha = 0）。
    g.switch_demo.set_pressed(true);
    for _ in 0..600 {
        g.switch_demo.advance(16.0);
    }
    let (_, sx, sy, bg) = knob_props(&g);
    assert_eq!(
        sx,
        Some(zephyra_ui::KNOB_PRESSED_SCALE),
        "按住时把手应放大（临界阻尼 1.5，停稳后恰为 1.5）"
    );
    assert_eq!(sy, Some(zephyra_ui::KNOB_PRESSED_SCALE));
    assert_eq!(bg, Some(0.0), "按住时白罩应完全消失（alpha = 0）");
}

/// 玻璃页「装饰」开关翻的是三块玻璃的 `DECORATIONS` 属性（none ↔ glass）——
/// 绑定断了，开关能点、模型是对的，玻璃块画面毫无差别：静默地错。
///
/// 默认是 **none**（见 `mod.rs` 里 `glass_decor` 的注释：边缘着色已由着色器画，
/// 旧装饰是重复的一层、每帧 1.6 ms）。
#[test]
fn the_glass_decor_toggle_flips_the_panes_decorations() {
    use zephyra_gallery::app::GLASS_DECOR_KEY;

    let theme = Theme::builtin(ThemeVariant::Dark);
    let ctx = Context::new(&theme);
    let mut g = Gallery::default();
    g.route = "materials";

    let decorations_of = |g: &Gallery, pane: &str| {
        let element = g.view(&ctx);
        let root = Node::from_element(&element);
        let glass = find_by_key(&root, pane).unwrap_or_else(|| panic!("要有 {pane} 这块玻璃"));
        // 先绑局部变量再返回（tail expression 的隐式临时 lifetime 会吃到 `element`）。
        let found = glass
            .props()
            .iter_local()
            .find(|(k, _)| std::ptr::eq(*k, &zephyra_core::props::DECORATIONS))
            .and_then(|(_, v)| v.get::<zephyra_core::decoration::Decorations>())
            .expect("玻璃块必须带 decoration 属性（即使是 none）");
        found
    };

    // 1) 默认**关**：三块玻璃都退回恒等（none，不进离屏 pass）。
    for pane in ["glass:a", "glass:b", "glass:c"] {
        assert!(
            decorations_of(&g, pane).is_identity(),
            "{pane} 默认不该带装饰 —— 玻璃的边缘着色由着色器画，旧装饰是 1.6 ms/帧 的重复层"
        );
    }

    // 2) 点一下：三块一起带上 glass 装饰。
    g.click(GLASS_DECOR_KEY);
    for pane in ["glass:a", "glass:b", "glass:c"] {
        assert!(
            !decorations_of(&g, pane).is_identity(),
            "{pane} 的装饰应恢复"
        );
    }

    // 3) 再点一下：又退回 none。
    g.click(GLASS_DECOR_KEY);
    for pane in ["glass:a", "glass:b", "glass:c"] {
        assert!(
            decorations_of(&g, pane).is_identity(),
            "{pane} 的装饰应退回 none"
        );
    }
}

// ------------------------------------------------------- 门禁：帧泵

/// `Gallery::wants_frames()` 的三个源都要能把它点亮，且静止后必须精确熄灭。
///
/// # 为什么这条门禁的分量在宿主侧
///
/// D3D12 / Vulkan 的保留式节奏是「无事件不画帧」：`tick()` 里只有 `dirty`
/// 才产帧，事件循环平时 `Wait`。视图回答不出「还要帧」时，动画只在鼠标
/// 事件那一拍推进一格，随后循环睡死 —— 画面上的把手是**跳**过去的，
/// 整段弹簧曲线看不见（用户原话「动画有问题」）。反过来，若熄灭不及时，
/// 窗口会永远 Poll 空转，静止时烧 GPU。两侧都是静默故障。
#[test]
fn wants_frames_covers_every_animation_source() {
    let mut g = Gallery::default();

    // 1) 入场动画在读秒：必须泵帧。
    g.enter_ms = 0.0;
    assert!(g.wants_frames(), "入场动画期间必须继续泵帧");

    // 2) 入场跑满、开关静止：一帧都不画。
    g.enter_ms = zephyra_style::anim::ENTER_MS;
    assert!(
        !g.wants_frames(),
        "完全没有动画时不该泵帧（静止零帧是保留式 UI 的卖点）"
    );

    // 3) 开关翻转后弹簧未收敛：必须泵帧。
    g.switch_demo.toggle();
    assert!(g.wants_frames(), "开关弹簧未收敛时必须继续泵帧");

    // 4) 收敛后熄灭。
    for _ in 0..600 {
        g.switch_demo.advance(16.0);
    }
    assert!(!g.wants_frames(), "开关收敛后必须精确熄灭");

    // 5) 玻璃页那个装饰开关走的是同一条模型，也要被算进来。
    g.glass_decor.toggle();
    assert!(g.wants_frames(), "第二个开关的弹簧同样要泵帧");
    for _ in 0..600 {
        g.glass_decor.advance(16.0);
    }
    assert!(!g.wants_frames(), "两个开关都收敛后才熄灭");
}

// ------------------------------------------------------- 门禁：i18n 深度融合

/// 模板里承载**面向用户的文字**的属性名。
///
/// # 为什么按**属性名**判，而不是按元素名
///
/// 同一个 `label` 在 `Button` 上是按钮文字、在 `DemoRow` 上是行首标签、
/// 在 `Section` 上根本不存在 —— 三者都是面向用户的文字，一律同判。
/// 按元素名判就要维护一张「哪些元素的哪个属性是文案」的表，而那张表
/// 本身会漂移（新组件没人登记）。
const TEXT_ATTRS: &[&str] = &[
    "label",
    "title",
    "text",
    "placeholder",
    "hint",
    "desc",
    "tip",
];

/// 应用模板里**允许**出现的裸字面量（白名单）。
///
/// # 为什么要有白名单
///
/// 有些字面量**本该**是字面量：命令行、符号、数字。它们不是「漏翻的散文」，
/// 翻译它们只会产出怪东西（把 `-` 译成别的符号、把 `cargo run …` 译成
/// 一句读不通的话）。
///
/// # 为什么白名单按**值**而不是按位置
///
/// 按位置（行号 / 属性序号）的话，改一行模板就全乱；按值的话，新增一个
/// 命令行只需要往表里加一行 —— 而门禁的报错信息会把它列出来，不会让人
/// 去猜是哪一处。
const LITERAL_ALLOWLIST: &[&str] = &[
    // ---- 命令行（测试页给的复现步骤；翻译它们没有意义）----
    "cargo run -p zstylec -- check pages.zstyle",
    "cargo run -p zephyra-gallery --bin gallery -- --dump-cpu",
    "  --route reference --lang zh-Hans",
    "  --family 3   # --list-families 列合法值",
    "cargo test -p zephyra-gallery",
    // ---- 符号（玻璃参数步进器的 − / +）----
    "-",
    "+",
    // ---- 数字（页头徽章的静态计数）----
    "12",
    // ---- 语言自称（English / 中文 / 日本語）----
    //
    // ⚠ **故意不进文案表** —— 自称不是翻译（同一种语言名在三种语言下是同一个
    // 字符串），进表要填 9 条两两相同的内容，而那 9 条迟早被误改成「翻译」。
    // 详见 `docs/developer/i18n.md` §5.2。
    "English",
    "中文",
    "日本語",
];

/// 剥掉模板里的 XML 块注释（`<!-- … -->`，可跨多行）。
///
/// 注释里写「这里该写 `text="…"`」是正当的，不剥会误报。
fn strip_zypui_comments(src: &str) -> String {
    let mut text = String::new();
    let mut rest = src;
    while let Some(start) = rest.find("<!--") {
        text.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    text.push_str(rest);
    text
}

/// 扫一份模板源码里所有「**裸**文案属性值」。返回 `(行号, 属性名, 值)`。
///
/// # 什么算「裸」
///
/// - `text="{tr('ui.x')}"` —— 表达式，**不算**；
/// - `text="{@bp_text}"` —— 绑定，**不算**；
/// - `label="ui.x"` —— 字面量，**算**（`label_key` 的语义 key 也算，
///   但那一条由另一条门禁单独查）；
/// - `label=""` —— 空串，**不算**（纯图标按钮）。
fn bare_text_literals(src: &str) -> Vec<(usize, String, String)> {
    let text = strip_zypui_comments(src);
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        for attr in TEXT_ATTRS {
            let pat = format!("{attr}=\"");
            let mut cursor = 0;
            while let Some(p) = line[cursor..].find(pat.as_str()) {
                let at = cursor + p;
                // 属性名必须是**完整**的：`subtitle="…"` 里也有 `title="`，
                // 不看前一个字符就会把 `subtitle` 当成 `title` 扫一遍。
                let prev = line[..at].chars().next_back();
                if prev.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                    cursor = at + pat.len();
                    continue;
                }
                let start = at + pat.len();
                let Some(end) = line[start..].find('"') else {
                    break;
                };
                let value = &line[start..start + end];
                cursor = start + end;
                if value.is_empty() || value.starts_with('{') {
                    continue;
                }
                out.push((idx + 1, (*attr).to_string(), value.to_string()));
            }
        }
    }
    out
}

/// 控件层模板里不许出现面向用户的裸文案。
///
/// # 为什么这条门禁在 **gallery** 里而不在控件层
///
/// 控件层自己测不了「我有没有发文案」—— 判据需要知道「什么是面向用户的
/// 文字」，而那份清单（`TEXT_ATTRS`）随应用变。放在应用侧，两边都在同一
/// 个 workspace 里，改一处两边同步。
///
/// # 为什么控件层**没有**白名单
///
/// 应用层至少有「命令行 / 符号 / 数字」这些天然的字面量，控件层没有 ——
/// 控件发的每个字都是给人看的。控件层哪天真的需要例外（例如内置一个
/// "×" 关闭符号），再在这里加白名单，并写清理由。
#[test]
fn control_layer_templates_emit_no_user_facing_text() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ZephyraUI/ZephyraUI/ui/controls.zypui");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 可读: {e}", path.display()));

    let bare = bare_text_literals(&src);
    let listed = bare
        .iter()
        .map(|(l, a, v)| format!("  第 {l} 行 {a}=\"{v}\""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        bare.is_empty(),
        "控件层模板里出现了面向用户的裸文案 —— 控件只发 key，翻译在控件内部发生：\n{listed}"
    );
}

/// 应用模板里不许出现面向用户的裸文案（白名单除外）。
///
/// # 为什么这条门禁现在才存在
///
/// 之前只有「用到的 key 有没有译文」（`every_tr_key_used_in_source_exists_in_every_language`），
/// 那是**单向**的：模板里写 `label="Toggle me"` 这种硬编码，它根本不是 key，
/// 门禁看不见它 —— 而切语言时它就是不变。
///
/// 症状特别难查：同一页里别的文字变中文了，就它不变，而它看上去完全正常。
#[test]
fn app_templates_emit_no_user_facing_text() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend/shell.zypui");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 可读: {e}", path.display()));

    let bare = bare_text_literals(&src);
    // 恒真防御：模板里一个文案属性都没有的话，这条门禁在假装通过。
    assert!(
        !bare.is_empty(),
        "模板里一个文案属性都没扫到 —— 扫描规则变了，这条门禁在假装通过"
    );

    let problems: Vec<String> = bare
        .iter()
        .filter(|(_, _, v)| !LITERAL_ALLOWLIST.contains(&v.as_str()))
        .map(|(l, a, v)| format!("  第 {l} 行 {a}=\"{v}\"（静态文案走 tr('…')，按钮走 label_key）"))
        .collect();
    assert!(
        problems.is_empty(),
        "应用模板里出现了面向用户的裸文案：\n{}",
        problems.join("\n")
    );
}

/// 每一个 `label_key` 的值，在**每一门语言**下都必须有译文。
///
/// # 键从哪来
///
/// 两个来源都扫，缺一不可：
///
/// 1. **模板里的字面量** —— `<Button label_key="ui.send"/>`；
/// 2. **Rust 模型里的 key** —— 走**模型本身**（不是扫源码）：
///    `g.check_group.header_label_key()` / `g.check_group.items()[i].label_key()`
///    / `g.demo_check.label_key()`。
///
/// 走模型而不是扫源码，是因为扫源码认不出「哪个实参是 label key」
/// （`Checkbox::new(key, label_key)` 两个都是 `&'static str`）—— 认错了
/// 就把事件 key 当成文案 key 去查表，**每次都红**，而正确的那个反而漏了。
///
/// # 为什么这条与 `every_tr_key_used_in_source_exists_in_every_language` 不是一条
///
/// 那条查的是 `tr('…')` 的键；这条查的是 `label_key` 的键。控件模板内部
/// 的 `tr(label_key)` 是**运行期**发生的，源码里扫不到 `tr('…')` ——
/// 漏了这条，按钮与复选框的文案就能无声地缺译。
#[test]
fn every_label_key_exists_in_every_language() {
    use zephyra_gallery::app::{messages, SUPPORTED};

    let mut keys: Vec<String> = Vec::new();
    let remember = |k: &str, keys: &mut Vec<String>| {
        if !keys.iter().any(|x| x == k) {
            keys.push(k.to_string());
        }
    };

    // 1) 模板里的 label_key 字面量。
    for rel in [
        "../ZephyraUI/ZephyraUI/ui/controls.zypui",
        "frontend/shell.zypui",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} 可读: {e}", path.display()));
        for line in strip_zypui_comments(&src).lines() {
            let mut cursor = 0;
            while let Some(p) = line[cursor..].find("label_key=\"") {
                let at = cursor + p;
                let prev = line[..at].chars().next_back();
                if prev.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                    cursor = at + 11;
                    continue;
                }
                let start = at + 11;
                let Some(end) = line[start..].find('"') else {
                    break;
                };
                let value = &line[start..start + end];
                cursor = start + end;
                if value.starts_with('{') {
                    continue; // 动态 key：由模型提供，见下面第 2 步
                }
                remember(value, &mut keys);
            }
        }
    }

    // 2) Rust 模型里的 key（模型是真相源，不扫源码）。
    let g = Gallery::default();
    remember(g.check_group.header_label_key(), &mut keys);
    for item in g.check_group.items() {
        remember(item.label_key(), &mut keys);
    }
    remember(g.demo_check.label_key(), &mut keys);
    // 开关族（参考页第三族）：开关的标签 key + 三个单选选项的标签 key。
    remember(g.switch_demo.label_key(), &mut keys);
    for option in g.radio_group.options() {
        remember(option.label_key(), &mut keys);
    }
    // 玻璃页「装饰」开关（默认开）。
    remember(g.glass_decor.label_key(), &mut keys);

    // 恒真防御：一个 key 都没收集到 ⇒ 门禁在假装通过。
    assert!(
        keys.len() >= 5,
        "只收集到 {} 个 label_key —— 收集规则变了，这条门禁在假装通过",
        keys.len()
    );

    let msgs = messages();
    let mut problems: Vec<String> = Vec::new();
    for key in &keys {
        for lang in SUPPORTED {
            if msgs.get(lang, key).is_none() {
                problems.push(format!("  {key} 在 {lang} 下缺译文"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "这些 label_key 在某一门语言下没有译文（控件会显示 key 本身）：\n{}",
        problems.join("\n")
    );
}

/// 控件层的 `KEY_PARAMS` 契约表必须与**控件模板**对得上。
///
/// # 这条门禁挡住什么
///
/// 契约表说「`Button` 的 `label_key` 实参收语义 key」，而模板里可能：
///
/// - **没声明这个实参** ⇒ 应用传 `label_key=` 会在**编译期**报错（zypuic 查）；
/// - **声明了却没人用** ⇒ 实参收下了没人翻译，画面上显示的是 **key 本身**
///   （`ui.send`）—— 而编译期不报错、运行期不崩、别的门禁也不红。
///
/// # 「没人用」怎么判
///
/// 两种用法都算「用上了」：
///
/// - **直接翻译**：体里有 `tr(label_key)`；
/// - **转发**：体里有 `label_key="{label_key}"`（原样传给子组件）。
///   `CheckBoxGroup` 就是转发者 —— 它把组头的 key 交给内层的 `CheckBox`。
///
/// 但转发必须有**终点**：整个契约表里至少有一个组件直接 `tr(param)`。
/// 否则「大家都在转发、没人翻译」也能过 —— 那正是这条门禁要挡的。
#[test]
fn the_control_layer_key_param_contract_matches_the_template() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ZephyraUI/ZephyraUI/ui/controls.zypui");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 可读: {e}", path.display()));

    /// 取一个组件的**声明头**（`<component name="X" …>`）与**体**。
    fn component_of(src: &str, name: &str) -> Option<(String, String)> {
        let decl = format!("<component name=\"{name}\"");
        let at = src.find(&decl)?;
        let head_end = at + src[at..].find('>')?;
        let head = src[at..head_end].to_string();
        let body_start = head_end;
        let body_end = src[body_start..]
            .find("</component>")
            .map(|e| body_start + e)
            .unwrap_or(src.len());
        Some((head, src[body_start..body_end].to_string()))
    }

    let mut problems: Vec<String> = Vec::new();
    let mut translated_somewhere = false;

    for (component, param) in zephyra_ui::i18n::KEY_PARAMS {
        let Some((head, body)) = component_of(&src, component) else {
            problems.push(format!("  控件层模板里没有组件 {component}"));
            continue;
        };
        let params = head
            .split("params=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("");
        if !params.split_whitespace().any(|p| p == *param) {
            problems.push(format!(
                "  组件 {component} 没声明实参 {param}（契约表说它收语义 key）"
            ));
            continue;
        }
        let direct = body.contains(&format!("tr({param})"));
        // 转发：`label_key="{label_key}"` —— 实参原样交给子组件。
        let forwards = body.contains(&format!("{param}=\"{{{param}}}\""));
        if direct {
            translated_somewhere = true;
        } else if !forwards {
            problems.push(format!(
                "  组件 {component} 收下了 {param} 却既不翻译也不转发 —— 画面上会显示 key 本身"
            ));
        }
    }

    assert!(
        translated_somewhere,
        "契约表里没有任何组件直接 `tr(param)` —— 全在转发、没人翻译"
    );
    assert!(
        problems.is_empty(),
        "控件层的 key 参数契约与模板对不上：\n{}",
        problems.join("\n")
    );
}

/// 切语言时，按钮与复选框的**可访问名**（= 画面上画出来的那几个字）必须跟着变。
///
/// # 为什么这条是「深度融合」的最终证明
///
/// 前面的门禁只查**静态**性质：key 在不在表里、模板里有没有裸文案。
/// 这条查的是**运行期**：同一棵树，把语言从 `en` 切到 `zh-Hans`，控件上的
/// 字必须变。
///
/// 它同时覆盖两条通道：
///
/// - **应用通道**（`ui.*`）：按钮的标签来自应用文案表；
/// - **控件层通道**（`zephyra.*`）：复选框组组头的「全选」来自**控件层自带**
///   的默认译文 —— 应用一个字都没写，它照样跟着语言变。
///
/// # 为什么用无障碍树来量而不是直接读属性
///
/// 无障碍名是从元素的 TEXT 属性派生的（`zephyra_uia::rules::accessible_name`），
/// 所以「可访问名变了」= 「画出来的字变了」。顺带还证明了**无障碍链路**
/// 拿到的是译文而不是 key —— 屏读器念的也是人话。
#[test]
fn switching_the_language_changes_the_button_and_checkbox_labels() {
    use zephyra_gallery::app::CHECK_GROUP_KEY;
    use zephyra_uia::WindowInfo;

    // `Gallery::route` 是 `&'static`（每帧重建的树不能持有运行期借用）。
    fn name_of(route: &'static str, family: usize, lang: usize, key: &str) -> String {
        let mut g = Gallery::default();
        g.route = route;
        g.ref_family = family;
        g.lang = lang;
        let theme = Theme::builtin(ThemeVariant::Dark);
        let ctx = Context::new(&theme);
        let root = Node::from_element(&g.view(&ctx));
        let win = WindowInfo {
            name: "Zephyra Gallery".into(),
            client: Rect::new(0.0, 0.0, 1400.0, 800.0),
        };
        let snap = zephyra_uia::rules::build(&root, &win, ctx);
        snap.find_by_key(key)
            .unwrap_or_else(|| panic!("{route}/{family}（lang={lang}）：{key} 必须在无障碍树上"))
            .name
            .clone()
    }

    // (路由, 族下标, key, 英文, 简体中文)
    let cases: &[(&str, usize, &str, &str, &str)] = &[
        // 组头：控件层自带的默认译文（应用没写任何东西）
        ("reference", 1, CHECK_GROUP_KEY, "Select all", "全选"),
        // 子项：应用文案表
        ("reference", 1, "check:alpha", "Enable feature", "启用功能"),
        (
            "reference",
            1,
            "check:locked",
            "Locked option",
            "锁定的选项",
        ),
        // 单个复选框（state 页）
        ("state", 0, "state-check", "Toggle me", "切换我"),
        // 按钮族
        ("reference", 0, "act:standard", "Standard", "标准"),
        ("reference", 0, "act:send", "Send", "发送"),
    ];

    for (route, family, key, en, zh) in cases {
        let got_en = name_of(route, *family, 0, key);
        let got_zh = name_of(route, *family, 1, key);
        assert_eq!(&got_en, en, "{route}/{key} 的英文文案");
        assert_eq!(&got_zh, zh, "{route}/{key} 的中文文案");
        assert_ne!(got_en, got_zh, "{route}/{key} 切语言后文案没变");
    }
}
