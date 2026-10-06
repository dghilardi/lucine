use crate::local_store::{read_local, write_local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::Mutex;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub device_ids: Vec<String>,
}

pub struct Zones {
    path: PathBuf,
    values: Mutex<Result<Vec<Zone>, String>>,
}

impl Zones {
    pub fn new(path: PathBuf) -> Self {
        let values = read_local::<Zone>(&path).and_then(|zones| {
            validate(&zones)?;
            Ok(zones)
        });
        Self {
            path,
            values: Mutex::new(values),
        }
    }

    pub async fn list(&self) -> Result<Vec<Zone>, String> {
        self.values.lock().await.clone()
    }

    pub async fn save(
        &self,
        id: Option<String>,
        name: String,
        mut device_ids: Vec<String>,
    ) -> Result<Vec<Zone>, String> {
        let mut values = self.values.lock().await;
        let mut next = values.clone()?;
        device_ids.sort();
        device_ids.dedup();
        let zone = Zone {
            id: id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: name.trim().into(),
            device_ids,
        };
        if let Some(id) = id {
            let index = next
                .iter()
                .position(|zone| zone.id == id)
                .ok_or("Zona non trovata. Riapri le impostazioni.")?;
            next[index] = zone;
        } else {
            next.push(zone);
        }
        validate(&next)?;
        self.persist(&next)?;
        *values = Ok(next.clone());
        Ok(next)
    }

    pub async fn delete(&self, id: &str) -> Result<Vec<Zone>, String> {
        let mut values = self.values.lock().await;
        let mut next = values.clone()?;
        let index = next
            .iter()
            .position(|zone| zone.id == id)
            .ok_or("Zona non trovata.")?;
        next.remove(index);
        self.persist(&next)?;
        *values = Ok(next.clone());
        Ok(next)
    }

    fn persist(&self, zones: &[Zone]) -> Result<(), String> {
        write_local(&self.path, zones)
    }
}

fn validate(zones: &[Zone]) -> Result<(), String> {
    if zones.len() > 32 {
        return Err("Puoi creare al massimo 32 zone.".into());
    }
    let mut ids = std::collections::HashSet::new();
    for zone in zones {
        if uuid::Uuid::parse_str(&zone.id).is_err()
            || !ids.insert(&zone.id)
            || zone.name.trim().is_empty()
            || zone.name.chars().count() > 80
            || zone.name.chars().any(char::is_control)
            || zone.device_ids.is_empty()
            || zone.device_ids.len() > 128
            || zone.device_ids.iter().any(|id| {
                id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
        {
            return Err("Scegli un nome valido e almeno una lampadina per la zona.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn zones_survive_restart_and_support_edit_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zones.json");
        let store = Zones::new(path.clone());
        let saved = store
            .save(
                None,
                " Zona demo ".into(),
                vec!["demo-2".into(), "demo-1".into(), "demo-1".into()],
            )
            .await
            .unwrap();
        assert_eq!(saved[0].name, "Zona demo");
        assert_eq!(saved[0].device_ids.len(), 2);
        let store = Zones::new(path.clone());
        assert_eq!(store.list().await.unwrap()[0].id, saved[0].id);
        store
            .save(
                Some(saved[0].id.clone()),
                "Zona rinominata".into(),
                vec!["demo-2".into()],
            )
            .await
            .unwrap();
        assert_eq!(store.list().await.unwrap()[0].name, "Zona rinominata");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        store.delete(&saved[0].id).await.unwrap();
        assert!(Zones::new(path).list().await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn invalid_updates_leave_existing_zones_intact() {
        let dir = tempfile::tempdir().unwrap();
        let store = Zones::new(dir.path().join("zones.json"));
        store
            .save(None, "Demo".into(), vec!["demo-1".into()])
            .await
            .unwrap();
        for (name, ids) in [
            ("", vec!["demo-1".into()]),
            ("Demo", vec![]),
            ("Demo", vec!["device/#".into()]),
        ] {
            assert!(store.save(None, name.into(), ids).await.is_err());
        }
        assert_eq!(store.list().await.unwrap().len(), 1);
    }
    #[tokio::test]
    async fn corrupt_configuration_is_not_silently_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zones.json");
        std::fs::write(&path, "bad-json").unwrap();
        let store = Zones::new(path.clone());
        assert!(store
            .save(None, "Demo".into(), vec!["demo-1".into()])
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "bad-json");
    }
}
