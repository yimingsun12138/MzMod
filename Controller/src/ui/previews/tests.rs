use super::*;

#[test]
fn preview_rows_match_the_selected_regional_template() -> anyhow::Result<()> {
    for region in Region::ALL {
        let files = crate::template::RegionalTemplate::read(region)?.files;
        let selected = &preview(region).items;
        let samples = [&selected.runeword, &selected.shield, &selected.sword];
        for sample in samples
            .into_iter()
            .chain(selected.runes.iter().map(|rune| &rune.label))
        {
            let path = format!("data/local/lng/strings/{}.json", sample.bank);
            let rows: Vec<serde_json::Value> = serde_json::from_slice(&files[&path])?;
            let row = rows
                .iter()
                .find(|row| row["id"] == sample.row["id"] && row["Key"] == sample.row["Key"]);
            assert_eq!(row, Some(&sample.row), "{} {path}", region.code());
        }
    }
    Ok(())
}

struct Preview {
    lines: Vec<(String, Rect, Color32)>,
    icons: Vec<(Rect, String)>,
    dividers: Vec<[egui::Pos2; 2]>,
}

fn pixel_fingerprint(image: &egui::ColorImage) -> String {
    crate::file_fingerprints::checksum(
        &image
            .pixels
            .iter()
            .flat_map(|pixel| pixel.to_array())
            .collect::<Vec<_>>(),
    )
}

impl Preview {
    fn draw(page: fn(&mut Ui, Region, &Settings), settings: &Settings) -> Self {
        Self::draw_in(page, Region::Int, settings)
    }

    fn draw_in(page: fn(&mut Ui, Region, &Settings), region: Region, settings: &Settings) -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(518., 640.))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page(ui, region, settings));
            },
        );
        let mut lines = Vec::new();
        let mut icons = Vec::new();
        let mut dividers = Vec::new();
        for clipped in output.shapes {
            match clipped.shape {
                egui::Shape::Text(text) => {
                    let bounds = Rect::from_min_size(text.pos, text.galley.size());
                    for section in &text.galley.job.sections {
                        assert!(
                            section.format.font_id.size <= OPTION_TITLE_SIZE,
                            "oversized {}",
                            text.galley.text()
                        );
                    }
                    assert!(
                        clipped.clip_rect.expand(1.).contains_rect(bounds),
                        "clipped {}: {bounds:?} in {:?}",
                        text.galley.text(),
                        clipped.clip_rect
                    );
                    lines.push((
                        text.galley.text().into(),
                        bounds,
                        text.galley.job.sections[0].format.color,
                    ));
                }
                egui::Shape::LineSegment { points, stroke } if stroke.color == LINE => {
                    dividers.push(points);
                }
                egui::Shape::Mesh(mesh) => {
                    let (_, delta) = output
                        .textures_delta
                        .set
                        .iter()
                        .find(|(id, _)| *id == mesh.texture_id)
                        .unwrap();
                    let egui::ImageData::Color(image) = &delta.image else {
                        panic!("expected an inventory image")
                    };
                    icons.push((mesh.calc_bounds(), pixel_fingerprint(image)));
                }
                _ => {}
            }
        }
        Self {
            lines,
            icons,
            dividers,
        }
    }

    fn first(&self, label: &str) -> egui::Pos2 {
        self.lines
            .iter()
            .find(|(s, _, _)| s == label)
            .unwrap()
            .1
            .center()
    }

    fn has(&self, label: &str) -> bool {
        self.lines.iter().any(|(s, _, _)| s == label)
    }
}

#[test]
fn preview_colors_match_the_installed_tooltip_in_each_region() -> anyhow::Result<()> {
    for region in Region::ALL {
        for item_colors in [true, false] {
            let settings = Settings {
                item_colors,
                ..crate::enabled_features()
            };
            let files = crate::render_mod(region, &settings)?;
            let profile: serde_json::Value =
                serde_json::from_slice(&files["data/global/ui/layouts/_profilehd.json"])?;
            let resolved = |binding: &serde_json::Value| {
                let mut value = binding;
                while let Some(reference) = value.as_str() {
                    value = &profile[reference.strip_prefix('$').unwrap()];
                }
                let [r, g, b, a] = ["r", "g", "b", "a"]
                    .map(|key| value[key].as_u64().unwrap().try_into().unwrap());
                Color32::from_rgba_unmultiplied(r, g, b, a)
            };
            let expected = ["EtherealColor", "SocketedColor"]
                .map(|field| resolved(&profile["TooltipStyle"][field]));
            let preview = Preview::draw_in(names, region, &settings);
            for sword in ["水晶劍|普", "Crystal Sword|NOR"] {
                let colors: Vec<_> = preview
                    .lines
                    .iter()
                    .filter_map(|(text, _, color)| (text == sword).then_some(*color))
                    .collect();
                assert_eq!(colors, expected, "{} {sword} {item_colors}", region.code());
            }
            let rune_preview = Preview::draw_in(runes, region, &settings);
            for (drawn, name, color) in [
                (
                    &preview,
                    "君主盾|精輕",
                    &profile["TooltipStyle"]["DefaultColor"],
                ),
                (&preview, "Spirit", &profile["FontColorGoldYellow"]),
                (&rune_preview, "Ort #9", &profile["FontColorOrange"]),
                (
                    &rune_preview,
                    "[x2+Flawless Sapphire x1]",
                    &profile["FontColorGray"],
                ),
            ] {
                let colors: Vec<_> = drawn
                    .lines
                    .iter()
                    .filter_map(|(label, _, color)| (label == name).then_some(*color))
                    .collect();
                assert!(!colors.is_empty(), "{} {name}", region.code());
                assert!(
                    colors.iter().all(|&c| c == resolved(color)),
                    "{} {name}",
                    region.code()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn inline_colors_share_one_centered_line_and_keep_unrecognized_markers() {
    let base = [240, 240, 240, 255];
    let red = [252, 70, 70, 255];
    let blue = [110, 110, 255, 255];
    let palette = BTreeMap::from([('1', red), ('3', blue)]);
    let cell = Rect::from_min_size(pos2(10., 10.), vec2(124., 72.));
    let label = "ÿc3[1234567890ÿc1ABCDEFGH]\nÉpéeÿc3ÿc1Redÿc?ÿc3Blueÿc";
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let output = ctx.run(Default::default(), |ctx| {
        tooltip(
            &ctx.layer_painter(egui::LayerId::background()),
            cell,
            cell.top() + CELL_PADDING,
            label,
            base,
            &palette,
        );
    });
    let lines: Vec<_> = output
        .shapes
        .into_iter()
        .filter_map(|shape| match shape.shape {
            egui::Shape::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 2);
    let expected_sections = [
        vec![("Épée", base), ("Redÿc?", red), ("Blueÿc", blue)],
        vec![("[1234567890", blue), ("ABCDEFGH]", red)],
    ];
    for (line, expected) in lines.iter().zip(expected_sections) {
        let job = &line.galley.job;
        let sections: Vec<_> = job
            .sections
            .iter()
            .map(|section| (&job.text[section.byte_range.clone()], section.format.color))
            .collect();
        assert_eq!(
            sections,
            expected
                .into_iter()
                .map(|(text, color)| (text, game_color(color)))
                .collect::<Vec<_>>()
        );
        assert_eq!(line.galley.rows.len(), 1);
        let bounds = Rect::from_min_size(line.pos, line.galley.size());
        assert!((bounds.center().x - cell.center().x).abs() < 1.);
        assert!(cell.contains_rect(bounds));
    }
    assert!(lines[0].pos.y < lines[1].pos.y);
    let hint = &lines[1].galley.job.sections;
    assert!(hint[0].format.font_id.size < 19. * TEXT_SCALE);
    assert_eq!(hint[0].format.font_id.size, hint[1].format.font_id.size);
}

#[test]
fn name_options_change_only_their_examples_and_keep_each_slot_in_place() {
    let defaults = crate::enabled_features();
    let all = Preview::draw(names, &defaults);
    let previews = [
        (
            "中文界面预览",
            ["符文之语", "精英轻盾", "无形无孔", "有孔"],
            ["精神", "君主盾|精輕", "水晶劍|普", "水晶劍|普"],
        ),
        (
            "英文界面预览",
            [
                "Runeword",
                "Elite light shield",
                "Ethereal, no sockets",
                "Socketed",
            ],
            [
                "Spirit",
                "Monarch|ELI [L]",
                "Crystal Sword|NOR",
                "Crystal Sword|NOR",
            ],
        ),
    ];
    assert_eq!(all.dividers.len(), 4);
    assert!(all.first("中文界面预览").y < all.first("英文界面预览").y);
    for ((title, captions, names), dividers) in previews.into_iter().zip(all.dividers.chunks(2)) {
        let (vertical, horizontal) = (dividers[0], dividers[1]);
        let center = vertical[0].lerp(vertical[1], 0.5);
        assert_eq!(center, horizontal[0].lerp(horizontal[1], 0.5));
        assert!((all.first(title).x - center.x).abs() < 1.);
        assert!(all.first(title).y < vertical[0].y);
        let canvas = Rect::from_min_max(
            pos2(horizontal[0].x, vertical[0].y),
            pos2(horizontal[1].x, vertical[1].y),
        );
        let (left, right) = canvas.split_left_right_at_x(center.x);
        let (spirit, ethereal_sword) = left.split_top_bottom_at_y(center.y);
        let (shield, socketed_sword) = right.split_top_bottom_at_y(center.y);
        for ((cell, caption), name) in [spirit, shield, ethereal_sword, socketed_sword]
            .into_iter()
            .zip(captions)
            .zip(names)
        {
            let mut group = Rect::NOTHING;
            for (_, rect, _) in all
                .lines
                .iter()
                .filter(|(_, rect, _)| cell.contains(rect.center()))
            {
                assert!((rect.center().x - cell.center().x).abs() < 1.);
                group = group.union(*rect);
            }
            let bounds = |value: &str| {
                all.lines
                    .iter()
                    .find(|(label, rect, _)| label == value && cell.contains(rect.center()))
                    .unwrap()
                    .1
            };
            let gap = bounds(name).top() - bounds(caption).bottom();
            assert!(
                (2.0..=9.0).contains(&gap),
                "{caption} is {gap} points from its name"
            );
            assert!(
                cell.shrink(12.).contains_rect(group),
                "{caption}: {group:?} in {cell:?}"
            );
            assert!((cell.center().y - group.center().y).abs() < 3.);
        }
    }
    assert_eq!(
        all.lines.iter().filter(|(s, _, _)| s == "Spirit").count(),
        2
    );
    let without_english = Preview::draw(
        names,
        &Settings {
            english_names: false,
            ..defaults.clone()
        },
    );
    let english_heading = all.first("英文界面预览").y;
    let english_lines = |p: &Preview| {
        p.lines
            .iter()
            .filter(|(_, rect, _)| rect.center().y >= english_heading)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(english_lines(&without_english), english_lines(&all));
    assert_eq!(
        without_english
            .lines
            .iter()
            .filter(|(s, _, _)| s == "Spirit")
            .count(),
        1
    );
    assert!(without_english.first("Spirit").y > english_heading);
    for (settings, removed, addition) in [
        (
            Settings {
                max_hints: false,
                ..defaults.clone()
            },
            "[MAX:35FCR/112Mana/8MA]",
            None,
        ),
        (
            Settings {
                base_tiers: false,
                ..defaults.clone()
            },
            "君主盾|精輕",
            Some(("君主盾|輕", "Monarch [L]")),
        ),
        (
            Settings {
                armor_weights: false,
                ..defaults.clone()
            },
            "君主盾|精輕",
            Some(("君主盾|精", "Monarch|ELI")),
        ),
    ] {
        let changed = Preview::draw(names, &settings);
        assert!(!changed.has(removed));
        if let Some((chinese, english)) = addition {
            assert!(changed.first(chinese).distance(all.first("君主盾|精輕")) < 1.);
            assert!(
                changed
                    .first(english)
                    .distance(all.first("Monarch|ELI [L]"))
                    < 1.
            );
        }
        for fixed in [
            "符文之语",
            "精神",
            "精英轻盾",
            "无形无孔",
            "有孔",
            "Runeword",
            "Elite light shield",
            "Socketed",
        ] {
            assert_eq!(all.first(fixed), changed.first(fixed));
        }
    }
    let colors = |p: &Preview, name: &str| {
        p.lines
            .iter()
            .filter_map(|(s, _, c)| (s == name).then_some(*c))
            .collect::<Vec<_>>()
    };
    let plain = Preview::draw(
        names,
        &Settings {
            item_colors: false,
            ..defaults
        },
    );
    for sword in ["水晶劍|普", "Crystal Sword|NOR"] {
        assert_eq!(
            colors(&all, sword),
            [
                Color32::from_rgb(128, 255, 255),
                Color32::from_rgb(255, 170, 255)
            ]
        );
        assert_eq!(colors(&plain, sword), [Color32::from_gray(99); 2]);
    }
}

#[test]
fn ort_and_ber_preview_name_icon_and_material_choices_independently() {
    let mut settings = crate::enabled_features();
    let full = Preview::draw(runes, &settings);
    let ort = full.first("歐特 Ort #9");
    let ber = full.first("貝 Ber #30");
    let recipes = ["[x2+無瑕藍寶石x1]", "[x2+Flawless Sapphire x1]"];
    let english_ort = full.first("Ort #9");
    let english_ber = full.first("Ber #30");
    assert!(ber.y < full.first("英文界面预览").y);
    assert_eq!(full.icons.len(), 4);
    assert_eq!(full.dividers.len(), 2);
    for (((heading, [left, right]), icons), divider) in [
        ("中文界面预览", [ort, ber]),
        ("英文界面预览", [english_ort, english_ber]),
    ]
    .into_iter()
    .zip(full.icons.chunks(2))
    .zip(&full.dividers)
    {
        let heading = full.first(heading);
        assert!((heading.x - divider[0].x).abs() < 1.);
        assert!(heading.y < divider[0].y && divider[1].y > left.y);
        assert!(left.x < divider[0].x && divider[0].x < right.x);
        assert!((left.y - right.y).abs() < 1.);
        assert!(((left.x + right.x) / 2. - heading.x).abs() < 1.);
        for (name, (icon, _)) in [left, right].into_iter().zip(icons) {
            assert!((name.x - icon.center().x).abs() < 1.);
            assert!(icon.top() > heading.y && icon.bottom() < name.y);
        }
    }
    for (recipe, name) in recipes.iter().zip([ber, english_ber]) {
        let line = full.first(recipe);
        assert!(line.y > name.y && (line.x - name.x).abs() < 1.);
    }
    assert_eq!(
        full.lines
            .iter()
            .filter(|(s, _, _)| s.starts_with("[x"))
            .count(),
        2
    );

    settings.english_names = false;
    let chinese_only = Preview::draw(runes, &settings);
    assert!(chinese_only.first("歐特 #9").distance(ort) < 1.);
    assert!(chinese_only.first("貝 #30").distance(ber) < 1.);
    assert_eq!(chinese_only.first("Ort #9"), english_ort);
    assert_eq!(chinese_only.first("Ber #30"), english_ber);
    assert_eq!(chinese_only.first(recipes[1]), full.first(recipes[1]));

    settings.rune_numbers = false;
    let plain = Preview::draw(runes, &settings);
    assert!(plain.first("歐特").distance(ort) < 1.);
    assert!(plain.first("貝").distance(ber) < 1.);
    assert!(plain.first("Ort").distance(english_ort) < 1.);
    assert!(plain.first("Ber").distance(english_ber) < 1.);
    assert_eq!(plain.icons, full.icons);
    for recipe in recipes {
        assert_eq!(plain.first(recipe), full.first(recipe));
    }
    settings.rune_icons = false;
    let native = Preview::draw(runes, &settings);
    assert_eq!(native.lines, plain.lines);
    assert_eq!(native.icons.len(), 4);
    for ((rect, pixels), (numbered_rect, numbered_pixels)) in native.icons.iter().zip(full.icons) {
        assert_eq!(rect.size(), vec2(36., 36.));
        assert_eq!(*rect, numbered_rect);
        assert_ne!(*pixels, numbered_pixels);
    }
    settings.rune_recipes = false;
    let minimal = Preview::draw(runes, &settings);
    assert!(minimal.first("貝").distance(ber) < 1.);
    assert!(minimal.first("Ber").distance(english_ber) < 1.);
    assert!(recipes.iter().all(|line| !minimal.has(line)));
    assert_eq!(minimal.icons, native.icons);
}

#[test]
fn both_preview_languages_draw_the_selected_regional_inventory_pixels() -> anyhow::Result<()> {
    use crate::item_catalog::{inventory_icon_path, inventory_sprite_name, ItemCatalog};
    use std::{fs, path::Path};

    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut source_ranges = BTreeMap::new();
    let catalog: ItemCatalog = serde_json::from_slice(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/item-catalog.json"
    )))?;
    for region in Region::ALL {
        let regional = preview(region);
        let files = crate::template::RegionalTemplate::read(region)?.files;
        for numbered in [false, true] {
            let settings = Settings {
                rune_icons: numbered,
                ..crate::enabled_features()
            };
            let drawn = Preview::draw_in(runes, region, &settings);
            assert_eq!(
                drawn.icons.len(),
                regional.items.runes.len() * PreviewLanguage::ALL.len()
            );
            for (column, example) in regional.items.runes.iter().enumerate() {
                let item = &example.label;
                let label = catalog
                    .label(
                        &item.bank,
                        item.row["id"].as_u64().unwrap(),
                        item.row["Key"].as_str().unwrap(),
                    )
                    .unwrap();
                let (_, rune) = label.resolve_rune_label(&catalog)?.unwrap();
                let sprite = inventory_sprite_name(&rune.route);
                let variants = &example.icons;
                let (root, path, range) = if numbered {
                    (
                        "../ModTemplate",
                        inventory_icon_path(&sprite),
                        &variants.numbered,
                    )
                } else {
                    ("assets/ui/native-rune-icons", sprite, &variants.native)
                };
                let root = crate_root.join(root);
                let common = root.join("common").join(&path);
                let physical_path = if common.exists() {
                    common
                } else {
                    root.join(region.code()).join(&path)
                };
                let source = fs::read(&physical_path)?;
                if let Some(previous) = source_ranges.insert(physical_path, range) {
                    assert_eq!(previous, range, "shared sprite {path}");
                }
                if numbered {
                    assert_eq!(files[&path], source, "{} {path}", region.code());
                }
                let png = eframe::icon_data::from_png_bytes(&RUNE_ICON_BYTES[range.clone()])?;
                let size = [8, 12].map(|offset| {
                    u32::from_le_bytes(source[offset..offset + 4].try_into().unwrap())
                });
                assert_eq!([png.width, png.height], size);
                assert_eq!(png.rgba, &source[40..], "{} {path}", region.code());
                let expected = pixel_fingerprint(&egui::ColorImage::from_rgba_unmultiplied(
                    size.map(|side| side as usize),
                    &source[40..],
                ));
                for icons in drawn.icons.chunks_exact(regional.items.runes.len()) {
                    assert_eq!(icons[column].1, expected, "{} {path}", region.code());
                }
            }
        }
    }
    Ok(())
}
