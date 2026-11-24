//! Select the display text for one item and locale from its template label.
use std::{borrow::Cow, io};

use serde_json::Value;

use crate::item_catalog::{
    native_locales, BaseGroup, ItemCatalog, LabelRecord, NameLine, Rune, RuneLabel,
};

const NAME_LOCALES: [&str; 3] = ["enUS", "zhTW", "zhCN"];

#[derive(Clone, Copy)]
pub(crate) struct LabelOptions {
    pub english_names: bool,
    pub max_hints: bool,
    pub base_tiers: bool,
    pub armor_weights: bool,
    pub rune_numbers: bool,
    pub rune_recipes: bool,
}

pub(crate) struct LabelRenderer<'a> {
    catalog: &'a ItemCatalog,
    options: LabelOptions,
    change_name_lines: bool,
    change_base_suffixes: bool,
    change_rune_names: bool,
}

impl<'a> LabelRenderer<'a> {
    pub(crate) fn new(catalog: &'a ItemCatalog, options: LabelOptions) -> Self {
        Self {
            catalog,
            options,
            change_name_lines: !(options.english_names && options.max_hints),
            change_base_suffixes: !(options.base_tiers && options.armor_weights),
            change_rune_names: !(options.english_names
                && options.rune_numbers
                && options.rune_recipes),
        }
    }

    pub(crate) fn keeps_template(&self) -> bool {
        !(self.change_name_lines || self.change_base_suffixes || self.change_rune_names)
    }

    pub(crate) fn bind(&self, record: &'a LabelRecord) -> io::Result<Label<'a>> {
        let lines = if self.change_name_lines {
            record.name_lines.as_slice()
        } else {
            &[]
        };
        let base = if self.change_base_suffixes {
            record.resolve_base_group(self.catalog)?
        } else {
            None
        };
        let rune = if self.change_rune_names {
            record.resolve_rune_label(self.catalog)?
        } else {
            None
        };
        Ok(Label {
            options: self.options,
            lines,
            base,
            rune,
        })
    }
}

pub(crate) struct Label<'a> {
    options: LabelOptions,
    lines: &'a [NameLine],
    base: Option<&'a BaseGroup>,
    rune: Option<(&'a RuneLabel, &'a Rune)>,
}

impl Label<'_> {
    pub(crate) fn locales<'a>(&'a self, row: &'a Value) -> impl Iterator<Item = &'a str> {
        let base_locales = self.base.into_iter().flat_map(move |_| native_locales(row));
        let name_locales = NAME_LOCALES.into_iter().filter(move |locale| {
            (!self.lines.is_empty() || self.rune.is_some())
                && !(self.base.is_some() && row.get(*locale).is_some())
        });
        base_locales.chain(name_locales)
    }

    pub(crate) fn text<'a>(&self, locale: &str, original: &'a str) -> io::Result<Cow<'a, str>> {
        let mut text = Cow::Borrowed(original);
        let localized_name = NAME_LOCALES.contains(&locale);
        if localized_name && !self.lines.is_empty() {
            let name = ItemName::parse(&text, self.lines, locale)?;
            text = Cow::Owned(name.render(self.options));
        }
        if let Some(base) = self.base {
            let suffixes = base
                .suffixes
                .get(locale)
                .ok_or_else(|| io::Error::other("missing base suffix declaration"))?;
            let name = text
                .strip_suffix(&suffixes.full)
                .ok_or_else(|| io::Error::other("unexpected base suffix"))?;
            if name.trim().is_empty() {
                return Err(io::Error::other("missing name before the base suffix"));
            }
            let suffix = match (self.options.base_tiers, self.options.armor_weights) {
                (true, true) => &suffixes.full,
                (true, false) => &suffixes.tier,
                (false, true) => &suffixes.weight,
                (false, false) => "",
            };
            text = Cow::Owned(format!("{name}{suffix}"));
        }
        if let Some((rule, rune)) = self.rune.filter(|_| localized_name) {
            text = Cow::Owned(render_rune(&text, rule, rune, locale, self.options)?);
        }
        Ok(text)
    }
}

/// The declared prefix belongs to the settings. The remaining localized text belongs to the game.
pub(crate) struct ItemName<'a> {
    roles: &'a [NameLine],
    max_hint: Option<&'a str>,
    pub(crate) english_name: Option<&'a str>,
    pub(crate) localized: &'a str,
}

impl<'a> ItemName<'a> {
    pub(crate) fn parse(
        original: &'a str,
        roles: &'a [NameLine],
        locale: &str,
    ) -> io::Result<Self> {
        let mut name = Self {
            roles,
            max_hint: None,
            english_name: None,
            localized: original,
        };
        for role in roles {
            let slot = match role {
                NameLine::EnglishName if locale == "enUS" => continue,
                NameLine::EnglishName => &mut name.english_name,
                NameLine::MaxHint => &mut name.max_hint,
            };
            if slot.is_some() {
                return Err(io::Error::other("repeated optional item name role"));
            }
            let (line, remaining) = name.localized.split_once('\n').ok_or_else(|| {
                io::Error::other("missing localized item name after optional line")
            })?;
            *slot = Some(line);
            name.localized = remaining;
        }
        Ok(name)
    }

    fn render(&self, options: LabelOptions) -> String {
        let mut text = String::new();
        for role in self.roles {
            let line = match role {
                NameLine::EnglishName if options.english_names => self.english_name,
                NameLine::MaxHint if options.max_hints => self.max_hint,
                _ => None,
            };
            if let Some(line) = line {
                text.push_str(line);
                text.push('\n');
            }
        }
        text.push_str(self.localized);
        text
    }
}

fn render_rune(
    original: &str,
    rule: &RuneLabel,
    rune: &Rune,
    locale: &str,
    options: LabelOptions,
) -> io::Result<String> {
    let (recipe, name) = if rule.recipe_line {
        let (recipe, name) = original
            .split_once('\n')
            .ok_or_else(|| io::Error::other("missing rune recipe line"))?;
        (Some(recipe), name)
    } else {
        (None, original)
    };
    let number = format!(" #{}", rule.number);
    let mut name = name
        .strip_suffix(&number)
        .ok_or_else(|| io::Error::other("missing rune number"))?;
    if locale != "enUS" && !options.english_names {
        name = name
            .strip_suffix(&format!(" {}", rune.english))
            .ok_or_else(|| io::Error::other("missing bilingual rune name"))?;
    }

    let mut label = String::new();
    if let Some(recipe) = recipe.filter(|_| options.rune_recipes) {
        label.push_str(recipe);
        label.push('\n');
    }
    label.push_str(name);
    if options.rune_numbers {
        label.push_str(&number);
    }
    Ok(label)
}
