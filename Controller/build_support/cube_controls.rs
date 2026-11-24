//! Native widgets used to open, operate, and close the mini cube.
use serde_json::{json, Value};
use std::io;

pub(super) const OPEN_MINI_CUBE: &str = "PanelManager:OpenPanel:HoradricCubeMiniLayout";
pub(super) const TOGGLE_MINI_CUBE: &str = "PanelManager:TogglePanel:HoradricCubeMiniLayout";

pub(super) fn widgets(root: &Value) -> io::Result<Vec<&Value>> {
    let mut pending = vec![root];
    let mut nodes = Vec::new();
    while let Some(widget) = pending.pop() {
        let object = widget
            .as_object()
            .ok_or_else(|| io::Error::other("expected a widget object"))?;
        nodes.push(widget);
        if let Some(children) = object.get("children") {
            pending.extend(
                children
                    .as_array()
                    .ok_or_else(|| io::Error::other("expected a widget children array"))?,
            );
        }
    }
    Ok(nodes)
}

pub(super) fn check_manual_entry(nodes: &[&Value], action: Option<&str>) -> io::Result<()> {
    let mut entries = nodes.iter().filter(|widget| {
        widget["fields"]["onClickMessage"]
            .as_str()
            .and_then(|message| message.strip_prefix("PanelManager:"))
            .and_then(|command| command.split_once(':'))
            .is_some_and(|(_, panel)| panel == "HoradricCubeMiniLayout")
    });
    let first = entries.next();
    let Some(action) = action else {
        return if first.is_none() {
            Ok(())
        } else {
            Err(io::Error::other(
                "unexpected cube button in the desktop HUD",
            ))
        };
    };
    if entries.next().is_none()
        && first.is_some_and(|widget| {
            widget["type"] == "ButtonWidget"
                && widget["fields"]["onClickMessage"] == action
                && widget["fields"]["tooltipString"] == "@Mzz2blib"
        })
    {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "expected one ButtonWidget with {action} and the mini cube tooltip"
        )))
    }
}

pub(crate) fn check_inventory(source: &[u8]) -> io::Result<()> {
    let root: Value = serde_json::from_slice(source)?;
    if root["type"] != "PlayerInventoryPanel" || root["name"] != "PlayerInventoryExpansionLayout" {
        return Err(io::Error::other(
            "expected PlayerInventoryPanel PlayerInventoryExpansionLayout",
        ));
    }
    check_manual_entry(&widgets(&root)?, Some(OPEN_MINI_CUBE))
}

pub(crate) fn check_panel(source: &[u8]) -> io::Result<()> {
    let root: Value = serde_json::from_slice(source)?;
    if root["type"] != "HoradricCubePanel" || root["name"] != "HoradricCubeMiniLayout" {
        return Err(io::Error::other(
            "expected HoradricCubePanel HoradricCubeMiniLayout",
        ));
    }
    let nodes = widgets(&root)?;
    let control = |name: &str| -> io::Result<&Value> {
        let mut matches = nodes
            .iter()
            .copied()
            .filter(|widget| widget["name"] == name);
        match (matches.next(), matches.next()) {
            (Some(widget), None) => Ok(widget),
            _ => Err(io::Error::other(format!(
                "expected one mini cube control named {name}"
            ))),
        }
    };
    let grid = control("grid")?;
    if grid["type"] != "InventoryGridWidget"
        || grid["fields"]["cellCount"] != json!({"x": 3, "y": 4})
    {
        return Err(io::Error::other(
            "expected a 3 by 4 mini cube inventory grid",
        ));
    }
    for (name, action) in [
        ("convert", "HoradricCubePanelMessage:Convert"),
        ("close", "PanelManager:ClosePanel:HoradricCubeMiniLayout"),
    ] {
        let button = control(name)?;
        if button["type"] != "ButtonWidget" || button["fields"]["onClickMessage"] != action {
            return Err(io::Error::other(format!(
                "expected {name} ButtonWidget with {action}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_entry_uses_the_input_specific_click_action() -> io::Result<()> {
        let mut inventory = json!({
            "type": "PlayerInventoryPanel", "name": "PlayerInventoryExpansionLayout",
            "children": [{"children": [
                {"type": "ButtonWidget", "name": "RegionalButton", "fields": {
                    "onClickMessage": OPEN_MINI_CUBE, "tooltipString": "@Mzz2blib"
                }},
                {"fields": {"onClickMessage": "PanelManager:OpenPanel:HoradricCubeMiniLayoutHelp"}}
            ]}]
        });
        check_inventory(&serde_json::to_vec(&inventory)?)?;
        let entry = &mut inventory["children"][0]["children"][0];
        entry["fields"]["onClickMessage"] = TOGGLE_MINI_CUBE.into();
        check_manual_entry(&widgets(entry)?, Some(TOGGLE_MINI_CUBE))?;
        assert!(check_manual_entry(&widgets(entry)?, None).is_err());
        assert!(check_manual_entry(&widgets(entry)?, Some(OPEN_MINI_CUBE)).is_err());
        entry["type"] = "ImageWidget".into();
        assert!(check_manual_entry(&widgets(entry)?, Some(TOGGLE_MINI_CUBE)).is_err());
        Ok(())
    }

    #[test]
    fn cube_grid_and_buttons_must_identify_one_operable_control_each() -> io::Result<()> {
        let source = json!({
            "type": "HoradricCubePanel", "name": "HoradricCubeMiniLayout", "children": [
                {"name":"grid", "type":"InventoryGridWidget", "fields":{"cellCount":{"x":3,"y":4}}},
                {"name":"convert", "type":"ButtonWidget", "fields":{"onClickMessage":"HoradricCubePanelMessage:Convert"}},
                {"name":"close", "type":"ButtonWidget", "fields":{"onClickMessage":"PanelManager:ClosePanel:HoradricCubeMiniLayout"}}
            ]
        });
        check_panel(&serde_json::to_vec(&source)?)?;
        for name in ["grid", "convert", "close"] {
            let mut panel = source.clone();
            let children = panel["children"].as_array_mut().unwrap();
            let index = children
                .iter()
                .position(|widget| widget["name"] == name)
                .unwrap();
            children.push(children[index].clone());
            assert!(
                check_panel(&serde_json::to_vec(&panel)?).is_err(),
                "ambiguous {name}"
            );
            panel["children"].as_array_mut().unwrap().pop();
            panel["children"][index]["type"] = "ImageWidget".into();
            assert!(
                check_panel(&serde_json::to_vec(&panel)?).is_err(),
                "inactive {name}"
            );
        }
        Ok(())
    }
}
