//! Check a shared suffix group against the regional item names that use it.
use std::{collections::BTreeSet, io};

use serde_json::Value;

use crate::item_catalog::{native_locales, BaseGroup};

pub(super) struct BaseLocaleCoverage<'a> {
    group: &'a BaseGroup,
    unused: BTreeSet<&'a str>,
    example: Option<(&'a str, u64, &'a str)>,
}

impl<'a> BaseLocaleCoverage<'a> {
    pub(super) fn new(group: &'a BaseGroup) -> Self {
        Self {
            group,
            unused: group.suffixes.keys().map(String::as_str).collect(),
            example: None,
        }
    }

    pub(super) fn record_row(
        &mut self,
        item: (&'a str, u64, &'a str),
        row: &Value,
    ) -> io::Result<()> {
        self.example.get_or_insert(item);
        let mut locales = native_locales(row).peekable();
        if locales.peek().is_none() {
            return Err(io::Error::other("expected a localized base name"));
        }
        for locale in locales {
            self.unused.remove(locale);
        }
        Ok(())
    }

    pub(super) fn check_group(&self, index: usize) -> io::Result<()> {
        let example = self
            .example
            .map(|(bank, id, key)| format!(" ({bank}/{id}/{key})"))
            .unwrap_or_default();
        let error = |locale: &str, reason: &str| {
            io::Error::other(format!(
                "items.json: base suffix group {index}{example}/{locale}: {reason}"
            ))
        };
        if let Some(locale) = self.unused.first() {
            return Err(error(
                locale,
                "no item using this group has this translation in either region",
            ));
        }
        for (locale, suffixes) in &self.group.suffixes {
            if suffixes.full.trim().is_empty() || suffixes.tier.trim().is_empty() {
                return Err(error(locale, "empty full or tier suffix"));
            }
            if !suffixes.weight.is_empty() && suffixes.weight.trim().is_empty() {
                return Err(error(locale, "blank weight suffix"));
            }
            if suffixes.weight.is_empty() && suffixes.full != suffixes.tier {
                return Err(error(
                    locale,
                    "a base without weight must keep its tier suffix when weight is disabled",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_group_covers_the_union_of_the_languages_on_its_items() -> io::Result<()> {
        let item = ("item-names", 20366, "uit");
        let group = serde_json::from_value(json!({"suffixes": {
            "deDE": {"full":" [X]|[L]", "tier":" [X]", "weight":" [L]"},
            "nlNL": {"full":" <X>", "tier":" <X>", "weight":""}
        }}))?;
        let mut coverage = BaseLocaleCoverage::new(&group);
        let german = json!({"id":20366, "Key":"uit", "deDE":"[ms]Monarch [X]|[L]", "note":"future metadata"});
        coverage.record_row(item, &german)?;
        let error = coverage.check_group(2).unwrap_err().to_string();
        assert!(error.contains("item-names/20366/uit") && error.contains("nlNL"));
        coverage.record_row(item, &json!({"nlNL":"Monarch <X>"}))?;
        coverage.check_group(2)?;
        Ok(())
    }

    #[test]
    fn empty_marks_cannot_replace_a_tier_or_change_the_name() -> io::Result<()> {
        let item = ("item-names", 2005, "crs");
        let row = json!({"deDE":"[ns]Kristallschwert [N]"});
        for (full, tier, weight) in [
            ("", " [N]", ""),
            (" [N]", " ", ""),
            (" [N]", " [N]", " "),
            (" [N]", " [X]", ""),
        ] {
            let group = serde_json::from_value(json!({"suffixes": {
                "deDE": {"full":full, "tier":tier, "weight":weight}
            }}))?;
            let mut coverage = BaseLocaleCoverage::new(&group);
            coverage.record_row(item, &row)?;
            let error = coverage.check_group(8).unwrap_err();
            assert!(error.to_string().contains("deDE:"), "{error}");
        }
        Ok(())
    }
}
