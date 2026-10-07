use crate::local_store::{read_local, write_local};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
use tokio::sync::Mutex;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub device_id: String,
    pub on: bool,
    pub brightness: Option<u8>,
    pub white: Option<u64>,
}

pub fn validate_targets(targets: &[Target]) -> Result<(), String> {
    validate_targets_inner(targets, false)
}

fn validate_targets_inner(targets: &[Target], allow_legacy: bool) -> Result<(), String> {
    let mut ids = HashSet::new();
    if targets.is_empty() || targets.len() > 128 {
        return Err("Scegli da 1 a 128 lampadine.".into());
    }
    for target in targets {
        if !allow_legacy && target.white == Some(161) {
            return Err("Il vecchio preset Neutro non è supportato. Modifica la scena scegliendo Caldo o Freddo.".into());
        }
        if target.device_id.is_empty()
            || target.device_id.len() > 128
            || !target
                .device_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(&target.device_id)
            || (target.on
                && (!target.brightness.is_some_and(|b| (1..=100).contains(&b))
                    || !target
                        .white
                        .is_some_and(|w| matches!(w, 160 | 162) || (allow_legacy && w == 161))))
            || (!target.on && (target.brightness.is_some() || target.white.is_some()))
        {
            return Err(
                "Azioni della scena non valide. Scegli luminosità e bianco per le luci accese."
                    .into(),
            );
        }
    }
    Ok(())
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub targets: Vec<Target>,
}

pub struct Scenes {
    path: PathBuf,
    values: Mutex<Result<Vec<Scene>, String>>,
}

impl Scenes {
    pub fn new(path: PathBuf) -> Self {
        let values = read_local::<Scene>(&path).and_then(|scenes| {
            validate(&scenes)?;
            Ok(scenes)
        });
        Self {
            path,
            values: Mutex::new(values),
        }
    }
    pub async fn list(&self) -> Result<Vec<Scene>, String> {
        self.values.lock().await.clone()
    }
    pub async fn save(
        &self,
        id: Option<String>,
        name: String,
        targets: Vec<Target>,
    ) -> Result<Vec<Scene>, String> {
        validate_targets(&targets)?;
        let mut values = self.values.lock().await;
        let mut next = values.clone()?;
        let scene = Scene {
            id: id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: name.trim().into(),
            targets,
        };
        if let Some(id) = id {
            let index = next
                .iter()
                .position(|scene| scene.id == id)
                .ok_or("Scena non trovata. Riapri le impostazioni.")?;
            next[index] = scene;
        } else {
            next.push(scene);
        }
        validate(&next)?;
        write_local(&self.path, &next)?;
        *values = Ok(next.clone());
        Ok(next)
    }
    pub async fn delete(&self, id: &str) -> Result<Vec<Scene>, String> {
        let mut values = self.values.lock().await;
        let mut next = values.clone()?;
        let index = next
            .iter()
            .position(|scene| scene.id == id)
            .ok_or("Scena non trovata.")?;
        next.remove(index);
        write_local(&self.path, &next)?;
        *values = Ok(next.clone());
        Ok(next)
    }
}

fn validate(scenes: &[Scene]) -> Result<(), String> {
    if scenes.len() > 32 {
        return Err("Puoi creare al massimo 32 scene.".into());
    }
    let mut ids = HashSet::new();
    for scene in scenes {
        if uuid::Uuid::parse_str(&scene.id).is_err()
            || !ids.insert(&scene.id)
            || scene.name.trim().is_empty()
            || scene.name.chars().count() > 80
            || scene.name.chars().any(char::is_control)
        {
            return Err("Scegli un nome valido per la scena.".into());
        }
        validate_targets_inner(&scene.targets, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> Target {
        Target {
            device_id: "demo-1".into(),
            on: true,
            brightness: Some(30),
            white: Some(160),
        }
    }
    #[tokio::test]
    async fn scenes_persist_edit_delete_and_remain_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scenes.json");
        let store = Scenes::new(path.clone());
        let scenes = store
            .save(None, " Scena demo ".into(), vec![target()])
            .await
            .unwrap();
        let store = Scenes::new(path.clone());
        assert_eq!(store.list().await.unwrap()[0].name, "Scena demo");
        let off = Target {
            on: false,
            brightness: None,
            white: None,
            ..target()
        };
        store
            .save(
                Some(scenes[0].id.clone()),
                "Scena modificata".into(),
                vec![off],
            )
            .await
            .unwrap();
        assert!(!Scenes::new(path.clone()).list().await.unwrap()[0].targets[0].on);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        store.delete(&scenes[0].id).await.unwrap();
        assert!(Scenes::new(path).list().await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn invalid_scenes_and_corrupt_files_are_not_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scenes.json");
        let store = Scenes::new(path.clone());
        store
            .save(None, "Demo".into(), vec![target()])
            .await
            .unwrap();
        for targets in [
            vec![],
            vec![target(), target()],
            vec![Target {
                brightness: Some(0),
                ..target()
            }],
            vec![Target {
                white: Some(168),
                ..target()
            }],
            vec![Target {
                on: false,
                ..target()
            }],
        ] {
            assert!(store.save(None, "Demo".into(), targets).await.is_err());
        }
        assert_eq!(store.list().await.unwrap().len(), 1);
        std::fs::write(&path, "broken").unwrap();
        assert!(Scenes::new(path.clone())
            .save(None, "Demo".into(), vec![target()])
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "broken");
    }
    #[tokio::test]
    async fn legacy_neutral_scenes_remain_editable_but_cannot_run_or_be_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scenes.json");
        let legacy = Scene {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Scena demo precedente".into(),
            targets: vec![Target {
                white: Some(161),
                ..target()
            }],
        };
        write_local(&path, std::slice::from_ref(&legacy)).unwrap();
        let store = Scenes::new(path.clone());
        let loaded = store.list().await.unwrap();
        assert_eq!(loaded[0].targets[0].white, Some(161));
        assert!(validate_targets(&loaded[0].targets).is_err());
        assert!(store
            .save(Some(legacy.id.clone()), legacy.name.clone(), legacy.targets)
            .await
            .is_err());
        store
            .save(Some(legacy.id), "Scena corretta".into(), vec![target()])
            .await
            .unwrap();
        assert_eq!(
            Scenes::new(path).list().await.unwrap()[0].targets[0].white,
            Some(160)
        );
    }
}
