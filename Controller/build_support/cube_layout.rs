//! Bind the one-time cube startup to the native HUD and derive the running layout.
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, io};

#[path = "cube_controls.rs"]
pub(crate) mod controls;
use controls::{check_manual_entry, widgets, OPEN_MINI_CUBE, TOGGLE_MINI_CUBE};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CubeHud {
    pub(crate) entry_panel: String,
    pub(crate) gameplay_panel: String,
    pub(crate) startup_panel: String,
}

impl CubeHud {
    pub(crate) fn from_slice(bytes: &[u8]) -> io::Result<Self> {
        let panels: Self = serde_json::from_slice(bytes)?;
        let names = [
            &panels.entry_panel,
            &panels.gameplay_panel,
            &panels.startup_panel,
        ];
        for (index, name) in names.iter().enumerate() {
            if !name.starts_with(|ch: char| ch.is_ascii_alphabetic())
                || !name
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
                || names[..index]
                    .iter()
                    .any(|other| name.eq_ignore_ascii_case(other))
            {
                return Err(io::Error::other(format!(
                    "expected distinct panel identifiers, found {name:?}"
                )));
            }
        }
        Ok(panels)
    }

    pub(crate) fn check_startup(&self, source: &[u8]) -> io::Result<()> {
        let root: Value = serde_json::from_slice(source)?;
        if root["type"] != "Panel" || root["name"] != self.startup_panel {
            return Err(io::Error::other(format!(
                "expected startup Panel {}",
                self.startup_panel
            )));
        }
        let sequence = [
            format!("PanelManager:ClosePanel:{}", self.entry_panel),
            format!("PanelManager:OpenPanel:{}", self.gameplay_panel),
            OPEN_MINI_CUBE.into(),
            "PlayerInventoryPanelMessage:Close".into(),
            format!("PanelManager:UnloadPanel:{}", self.startup_panel),
        ];
        let mut delays = BTreeMap::new();
        for timer in widgets(&root)?
            .into_iter()
            .filter(|widget| widget["type"] == "TimerWidget")
        {
            let message = timer["fields"]["message"]
                .as_str()
                .ok_or_else(|| io::Error::other("expected a startup timer message"))?;
            if !sequence.iter().any(|action| action == message) {
                return Err(io::Error::other(format!(
                    "unexpected startup timer action {message}"
                )));
            }
            if delays
                .insert(message, timer_delay(timer, message)?)
                .is_some()
            {
                return Err(io::Error::other(format!(
                    "duplicate startup timer for {message}"
                )));
            }
        }
        let mut previous = None;
        for message in &sequence {
            let delay = delays
                .get(message.as_str())
                .ok_or_else(|| io::Error::other(format!("missing startup timer for {message}")))?;
            if let Some((earlier, time)) = previous {
                if time >= delay {
                    return Err(io::Error::other(format!(
                        "startup timer {earlier} must run before {message}"
                    )));
                }
            }
            previous = Some((message, delay));
        }
        Ok(())
    }

    pub(crate) fn render_gameplay(&self, entry: &[u8], controller: bool) -> io::Result<Vec<u8>> {
        let mut root: Value = serde_json::from_slice(entry)?;
        if root["type"] != "HUDPanelHD" || root["name"] != self.entry_panel {
            return Err(io::Error::other(format!(
                "expected HUDPanelHD {}",
                self.entry_panel
            )));
        }
        let startup = format!("PanelManager:OpenPanel:{}", self.startup_panel);
        let nodes = widgets(&root)?;
        let triggers = nodes
            .iter()
            .filter(|widget| {
                widget["type"] == "TimerWidget" && widget["fields"]["message"] == startup
            })
            .count();
        if triggers != usize::from(!controller) {
            return Err(io::Error::other(format!(
                "unexpected number of {startup} timers: {triggers}"
            )));
        }
        check_manual_entry(&nodes, controller.then_some(TOGGLE_MINI_CUBE))?;
        if controller {
            let entry_base = format!("{}HD.json", self.entry_panel);
            if root["basedOn"] != entry_base {
                return Err(io::Error::other(format!("expected basedOn {entry_base}")));
            }
            root["basedOn"] = format!("{}HD.json", self.gameplay_panel).into();
        } else {
            if root.get("basedOn").is_some() {
                return Err(io::Error::other(
                    "desktop entry HUD cannot inherit another layout",
                ));
            }
            let children = root["children"]
                .as_array_mut()
                .ok_or_else(|| io::Error::other("expected HUD children"))?;
            let index = children
                .iter()
                .position(|child| {
                    child["type"] == "TimerWidget" && child["fields"]["message"] == startup
                })
                .ok_or_else(|| {
                    io::Error::other(format!("expected a direct child timer for {startup}"))
                })?;
            timer_delay(&children[index], &startup)?;
            children.remove(index);
        }
        root["name"] = self.gameplay_panel.clone().into();
        let mut bytes = serde_json::to_vec_pretty(&root)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

fn timer_delay(timer: &Value, message: &str) -> io::Result<f64> {
    timer["fields"]["time"]
        .as_f64()
        .filter(|time| time.is_finite() && *time >= 0.0)
        .ok_or_else(|| {
            io::Error::other(format!(
                "{message}: expected a finite, nonnegative numeric timer time"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PANELS: &[u8] =
        br#"{"entry_panel":"Entry","gameplay_panel":"Running","startup_panel":"Startup"}"#;

    #[test]
    fn startup_uses_timer_deadlines_independently_of_widget_order() -> io::Result<()> {
        let hud = CubeHud::from_slice(PANELS)?;
        let actions = [
            (4, "PanelManager:UnloadPanel:Startup"),
            (3, "PlayerInventoryPanelMessage:Close"),
            (2, "PanelManager:OpenPanel:HoradricCubeMiniLayout"),
            (1, "PanelManager:OpenPanel:Running"),
            (0, "PanelManager:ClosePanel:Entry"),
        ];
        let timers = actions.map(|(time, message)| {
            json!({
                "type": "TimerWidget", "fields": {"time": time, "message": message}
            })
        });
        let mut root = json!({"type":"Panel", "name":"Startup", "children":timers});
        hud.check_startup(&serde_json::to_vec(&root)?)?;
        let unload = format!("PanelManager:UnloadPanel:{}", hud.startup_panel);
        root["children"][0]["fields"]["time"] = json!(0);
        let error = hud.check_startup(&serde_json::to_vec(&root)?).unwrap_err();
        assert!(error.to_string().contains(&unload), "{error}");
        Ok(())
    }

    #[test]
    fn only_the_entry_timer_that_opens_startup_owns_its_delay() -> io::Result<()> {
        let hud = CubeHud::from_slice(PANELS)?;
        let startup = format!("PanelManager:OpenPanel:{}", hud.startup_panel);
        let mut entry = json!({
            "name": hud.entry_panel, "type": "HUDPanelHD", "children": [
                {"type": "TimerWidget", "fields": {"message": startup, "time": "0.1"}},
                {"type": "TimerWidget", "fields": {"message": "NativeAction"}}
            ]
        });
        let error = hud
            .render_gameplay(&serde_json::to_vec(&entry)?, false)
            .unwrap_err();
        assert!(error.to_string().contains(&startup), "{error}");
        entry["children"][0]["fields"]["time"] = json!(0);
        let gameplay: Value =
            serde_json::from_slice(&hud.render_gameplay(&serde_json::to_vec(&entry)?, false)?)?;
        assert_eq!(gameplay["children"], json!([entry["children"][1]]));
        Ok(())
    }
}
