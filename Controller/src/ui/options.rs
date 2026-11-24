//! Page definitions and the saved choices that each page owns.
use super::previews;
use crate::{Region, Settings};
use eframe::egui::Ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Page {
    About,
    Drops,
    Names,
    Runes,
    Skills,
    Guides,
    Cube,
    Appearance,
}

pub(super) enum PageContent {
    About,
    Drops(&'static [DropGroup]),
    Preferences {
        toggles: &'static [Toggle],
        preview: Option<fn(&mut Ui, Region, &Settings)>,
    },
}

pub(super) struct Toggle {
    pub(super) label: &'static str,
    pub(super) detail: &'static str,
    pub(super) search_aliases: &'static str,
    field: &'static str,
    read: fn(&Settings) -> bool,
    edit: fn(&mut Settings) -> &mut bool,
}

impl Toggle {
    pub(super) fn is_available(&self, settings: &Settings) -> bool {
        settings.prerequisites_met(self.field)
    }

    pub(super) fn show_control(
        &self,
        ui: &mut Ui,
        settings: &mut Settings,
        draw: impl FnOnce(&mut Ui, &mut bool),
    ) {
        let available = self.is_available(settings);
        ui.add_enabled_ui(available, |ui| {
            if available {
                draw(ui, (self.edit)(settings));
            } else {
                draw(ui, &mut false);
            }
        });
    }
}

macro_rules! option {
    ($field:ident, $label:literal, aliases = $aliases:literal) => {
        option!($field, $label, "", $aliases)
    };
    ($field:ident, $label:literal, $detail:literal) => {
        option!($field, $label, $detail, "")
    };
    ($field:ident, $label:literal, $detail:literal, $aliases:literal) => {
        Toggle {
            label: $label,
            detail: $detail,
            search_aliases: $aliases,
            field: stringify!($field),
            read: |settings| settings.$field,
            edit: |settings| &mut settings.$field,
        }
    };
}

#[derive(Clone, Copy)]
enum PageChoice {
    Toggle(&'static Toggle),
    RuneBeam(usize),
}

impl PageChoice {
    fn read(self, settings: &Settings) -> bool {
        match self {
            Self::Toggle(toggle) => (toggle.read)(settings),
            Self::RuneBeam(index) => settings.rune_beams[index],
        }
    }

    fn edit(self, settings: &mut Settings) -> &mut bool {
        match self {
            Self::Toggle(toggle) => (toggle.edit)(settings),
            Self::RuneBeam(index) => &mut settings.rune_beams[index],
        }
    }
}

pub(super) struct DropGroup {
    pub(super) title: &'static str,
    pub(super) aliases: &'static str,
    pub(super) rows: &'static [&'static [Toggle]],
}

impl DropGroup {
    pub(super) fn toggles(&self) -> impl Iterator<Item = &'static Toggle> + '_ {
        self.rows.iter().flat_map(|row| row.iter())
    }
}

pub(super) const DROP_GROUPS: &[DropGroup] = &[
    DropGroup {
        title: "咒符",
        aliases: "护身符 護身符 护符 護符 神符 charm",
        rows: &[
            &[
                option!(small_charm_beam, "魔法小型咒符", aliases = "小型神符 小型护身符 小护符 小护身符 小護身符 small charm"),
                option!(annihilus_beam, "滅絕（暗金小型咒符）", aliases = "灭绝 毁灭 毀滅 annihilus"),
            ],
            &[
                option!(large_charm_beam, "魔法大型咒符", aliases = "大型神符 大型护身符 中护符 中护身符 中型护身符 大型護身符 large charm"),
                option!(torch_beam, "地獄火炬（暗金大型咒符）", aliases = "地狱火炬 hellfire torch"),
            ],
            &[option!(grand_charm_beam, "特大咒符（含基德、破免）", aliases = "巨型神符 超大型护身符 大护符 大护身符 超大型護身符 基德的好运 基德的財運 破免板 grand charm gheed sunder")],
        ],
    },
    DropGroup {
        title: "珠宝",
        aliases: "珠寶 jewel",
        rows: &[&[
            option!(jewel_beam, "魔法与稀有珠宝", aliases = "蓝黄 蓝珠宝 黄珠宝 魔法珠宝 蓝色珠宝 黄色珠宝 亮金 藍色 黃色 魔法珠寶 稀有珠寶 magic rare jewel"),
            option!(unique_jewel_beam, "暗金珠宝", aliases = "独特珠宝 獨特珠寶 暗金珠寶 彩虹刻面 彩虹雕面 巨型 巨大 unique jewel rainbow facet colossal"),
        ]],
    },
];

impl Page {
    pub const ALL: [Self; 8] = [
        Self::About,
        Self::Drops,
        Self::Names,
        Self::Runes,
        Self::Skills,
        Self::Guides,
        Self::Cube,
        Self::Appearance,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::About => "模组说明",
            Self::Drops => "掉落光柱",
            Self::Names => "物品名称",
            Self::Runes => "符文显示",
            Self::Skills => "技能护眼",
            Self::Guides => "场景指引",
            Self::Cube => "迷你魔盒",
            Self::Appearance => "界面外观",
        }
    }

    pub(super) fn content(self) -> PageContent {
        match self {
            Self::About => PageContent::About,
            Self::Drops => PageContent::Drops(DROP_GROUPS),
            Self::Names => PageContent::Preferences {
                toggles: &[
                    option!(
                        english_names,
                        "英文名称对照",
                        "为暗金、套装、符文和符文之语附上英文原名（仅中文界面有效）"
                    ),
                    option!(max_hints, "MAX上限提示", "在装备名称下列出可变属性的最高值"),
                    option!(base_tiers, "底材阶级", "在底材名后标出普通、扩展或精英阶级"),
                    option!(
                        armor_weights,
                        "护甲与盾牌重量",
                        "在身体护甲和盾牌的名称后标出轻型、中型或重型"
                    ),
                    option!(
                        item_colors,
                        "无形与有孔名称配色",
                        "无形无孔的名称用淡青色，有孔的名称用淡粉紫色"
                    ),
                ],
                preview: Some(previews::names),
            },
            Self::Runes => PageContent::Preferences {
                toggles: &[
                    option!(rune_numbers, "名称编号", "在符文名称后标出编号"),
                    option!(rune_icons, "背包图标编号", "在背包的符文图标右上角显示编号"),
                    option!(
                        rune_recipes,
                        "升级材料提示",
                        "在#10至#32的符文名下，列出升级所需的符文和宝石"
                    ),
                ],
                preview: Some(previews::runes),
            },
            Self::Skills => PageContent::Preferences {
                toggles: &[
                    option!(
                        sorceress_eye_care,
                        "法师雷云风暴",
                        "减弱放电与落雷的闪光，保留脚下电环和命中效果"
                    ),
                    option!(
                        assassin_eye_care,
                        "刺客元素武学",
                        "减弱烈焰拳、雷电爪、寒冰刃与凤凰攻击释放时的闪光"
                    ),
                ],
                preview: None,
            },
            Self::Guides => PageContent::Preferences {
                toggles: &[
                    option!(waypoints, "小站指路", "用光效标出传送小站的位置"),
                    option!(
                        entrances,
                        "地图入口",
                        "用光效标出通往相邻区域的洞口、楼梯和通道"
                    ),
                    option!(
                        tomb_symbols,
                        "塔拉夏古墓符号",
                        "在七座古墓入口点亮各自的符号"
                    ),
                    option!(
                        hall_directions,
                        "沃特之厅方向",
                        "用光效指示通往尼拉塞克的方向"
                    ),
                    option!(
                        horazon_journal,
                        "赫拉森日誌",
                        "用光效标出神秘避难所中的日誌书台"
                    ),
                    option!(
                        quest_books,
                        "任务书光晕",
                        "为女伯爵古书与藍伊森之書的书页添上淡金色光晕"
                    ),
                ],
                preview: None,
            },
            Self::Cube => PageContent::Preferences {
                toggles: &[
                    option!(
                        mini_cube,
                        "启用迷你魔盒",
                        "从背包或手柄入口打开迷你魔盒窗口"
                    ),
                    option!(
                        auto_open_cube,
                        "进入游戏自动打开",
                        "每局开始时自动展开迷你魔盒"
                    ),
                ],
                preview: None,
            },
            Self::Appearance => PageContent::Preferences {
                toggles: &[
                    option!(custom_font, "中文自定义字体", "将游戏的中文字体换为圆体"),
                    option!(
                        hud_style,
                        "经验条外观",
                        "经验条每10%标出一格，从金黄渐变为橙红"
                    ),
                    option!(
                        health_bars,
                        "怪物血条与佣兵名称",
                        "键鼠下加宽怪物血条，词条在下方居中显示。国服佣兵名称采用国际服的字体和字距"
                    ),
                ],
                preview: None,
            },
        }
    }

    fn visit_choices(self, rune_count: usize, mut visit: impl FnMut(PageChoice)) {
        match self.content() {
            PageContent::About => {}
            PageContent::Drops(groups) => {
                for index in 0..rune_count {
                    visit(PageChoice::RuneBeam(index));
                }
                for toggle in groups.iter().flat_map(DropGroup::toggles) {
                    visit(PageChoice::Toggle(toggle));
                }
            }
            PageContent::Preferences { toggles, .. } => {
                for toggle in toggles {
                    visit(PageChoice::Toggle(toggle));
                }
            }
        }
    }

    pub fn restore(self, settings: &mut Settings) {
        let defaults = Settings::default();
        self.visit_choices(defaults.rune_beams.len(), |choice| {
            *choice.edit(settings) = choice.read(&defaults);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_controls_follow_the_template_prerequisites() {
        let catalog: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../ModTemplate/visuals.json")).unwrap();
        let mut toggles = std::collections::BTreeMap::new();
        for page in Page::ALL {
            page.visit_choices(0, |choice| {
                if let PageChoice::Toggle(toggle) = choice {
                    toggles.insert(toggle.field, toggle);
                }
            });
        }
        for (owner, group) in catalog["overrides"].as_object().unwrap() {
            let control = toggles[owner.as_str()];
            let mut settings = crate::enabled_features();
            *(control.edit)(&mut settings) = false;
            assert!(control.is_available(&settings), "{owner}");
            if let Some(required) = group["requires"].as_array() {
                for name in required {
                    let name = name.as_str().unwrap();
                    let prerequisite = toggles[name];
                    *(prerequisite.edit)(&mut settings) = false;
                    assert!(!control.is_available(&settings), "{owner} requires {name}");
                    *(prerequisite.edit)(&mut settings) = true;
                }
            }
        }
    }

    #[test]
    fn each_saved_choice_belongs_to_one_page() {
        let defaults = Settings::default();
        let mut edited = defaults.clone();
        let mut count = 0;
        for page in Page::ALL {
            page.visit_choices(defaults.rune_beams.len(), |choice| {
                let original = choice.read(&defaults);
                assert_eq!(
                    choice.read(&edited),
                    original,
                    "duplicate choice on {page:?}"
                );
                *choice.edit(&mut edited) = !original;
                count += 1;
            });
        }
        let fields = serde_json::to_value(defaults).unwrap();
        let saved_choices: usize = fields
            .as_object()
            .unwrap()
            .values()
            .map(|value| value.as_array().map_or(1, Vec::len))
            .sum();
        assert_eq!(count, saved_choices);
    }

    #[test]
    fn restoring_drops_resets_every_named_option_and_rune_slot() {
        let defaults = Settings::default();
        let mut edited = defaults.clone();
        for option in DROP_GROUPS.iter().flat_map(DropGroup::toggles) {
            *(option.edit)(&mut edited) = !(option.read)(&defaults);
        }
        edited.rune_beams = defaults.rune_beams.map(|enabled| !enabled);
        Page::Drops.restore(&mut edited);
        assert_eq!(edited, defaults);
    }

    #[test]
    fn restoring_one_page_preserves_other_drafts() {
        let defaults = Settings::default();
        let mut draft = defaults.clone();
        draft.max_hints = !defaults.max_hints;
        draft.mini_cube = !defaults.mini_cube;
        draft.rune_beams[0] = !defaults.rune_beams[0];
        Page::Names.restore(&mut draft);
        assert_eq!(draft.max_hints, defaults.max_hints);
        assert_ne!(draft.rune_beams[0], defaults.rune_beams[0]);
        assert_ne!(draft.mini_cube, defaults.mini_cube);
        Page::Drops.restore(&mut draft);
        assert_eq!(draft.rune_beams[0], defaults.rune_beams[0]);
        assert_ne!(draft.mini_cube, defaults.mini_cube);
    }
}
