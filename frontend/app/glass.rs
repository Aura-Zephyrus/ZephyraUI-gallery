// ------------------------------------------------------- 液态玻璃的折射参数（frontend）

/// 折射的四个可调参数 —— 全部以**档位下标**存储，值与显示文本都查表。
///
/// # 为什么存下标而不是 `f32`
///
/// `view()` 每帧重建整棵树，运行期 `format!("{:.2}", v)` 会走 `Box::leak`
/// 每帧一次（见 `pages::basics::spin_text` 的理由）。而这一页的参数是要
/// **每帧读**的（画在玻璃块上、也画在参数面板上）。
///
/// 存下标让「数值」与「文本」都变成一次数组查表 —— 零分配、零泄漏，
/// 而且**加减只是 `±1` 后 clamp**，不需要浮点误差累积。
///
/// # 为什么不是「四个自由 f32」
///
/// 那样步进要写 `(v + 0.05).min(1.0)`，而浮点累加在边界上会漂
/// （0.05 加 20 次到 1.0000001，然后 `.min(1.0)` 与不等号判据打架）。
/// 档位把这件事变成整数运算。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glass {
    /// 用哪条**算法**（见 [`GlassMode`]）。
    pub mode: GlassMode,
    /// 玻璃**厚度**（像素）档位，见 [`THICKNESS`]。
    pub thickness: usize,
    /// 倒角带**宽度**（像素）档位，见 [`BEZEL`]。
    pub bezel: usize,
    /// **色散**量档位，见 [`DISPERSION`]。
    pub dispersion: usize,
    /// **折射率**（IOR）档位，见 [`IOR`]。
    pub ior: usize,
}

impl Default for Glass {
    /// 默认值：厚 48px、倒角带 40px、几乎不色散、IOR 1.50。
    ///
    /// 为什么不取全 0：全 0 时 `strength = 0` 会让整条滤镜链**恒等**，
    /// 于是玻璃块根本不进离屏 pass —— 那一帧看起来像「折射没接上」，
    /// 而真正的原因只是「厚度调成 0 了」。默认给一个能看见效果的组合。
    ///
    /// # ⚠ 48 / 40 是**算 + 看**定下来的，别再往回调
    ///
    /// 上一版默认是 28 / 26。按 `PSRefraction` 的式子代入 220×120 的玻璃块算它的
    /// 位移剖面：峰值 13px，而位移在**距边 1px** 内就从 0 爬到峰值、13px 处就
    /// 衰减到 1px 量级 —— 弯曲被塞进一条 1~3px 宽的窄缝里。放到 25px 一格的
    /// 图案上，观感是「格子少了一根」，**读不出「被掰弯」**，整块玻璃像贴上去的
    /// 贴纸（改前的真 GPU 截图正是这个观感：三块玻璃只有边缘一条窄接缝）。
    ///
    /// 把两个数一起抬到 48 / 40 之后：峰值 ~23px，衰减拉宽到 ~20px
    /// （`t = -d/bezel`，所以倒角带宽直接决定弯曲在屏幕上有多少像素宽）。
    /// 同法算得三块玻璃的位移峰值约 **14 / 23 / 34px**（薄 / 普通 / 厚，
    /// `PANE_THICKNESS_OFFSETS` 各差 16px 厚度）；改后的真 GPU 截图上，边缘的
    /// 格子被压成窄带、往内一格被放大 —— 弯折一眼看得见。
    ///
    /// ⚠ 厚度与倒角带**必须一起动**：只抬厚度会让 23px 的位移仍然塞在 1~3px 的
    /// 窄缝里（更狠，但还是接缝不是弯曲）；只抬倒角带则弯曲变宽了却没有幅度。
    /// 抬到 48 / 40 之后再往上（60+）时，「厚」那块的位移会吃掉玻璃块高度的
    /// 四成以上，画面变成一团涂抹 —— 那是**参数演示**该给用户去拧的，不是默认值。
    fn default() -> Self {
        Self {
            // 默认走原版液态玻璃（AndroidLiquidGlass 的 `lens()` 逐字转译）——
            // 背景真的被掰弯、边缘有 7 通道色散，这才是「液态玻璃」该有的样子。
            // 便宜路（`PixelOffset`）仍能一键切到，只是不再当默认值。
            mode: GlassMode::Original,
            thickness: 12,
            bezel: 19,
            dispersion: 4,
            ior: 10,
        }
    }
}

/// 玻璃块的**两种算法**。
///
/// # 三种算法
///
/// - [`GlassMode::Original`] = **原版液态玻璃**（`Filter::Lens`）：逐字转译
///   AndroidLiquidGlass 的 `RoundedRectRefractionWithDispersionShaderString`。
///   背景沿 SDF 倒角带被`circleMap` 位移掰弯，边缘有 7 通道分色（红/橙/黄/绿/
///   青/蓝/紫按 `(cx·cy)/(hx·hy)` 加权）—— 这才是原版观感。默认档。
/// - [`GlassMode::Physical`] = **斯涅尔折射**（`Filter::Refraction`）：本仓自己的
///   折射公式，四个物理量（厚度 / 倒角 / 色散 / IOR）都可调，色散是 R/B 相对 G
///   的均匀偏移（边缘一圈彩虹，与原版的角部分色不同）。
/// - [`GlassMode::Cheap`] = **SVG 式像素偏移**：掰形状自己的边，不采样背景 ⇒
///   不开离屏 pass ⇒ 几乎免费，但背景**不被掰弯**。
///
/// # 为什么把原版设为默认
///
/// 用户反馈「玻璃材质控件和原版液态玻璃差别太大」：廉价路没有背景折射、
/// 物理路的均匀色散又是整圈彩虹 —— 两者都不是原版观感。原版那条路
/// (`Filter::Lens`) 在三个后端**早已逐字实现**（见 `zephyra.hlsl` 的 `PSLens` /
/// `zephyra.wgsl` 的 `fs_lens`），只是没接到这块玻璃材质上。
///
/// # 代价
///
/// 折射要读「背后」，因此需要一份目标副本：实测（gallery 1400×800 / Intel
/// UHD 630 / D3D12 / MSAA 开）材质页三块玻璃走离屏折射约 **5.06 ms/帧**，
/// 廉价路 ~0。要省预算就切到 [`GlassMode::Cheap`] —— 那条路完整保留。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassMode {
    /// 原版液态玻璃（`lens()` 逐字转译，默认）。
    #[default]
    Original,
    /// SVG 式像素偏移（便宜）。
    Cheap,
    /// 物理折射（斯涅尔）。
    Physical,
}

impl Glass {
    /// 当前档位对应的 `Filter::Refraction`。
    ///
    /// # 为什么这里可以带**数值**而其它属性不行
    ///
    /// 本仓的规矩是「外观不进源码」，门禁 `the_whole_tree_writes_no_appearance_in_source`
    /// 按 `iter_local()` 逐个属性查。它列的 19 个 APPEARANCE 里**没有 `FILTER`** ——
    /// 而这是对的：
    ///
    /// `FILTER` 的四个参数是**物理量**（玻璃厚度、折射率、色散、倒角带宽度），
    /// 不是**样式**（颜色、间距、圆角）。它们没有 `@令牌` 对应物，也不会有
    /// 主题版本 —— 一块玻璃有多厚不取决于界面是深色还是浅色。
    ///
    /// 更关键的是**这个 demo 的职责就是让人调它们**。参数写进 `.zstyle`
    /// 就只剩三档静态值，「折射允许调参数」直接不成立。
    ///
    /// `PropertyValue::Filter` 参与 `PartialEq`（`property.rs` 的相等表里有一项），
    /// 所以改参数能被 `diff` 正确识别为「这棵树变了」——不需要额外的失效通道。
    pub fn filter(&self) -> zephyra_core::filter::Filter {
        self.filter_with_thickness_offset(0)
    }

    /// 厚度按 `offset` 挪几档后的滤镜。
    ///
    /// # 为什么要「偏移」而不是三个独立参数
    ///
    /// 三块玻璃（薄 / 常规 / 厚）**共用**面板调的那一组参数 —— 拖一次面板，
    /// 三块**一起**变（这页要教的是**单项**参数的因果：厚度影响多少像素位移、
    /// IOR 到 1 时位移消失）。而厚度**各不相同** —— 那是「薄 / 常规 / 厚」
    /// 这三个标题**名副其实**的唯一办法。
    ///
    /// 早先那一版三块玻璃用的是**完全相同**的滤镜，标题却写着 Thin / Normal /
    /// Thick —— 画面上三块一模一样，标题在**说谎**。
    ///
    /// # 偏移量按**档位**给，不按像素
    ///
    /// 厚度存的是档位下标（见 [`Glass`]），所以偏移也是档位数：厚度的档位
    /// 步长是 4px，偏移 4 档 = 16px。
    pub fn filter_with_thickness_offset(&self, offset: isize) -> zephyra_core::filter::Filter {
        let idx =
            (self.thickness as isize + offset).clamp(0, THICKNESS.len() as isize - 1) as usize;
        match self.mode {
            GlassMode::Original => zephyra_core::filter::Filter::Lens {
                // 原版 `lens(refractionHeight, refractionAmount)`：倒角带宽度决定
                // 掰弯在屏幕上的铺开宽度，厚度决定位移峰值。两档都按 1/2 缩放，
                // 让默认值落在原版 `lens(12, 24)` 的量级（48→24、40→20）。
                refraction_height: BEZEL[self.bezel] * 0.5,
                refraction_amount: THICKNESS[idx] * 0.5,
            },
            GlassMode::Cheap => zephyra_core::filter::Filter::PixelOffset {
                // ⚠ 像素偏移的 `amount` 是**形状位移**（把边往里推），
                // 而物理折射的 `strength` 是**折射位移**（掰背景）—— 量级完全不同。
                // 同一张档位表乘 1/4 之后落在「看得见有厚度、形状还站得住」的区间
                // （玻璃块 220×120 ⇒ 短边半高 60，着色器另夹一道 40% ⇒ 24px 上限）。
                amount: THICKNESS[idx] * 0.25,
                // 作用带宽度同理乘 1/2：折射的倒角带是 2..48px，
                // 而像素偏移的作用带超过元素尺寸就没有意义了。
                bezel: BEZEL[self.bezel] * 0.5,
            },
            GlassMode::Physical => zephyra_core::filter::Filter::Refraction {
                strength: THICKNESS[idx],
                bezel: BEZEL[self.bezel],
                dispersion: DISPERSION[self.dispersion],
                ior: IOR[self.ior],
            },
        }
    }

    /// 便宜模式下面板显示的两个值（位移幅度 / 作用带宽度）。
    ///
    /// ⚠ 物理折射的四个参数（厚度 / 倒角 / 色散 / IOR）在便宜模式下**没有对应物**
    /// —— 像素偏移不掰背景，所以既没有色散也没有折射率。
    /// ⇒ 便宜模式只给**两个**旋钮，而不是把那两个旋钮留成「点了不动」的摆设。
    pub fn cheap_texts(&self) -> (&'static str, &'static str) {
        (
            OFFSET_AMOUNT_TEXT[self.thickness],
            OFFSET_BEZEL_TEXT[self.bezel],
        )
    }

    /// 四个参数各自的显示文本（查表，见 [`Glass`] 的理由）。
    pub fn texts(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        (
            THICKNESS_TEXT[self.thickness],
            BEZEL_TEXT[self.bezel],
            DISPERSION_TEXT[self.dispersion],
            IOR_TEXT[self.ior],
        )
    }

    /// 某一个参数加/减一档，自动夹在合法范围内。
    ///
    /// # 返回 `bool`（有没有真的动）
    ///
    /// 面板上的 `[−] [+] 在边界上是 no-op —— 而「点了没反应」必须与
    /// 「点了但被 clamp 回同一个值」区分得开，否则门禁
    /// `glass_params_are_not_hard_coded` 会把「clamp 生效」误判成「参数没接线」。
    pub fn nudge(&mut self, which: Param, delta: i32) -> bool {
        let slot = match which {
            Param::Thickness => &mut self.thickness,
            Param::Bezel => &mut self.bezel,
            Param::Dispersion => &mut self.dispersion,
            Param::Ior => &mut self.ior,
        };
        let max = match which {
            Param::Thickness => THICKNESS.len() - 1,
            Param::Bezel => BEZEL.len() - 1,
            Param::Dispersion => DISPERSION.len() - 1,
            Param::Ior => IOR.len() - 1,
        };
        // `as i64` 是必要的：`delta` 是 `i32`，而 `*slot` 可到 60，两者相加在
        // 理论上不溢出，但 `saturating_add` 的参数类型要求两边同宽。
        let next = (*slot as i64 + delta as i64).clamp(0, max as i64);
        let moved = next as usize != *slot;
        *slot = next as usize;
        moved
    }
}

/// 三块玻璃各自的**厚度档位偏移**（相对面板调的那一档）。
///
/// # 为什么是偏移而不是三组独立参数
///
/// 三块玻璃共用面板调的那一组参数（拖一次，三块一起变），但厚度**各不相同**
/// —— 那是「薄 / 常规 / 厚」三个标题**名副其实**的唯一办法。详见
/// [`Glass::filter_with_thickness_offset`]。
///
/// # 为什么是 ±4 档
///
/// 厚度的档位步长是 4px，所以 ±4 档 = **±16px**。按着色器式子算得三块玻璃的
/// 位移峰值（默认档：厚度 48 / 倒角带 40 / ior=1.50，玻璃块 220×120）约
/// **14 / 23 / 34px** —— 一眼能分出厚薄，而不会大到「厚的那块把画面揉成一团」
/// （34px ≈ 玻璃块高度的 28%）。
///
/// ⚠ 这三个数跟着 [`Glass::default`] 走：默认档调小（上一版是 28/26）时算出来
/// 是 3 / 13 / 26px，那时「薄」那块只有 3px —— 在 25px 一格的图案上**等于没有**，
/// 三个标题就成了三个长得一模一样的方块。调默认档时把这里一起改。
pub const PANE_THICKNESS_OFFSETS: [isize; 3] = [-4, 0, 4];

/// 四个参数之一（用来索引 [`Glass::nudge`] 与 click 分派）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    Thickness,
    Bezel,
    Dispersion,
    Ior,
}

impl Param {
    /// 四个参数的稳定顺序 —— **门禁与样式表都按这个顺序**。
    ///
    /// 显式给 `const` 而不是靠枚举的声明序：后者一旦有人在中间插一个变体，
    /// 面板顺序就会静默改变，而画面上「还是那四个，只是换了位置」。
    pub const ALL: [Param; 4] = [
        Param::Thickness,
        Param::Bezel,
        Param::Dispersion,
        Param::Ior,
    ];

    /// 参数名（面板上的标签）。
    pub fn label(self) -> &'static str {
        match self {
            Param::Thickness => "Thickness",
            Param::Bezel => "Bezel",
            Param::Dispersion => "Dispersion",
            Param::Ior => "IOR",
        }
    }

    /// 参数名（面板上的标签）—— 按当前**算法**取。
    ///
    /// ⚠ 便宜模式下「Thickness」这个叫法是错的：像素偏移的 `amount` 不是厚度，
    /// 是**把边往里推多少像素**。两个算法共用同一个档位槽位，但那一槽的
    /// **物理含义不同** —— 标签必须跟着算法走，否则面板在说一件假事。
    pub fn label_for(self, mode: GlassMode) -> &'static str {
        match (mode, self) {
            // 原版透镜与像素偏移都没有 IOR / 色散旋钮：Thickness / Bezel 在这里
            // 分别映射成 `refraction_amount` / `refraction_height`（原版）与
            // `amount` / `bezel`（廉价路），统一叫 Amount / Bezel 比叫「厚度」诚实。
            (GlassMode::Cheap | GlassMode::Original, Param::Thickness) => "Amount",
            _ => self.label(),
        }
    }

    /// 该参数的**唯一 key 前缀**（`glass-<name>` 与 `glass-<name>:dec`）。
    pub fn slug(self) -> &'static str {
        match self {
            Param::Thickness => "thickness",
            Param::Bezel => "bezel",
            Param::Dispersion => "dispersion",
            Param::Ior => "ior",
        }
    }

    /// 该参数在 [`Glass::filter`] 四个字段里的**下标**（0..=3）。
    ///
    /// # 为什么门禁需要它
    ///
    /// 门禁要断的是「挪了 `Thickness`，树上变的**只有** thickness」——
    /// 而「四个字段全都变了」也会绿。一条只断「全都变了」的判据挡不住
    /// `nudge` 里少写一个字段、或者反过来错位写到别人字段上的形状。
    /// 有了这个下标，门禁才能逐字段断言「该变的变了，不该变的没变」。
    pub const fn index(self) -> usize {
        match self {
            Param::Thickness => 0,
            Param::Bezel => 1,
            Param::Dispersion => 2,
            Param::Ior => 3,
        }
    }
}

/// 厚度档位（像素）：0..=80，步长 4。
///
/// # 为什么上限是 80
///
/// 玻璃块本身 220px 宽 / 120px 高（`pages.zstyle` 的 `.glass-pane`）。
/// 位移是 `h · 厚度 · sin(Δ)/cos(Θr)`，`h ∈ [0,1]`，于是**位移以厚度为上界**
/// （`Filter::Refraction::outsets` = 厚度那条契约就是照着它写的；代入公式算，
/// 峰值约为厚度的一半，默认档 48px 厚时峰值 ~23px）。再大的厚度只是把整块玻璃推向
/// 「背景被整个吃掉」—— 调到 80 时「厚」那块已经把 120px 高的玻璃块抹掉了
/// 六成以上，那是演示极限，不是能用的默认值。
pub const THICKNESS: [f32; 21] = [
    0.0, 4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0, 44.0, 48.0, 52.0, 56.0, 60.0,
    64.0, 68.0, 72.0, 76.0, 80.0,
];

/// 倒角带档位（像素）：2..=48，步长 2。
///
/// 下限 2 而不是 0：倒角带宽为 0 时 `t = -d / 0` 会除零，而着色器里的
/// `max(color.g, 1.0)` 只把它夹到 1 —— 那意味着「带外全部落到带内」，
/// 整块玻璃都在强折射区。
///
/// # ⚠ 上限 48 的依据是**半尺寸**，不是圆角半径
///
/// 着色器里夹的是 `min(bezel, half_size.x, half_size.y)` —— 默认玻璃块
/// 220×120 ⇒ 半尺寸 110×60 ⇒ 倒角带最多用到 60px。本表只到 48，
/// 所以每一档都是**真的生效**的。
///
/// ⚠ 这里一度写着「会被圆角半径（12px）压一次，所以超过 12 的档位看不出差别」
/// —— 那是**倒角带按圆角半径夹**那版的老话。按 `t = -d/bezel` 算：默认档从 26
/// 抬到 40（= 圆角半径的三倍多），弯折在屏幕上的铺开宽度从 ~13px 拉到 ~20px
/// —— 抬得动，不是白抬。旧注释会让人以为「调大倒角带是白调」，于是把默认值也一起
/// 调回去 —— 那正是「玻璃像贴纸」的老毛病复发的路子。
pub const BEZEL: [f32; 24] = [
    2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 30.0, 32.0,
    34.0, 36.0, 38.0, 40.0, 42.0, 44.0, 46.0, 48.0,
];

/// 色散档位：0.00..=1.00，步长 0.05。
///
/// 上限 1.0 不是「最大色散」，而是「R / B 相对 G 的偏移比例 ±100%」——
/// 再大就只是把 R 拉得更远，不会更像玻璃。
pub const DISPERSION: [f32; 21] = [
    0.0, 0.05, 0.10, 0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60, 0.65, 0.70, 0.75,
    0.80, 0.85, 0.90, 0.95, 1.0,
];

/// 折射率档位：1.00..=2.00，步长 0.05。
///
/// # 1.00 与 2.00 这两端各代表什么
///
/// - `1.00` = **没有折射**。着色器里 `asin(sin(Θs) / 1.0) = Θs`，位移
///   恰好为 0 —— 于是玻璃变成一块纯透镜板，什么都不弯。这条档位是**诊断
///   工具**：`IOR = 1` 时画面若仍有位移，说明公式被改坏了。
/// - `2.00` 超出任何真实玻璃（钻石 2.42、水 1.33、冕牌玻璃 1.45~1.52）。
///   保留它是为了让「IOR 越大越硬」这条趋势能被推到尽头看。
///
/// 1.50 正好落在第 10 档 —— 那就是默认值。
pub const IOR: [f32; 21] = [
    1.00, 1.05, 1.10, 1.15, 1.20, 1.25, 1.30, 1.35, 1.40, 1.45, 1.50, 1.55, 1.60, 1.65, 1.70, 1.75,
    1.80, 1.85, 1.90, 1.95, 2.00,
];

/// 便宜模式下的**位移幅度**显示文本（= [`THICKNESS_TEXT`] × 1/4）。
///
/// 为什么要另开一张表而不是 `format!`：见 [`Glass`] 的理由（每帧重建的视图里
/// `format!` 会走 `Box::leak`）。
pub const OFFSET_AMOUNT_TEXT: [&str; 21] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
    "17", "18", "19", "20",
];

/// 便宜模式下的**作用带宽度**显示文本（= [`BEZEL_TEXT`] × 1/2）。
pub const OFFSET_BEZEL_TEXT: [&str; 24] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17",
    "18", "19", "20", "21", "22", "23", "24",
];

/// 各档的显示文本。
///
/// ⚠ 三张表与数值表**逐项对应**，长度必须相等 —— 门禁
/// `glass_param_tables_line_up` 钉住这一点。手写而不是 `format!` 的理由
/// 见 [`Glass`]。
pub const THICKNESS_TEXT: [&str; 21] = [
    "0", "4", "8", "12", "16", "20", "24", "28", "32", "36", "40", "44", "48", "52", "56", "60",
    "64", "68", "72", "76", "80",
];

/// 见 [`THICKNESS_TEXT`]。
pub const BEZEL_TEXT: [&str; 24] = [
    "2", "4", "6", "8", "10", "12", "14", "16", "18", "20", "22", "24", "26", "28", "30", "32",
    "34", "36", "38", "40", "42", "44", "46", "48",
];

/// 见 [`THICKNESS_TEXT`]。色散固定两位小数 —— 步长 0.05，一位小数会让
/// 「3」与「4」看起来一样。
pub const DISPERSION_TEXT: [&str; 21] = [
    "0.00", "0.05", "0.10", "0.15", "0.20", "0.25", "0.30", "0.35", "0.40", "0.45", "0.50", "0.55",
    "0.60", "0.65", "0.70", "0.75", "0.80", "0.85", "0.90", "0.95", "1.00",
];

/// 见 [`THICKNESS_TEXT`]。IOR 固定两位小数，且 1.50 / 1.00 这两个值对判读
/// 很关键（前者是普通玻璃，后者是「不折射」的诊断档）。
pub const IOR_TEXT: [&str; 21] = [
    "1.00", "1.05", "1.10", "1.15", "1.20", "1.25", "1.30", "1.35", "1.40", "1.45", "1.50", "1.55",
    "1.60", "1.65", "1.70", "1.75", "1.80", "1.85", "1.90", "1.95", "2.00",
];
