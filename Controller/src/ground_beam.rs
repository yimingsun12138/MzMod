//! Attach or remove the catalog's drop-beam entity and the particle dependencies it owns.
use serde_json::{json, Value};
use std::io;

pub(crate) struct GroundBeam<'a> {
    entity: &'a Value,
    id: u64,
    name: &'a str,
    particles: Vec<&'a str>,
}

impl<'a> GroundBeam<'a> {
    pub(crate) fn from_entity(entity: &'a Value) -> io::Result<Self> {
        if entity["type"] != "Entity" {
            return Err(io::Error::other("drop beam must be a game Entity"));
        }
        let id = entity["id"]
            .as_u64()
            .ok_or_else(|| io::Error::other("beam entity id must be an unsigned integer"))?;
        let name = entity["name"]
            .as_str()
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| io::Error::other("beam entity name must be a nonempty string"))?;
        let components = entity["components"]
            .as_array()
            .ok_or_else(|| io::Error::other("beam entity components must be an array"))?;
        let mut particles = Vec::new();
        for component in components
            .iter()
            .filter(|c| c["type"] == "VfxDefinitionComponent")
        {
            let path = component["filename"]
                .as_str()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| {
                    io::Error::other("beam VfxDefinitionComponent needs a particle filename")
                })?;
            if !particles.contains(&path) {
                particles.push(path);
            }
        }
        if particles.is_empty() {
            return Err(io::Error::other(
                "beam entity needs a VfxDefinitionComponent",
            ));
        }
        Ok(Self {
            entity,
            id,
            name,
            particles,
        })
    }

    fn identifies(&self, entity: &Value) -> bool {
        entity["id"].as_u64() == Some(self.id) || entity["name"].as_str() == Some(self.name)
    }

    /// An installed beam must declare each dependency. An unlit model can already share it.
    fn dependencies<'m>(
        &self,
        model: &'m mut Value,
        beam_installed: bool,
    ) -> io::Result<&'m mut Vec<Value>> {
        let dependencies = model
            .pointer_mut("/dependencies/particles")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                io::Error::other("ground model dependencies.particles must be an array")
            })?;
        for &path in &self.particles {
            let mut matches = dependencies.iter().filter(|row| row["path"] == path);
            let present = matches.next().is_some();
            if beam_installed && !present {
                return Err(io::Error::other(format!(
                    "missing beam particle dependency {path:?}"
                )));
            }
            if matches.next().is_some() {
                return Err(io::Error::other(format!(
                    "duplicate beam particle dependency {path:?}"
                )));
            }
        }
        Ok(dependencies)
    }

    pub(crate) fn attach(&self, model: &mut Value) -> io::Result<()> {
        if entities(model)?
            .iter()
            .any(|entity| self.identifies(entity))
        {
            return Err(io::Error::other(format!(
                "beam entity id {} or name {:?} already in use",
                self.id, self.name
            )));
        }
        let dependencies = self.dependencies(model, false)?;
        for &path in &self.particles {
            if !dependencies.iter().any(|row| row["path"] == path) {
                dependencies.push(json!({"path": path}));
            }
        }
        entities(model)?.push(self.entity.clone());
        Ok(())
    }

    pub(crate) fn detach(&self, model: &mut Value) -> io::Result<()> {
        let current = entities(model)?;
        let mut matching = current
            .iter()
            .enumerate()
            .filter(|(_, e)| self.identifies(e));
        let Some((index, entity)) = matching.next() else {
            return Err(io::Error::other(format!(
                "missing beam entity {:?} (id {})",
                self.name, self.id
            )));
        };
        if matching.next().is_some() || entity != self.entity {
            return Err(io::Error::other(format!(
                "expected one complete catalog beam entity {:?} (id {})",
                self.name, self.id
            )));
        }

        let mut unused = self.particles.clone();
        for (other_index, other) in current.iter().enumerate() {
            if other_index != index {
                if let Some(components) = other["components"].as_array() {
                    unused.retain(|path| !components.iter().any(|c| c["filename"] == *path));
                }
            }
        }
        self.dependencies(model, true)?.retain(|row| {
            !row["path"]
                .as_str()
                .is_some_and(|path| unused.contains(&path))
        });
        entities(model)?.remove(index);
        Ok(())
    }
}

fn entities(model: &mut Value) -> io::Result<&mut Vec<Value>> {
    model
        .get_mut("entities")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| io::Error::other("ground model entities must be an array"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(path: &str) -> Value {
        json!({"type":"VfxDefinitionComponent", "filename":path})
    }

    fn beam_entity() -> Value {
        json!({"type":"Entity", "id":2, "name":"drop", "components":[effect("shared"), effect("private"), effect("shared")]})
    }

    fn unlit_model() -> Value {
        json!({
            "entities":[{"id":1, "name":"appearance", "components":[effect("shared")]}],
            "dependencies":{"particles":[{"path":"other"}, {"path":"shared", "variant":4}, {"path":"other"}]},
            "model_metadata":{"value":9}
        })
    }

    #[test]
    fn shared_dependencies_keep_their_metadata_and_owned_effects_are_reversible() -> io::Result<()>
    {
        let entity = beam_entity();
        let beam = GroundBeam::from_entity(&entity)?;
        let native = unlit_model();
        let mut model = native.clone();
        beam.attach(&mut model)?;
        let mut expected = native.clone();
        expected["entities"]
            .as_array_mut()
            .unwrap()
            .push(entity.clone());
        expected["dependencies"]["particles"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":"private"}));
        assert_eq!(model, expected);
        beam.detach(&mut model)?;
        assert_eq!(model, native);
        Ok(())
    }

    #[test]
    fn conflicting_entities_and_dependencies_leave_the_model_untouched() -> io::Result<()> {
        let entity = beam_entity();
        let beam = GroundBeam::from_entity(&entity)?;
        for collision in [json!({"id":2}), json!({"name":"drop"}), entity.clone()] {
            let mut native = unlit_model();
            native["entities"].as_array_mut().unwrap().push(collision);
            let mut model = native.clone();
            assert!(beam.attach(&mut model).is_err());
            assert_eq!(model, native);
        }

        let mut lit = unlit_model();
        beam.attach(&mut lit)?;
        let mut changed = lit.clone();
        changed["entities"][1]["components"][0]["variant"] = 3.into();
        let mut duplicate_id = lit.clone();
        duplicate_id["entities"][0]["id"] = 2.into();
        let mut missing = lit.clone();
        missing["dependencies"]["particles"]
            .as_array_mut()
            .unwrap()
            .pop();
        let mut ambiguous = lit;
        ambiguous["dependencies"]["particles"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":"shared", "variant":5}));
        for invalid in [unlit_model(), changed, duplicate_id, missing, ambiguous] {
            let mut model = invalid.clone();
            assert!(beam.detach(&mut model).is_err());
            assert_eq!(model, invalid);
        }
        Ok(())
    }

    #[test]
    fn a_beam_needs_an_identity_and_a_filename_for_each_visual_effect() {
        for (field, invalid) in [
            ("type", json!("Component")),
            ("id", json!(-2)),
            ("name", json!(" ")),
            ("components", json!([])),
            ("components", json!([effect("valid"), effect(" ")])),
        ] {
            let mut entity = beam_entity();
            entity[field] = invalid;
            assert!(GroundBeam::from_entity(&entity).is_err(), "{field}");
        }
    }
}
