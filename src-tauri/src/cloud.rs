//! Independent projection of cloud metadata. Raw configurations stay in Rust.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDevice {
    pub id: String,
    pub name: String,
    pub room_id: u64,
    #[serde(skip_serializing)]
    pub account_id: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudRoom {
    pub id: u64,
    pub name: String,
    pub device_ids: Vec<String>,
    pub other_devices: usize,
    pub revision: String,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PowerTarget {
    pub device_id: String,
    pub on: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudScene {
    pub id: String,
    pub name: String,
    pub targets: Vec<PowerTarget>,
    pub editable: bool,
    pub reason: Option<String>,
    pub revision: String,
}
#[derive(Clone, Serialize, Default)]
pub struct Catalog {
    pub rooms: Vec<CloudRoom>,
    pub scenes: Vec<CloudScene>,
    pub devices: Vec<CloudDevice>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoomDraft {
    pub id: Option<u64>,
    pub revision: Option<String>,
    pub name: String,
    pub device_ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneDraft {
    pub id: Option<String>,
    pub revision: Option<String>,
    pub name: String,
    pub targets: Vec<PowerTarget>,
    pub timezone: String,
}
pub fn revision(value: &Value) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    value.to_string().hash(&mut hash);
    format!("{:016x}", hash.finish())
}
pub fn name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        Err("Scegli un nome da 1 a 80 caratteri, senza caratteri di controllo.".into())
    } else {
        Ok(value.into())
    }
}
pub fn new_room_body(home_id: u64, home_db: &str, room_name: &str) -> Result<Value, String> {
    Ok(json!({"homeID":home_id,"homeDB":home_db,"room":{"roomName":name(room_name)?}}))
}
pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn list<'a>(value: &'a Value, key: &str, empty_object: bool) -> Result<&'a [Value], String> {
    if empty_object && value.as_object().is_some_and(|o| o.is_empty()) {
        return Ok(&[]);
    }
    value[key]
        .as_array()
        .filter(|a| a.len() <= 1024)
        .map(Vec::as_slice)
        .ok_or_else(|| "Configurazione cloud non valida o troppo grande.".into())
}
pub fn supported(devices: &[Value]) -> Result<Vec<CloudDevice>, String> {
    let mut ids = HashSet::new();
    let mut account_ids = HashSet::new();
    devices
        .iter()
        .filter(|d| d["product_id"] == "12")
        .map(|d| {
            let id = d["ID"]
                .as_str()
                .filter(|id| identifier(id))
                .ok_or("Lampadina cloud non valida.")?;
            let account_id = d["devIdInt"]
                .as_u64()
                .filter(|id| *id > 0)
                .ok_or("Identificativo account lampadina non valido.")?;
            if !ids.insert(id) || !account_ids.insert(account_id) {
                return Err("Lampadina cloud duplicata.".into());
            }
            Ok(CloudDevice {
                account_id,
                id: id.into(),
                name: d["alias"]
                    .as_str()
                    .ok_or("Nome lampadina non valido.")?
                    .into(),
                room_id: d["roomID"].as_u64().ok_or("Stanza lampadina non valida.")?,
            })
        })
        .collect()
}
pub fn project(rooms: &Value, scenes: &Value, devices: &Value) -> Result<Catalog, String> {
    let raw_devices = list(devices, "list", false)?;
    let devices = supported(raw_devices)?;
    let mut room_ids = HashSet::new();
    let order = rooms["orders"].as_array().cloned().unwrap_or_default();
    let mut rooms = list(rooms, "list", false)?
        .iter()
        .map(|r| {
            let id = r["roomID"]
                .as_u64()
                .filter(|id| *id > 0)
                .ok_or("Stanza cloud non valida.")?;
            if !room_ids.insert(id) {
                return Err("Stanza cloud duplicata.".into());
            }
            let mut members: Vec<_> = raw_devices
                .iter()
                .filter(|d| d["roomID"].as_u64() == Some(id))
                .map(|d| d["ID"].clone())
                .collect();
            members.sort_by_key(Value::to_string);
            let device_ids: Vec<_> = devices
                .iter()
                .filter(|d| d.room_id == id)
                .map(|d| d.id.clone())
                .collect();
            Ok(CloudRoom {
                id,
                name: r["roomName"]
                    .as_str()
                    .ok_or("Nome stanza non valido.")?
                    .into(),
                other_devices: members.len() - device_ids.len(),
                device_ids,
                revision: revision(&json!([r, members])),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    rooms.sort_by_key(|room| {
        order
            .iter()
            .position(|id| id.as_u64() == Some(room.id))
            .unwrap_or(usize::MAX)
    });
    let mut scene_ids = HashSet::new();
    let scenes = list(scenes, "sceneList", true)?
        .iter()
        .map(|s| {
            let id = s["sceneId"]
                .as_str()
                .filter(|id| identifier(id))
                .ok_or("Scena cloud non valida.")?;
            if !scene_ids.insert(id) {
                return Err("Scena cloud duplicata.".into());
            }
            let parsed = power_targets(s, &devices);
            let editable = parsed.is_ok();
            let reason = parsed.as_ref().err().cloned();
            Ok(CloudScene {
                id: id.into(),
                name: s["name"].as_str().ok_or("Nome scena non valido.")?.into(),
                targets: parsed.unwrap_or_default(),
                editable,
                reason,
                revision: revision(s),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Catalog {
        rooms,
        scenes,
        devices,
    })
}
pub fn power_targets(scene: &Value, devices: &[CloudDevice]) -> Result<Vec<PowerTarget>, String> {
    const KEYS: &[&str] = &[
        "sceneId",
        "name",
        "homeId",
        "autoTimer",
        "city",
        "delay",
        "desc",
        "jSceneEn",
        "jiachao",
        "latitude",
        "longitude",
        "queryList",
        "repeatDay",
        "sceneEn",
        "scenePic",
        "taskList",
        "timer",
        "timerType",
        "timestamp",
        "tz",
    ];
    let unsupported = || {
        "Scena con automazioni, azioni o dispositivi non supportati. Gestiscila nell’app Android."
            .to_string()
    };
    if !scene
        .as_object()
        .is_some_and(|o| o.keys().all(|k| KEYS.contains(&k.as_str())))
        || [
            "autoTimer",
            "jSceneEn",
            "delay",
            "repeatDay",
            "timerType",
            "jiachao",
        ]
        .iter()
        .any(|k| scene[*k].as_u64() != Some(0))
        || scene["sceneEn"].as_u64() != Some(1)
        || !scene["queryList"].as_array().is_some_and(Vec::is_empty)
    {
        return Err(unsupported());
    }
    let tasks = scene["taskList"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 128)
        .ok_or_else(unsupported)?;
    let mut targets = Vec::new();
    let mut ids = HashSet::new();
    for t in tasks {
        if !t.as_object().is_some_and(|o| {
            o.keys()
                .all(|k| ["deviceIds", "dtype", "parentId", "sid", "subId"].contains(&k.as_str()))
        }) || t["dtype"] != "LT"
            || t["subId"] != ""
            || !matches!(t["sid"].as_u64(), Some(11 | 12))
        {
            return Err(unsupported());
        }
        let account_id = t["deviceIds"]
            .as_str()
            .and_then(|id| id.parse::<u64>().ok())
            .ok_or_else(unsupported)?;
        let device = devices
            .iter()
            .find(|d| d.account_id == account_id)
            .ok_or_else(unsupported)?;
        if !ids.insert(account_id) {
            return Err(unsupported());
        }
        targets.push(PowerTarget {
            device_id: device.id.clone(),
            on: t["sid"] == 11,
        });
    }
    Ok(targets)
}
pub fn validate_members(
    ids: &[String],
    devices: &[CloudDevice],
    allow_empty: bool,
) -> Result<(), String> {
    let mut unique = HashSet::new();
    if ids.len() > 128
        || (!allow_empty && ids.is_empty())
        || ids
            .iter()
            .any(|id| !unique.insert(id) || !devices.iter().any(|d| &d.id == id))
    {
        Err("Seleziona lampadine disponibili nell’account, senza duplicati (massimo 128).".into())
    } else {
        Ok(())
    }
}
pub fn scene_body(
    draft: &SceneDraft,
    previous: Option<&Value>,
    home_id: u64,
    devices: &[Value],
) -> Result<Value, String> {
    let supported = supported(devices)?;
    validate_members(
        &draft
            .targets
            .iter()
            .map(|t| t.device_id.clone())
            .collect::<Vec<_>>(),
        &supported,
        false,
    )?;
    if draft.timezone.is_empty()
        || draft.timezone.len() > 80
        || !draft
            .timezone
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
    {
        return Err("Fuso orario non valido.".into());
    }
    let mut body = if let Some(s) = previous {
        power_targets(s, &supported)?;
        s.clone()
    } else {
        json!({
            "homeId": home_id,
            "sceneEn": 1,
            "jSceneEn": 0,
            "autoTimer": 0,
            "delay": 0,
            "repeatDay": 0,
            "timerType": 0,
            "jiachao": 0,
            "queryList": [],
            "scenePic": 0,
            "desc": "",
            "city": "",
            "latitude": 0,
            "longitude": 0,
            "timer": "00:00",
            "tz": draft.timezone,
        })
    };
    body["name"] = json!(name(&draft.name)?);
    body["taskList"] = Value::Array(
        draft
            .targets
            .iter()
            .map(|target| {
                // validate_members above has resolved every target to a supported device.
                let device = devices
                    .iter()
                    .find(|d| d["ID"].as_str() == Some(&target.device_id))
                    .unwrap();
                json!({
                    "deviceIds": device["devIdInt"].as_u64().unwrap().to_string(),
                    "dtype": "LT",
                    "parentId": device["parentId"],
                    "sid": if target.on { 11 } else { 12 },
                    "subId": "",
                })
            })
            .collect(),
    );
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device() -> Value {
        json!({"ID":"fixture-bulb","alias":"Lampadina inventata","product_id":"12","roomID":7,"parentId":"","devIdInt":6543})
    }
    fn scene() -> Value {
        json!({"sceneId":"fixture-scene","name":"Scena inventata","autoTimer":0,"jSceneEn":0,"delay":0,"repeatDay":0,"timerType":0,"jiachao":0,"sceneEn":1,"queryList":[],"taskList":[{"deviceIds":"6543","dtype":"LT","parentId":"","sid":11,"subId":""}]})
    }
    #[test]
    fn catalog_scopes_members_and_blocks_unknown_actions() {
        let mut other = device();
        other["ID"] = json!("fixture-other");
        other["product_id"] = json!("99");
        other["devIdInt"] = json!(6544);
        let c = project(
            &json!({"list":[{"roomID":7,"roomName":"Stanza inventata"}]}),
            &json!({"sceneList":[scene()]}),
            &json!({"list":[device(),other]}),
        )
        .unwrap();
        assert_eq!(c.rooms[0].device_ids, vec!["fixture-bulb"]);
        assert_eq!(c.rooms[0].other_devices, 1);
        assert!(c.scenes[0].editable);
        for key in [
            "autoTimer",
            "jSceneEn",
            "delay",
            "repeatDay",
            "timerType",
            "jiachao",
        ] {
            let mut s = scene();
            s[key] = json!(1);
            assert!(power_targets(&s, &c.devices).is_err());
        }
        let mut s = scene();
        s["unknownAutomation"] = json!(1);
        assert!(power_targets(&s, &c.devices).is_err());
        let mut s = scene();
        s["taskList"][0]["subId"] = json!("unknown");
        assert!(power_targets(&s, &c.devices).is_err());
        s = scene();
        s["taskList"][0]["deviceIds"] = json!("fixture-other");
        assert!(power_targets(&s, &c.devices).is_err());
        s = scene();
        s["taskList"][0]["sid"] = json!(999);
        assert!(power_targets(&s, &c.devices).is_err());
    }
    #[test]
    fn edits_preserve_unrelated_fields_and_validate_members() {
        let mut old = scene();
        old["desc"] = json!("Descrizione inventata da conservare");
        let draft = SceneDraft {
            id: Some("fixture-scene".into()),
            revision: None,
            name: " Nuovo nome ".into(),
            targets: vec![PowerTarget {
                device_id: "fixture-bulb".into(),
                on: false,
            }],
            timezone: "UTC".into(),
        };
        let next = scene_body(&draft, Some(&old), 1, &[device()]).unwrap();
        assert_eq!(next["desc"], old["desc"]);
        assert_eq!(next["taskList"][0]["sid"], 12);
        assert_eq!(next["name"], "Nuovo nome");
        assert_ne!(revision(&old), revision(&next));
        let mut bad = draft;
        bad.targets.push(bad.targets[0].clone());
        assert!(scene_body(&bad, Some(&old), 1, &[device()]).is_err());
    }
    #[test]
    fn account_ids_map_to_mqtt_without_exposing_raw_configuration() {
        let mut raw = device();
        raw["password"] = json!("fixture-secret-placeholder");
        let c = project(
            &json!({"list": []}),
            &json!({"sceneList": [scene()]}),
            &json!({"list": [raw]}),
        )
        .unwrap();
        assert_eq!(c.scenes[0].targets[0].device_id, "fixture-bulb");
        let exported = serde_json::to_value(&c).unwrap();
        assert!(exported["devices"][0].get("accountId").is_none());
        assert!(exported["devices"][0].get("password").is_none());
        assert!(exported["scenes"][0].get("taskList").is_none());
        let draft = SceneDraft {
            id: None,
            revision: None,
            name: "Scena nuova inventata".into(),
            targets: c.scenes[0].targets.clone(),
            timezone: "UTC".into(),
        };
        let body = scene_body(&draft, None, 1, &[device()]).unwrap();
        assert_eq!(body["taskList"][0]["deviceIds"], "6543");
    }
    #[test]
    fn empty_scene_account_and_malformed_catalog_are_distinct() {
        assert!(list(&json!({}), "sceneList", true).unwrap().is_empty());
        assert!(list(&json!({"error":1}), "sceneList", true).is_err());
        assert!(name("\n").is_err());
        assert!(name("bad\nname").is_err());
        assert!(!identifier("../other"));
    }
    #[test]
    fn room_creation_uses_nested_object_and_revision_tracks_membership() {
        let body = new_room_body(1, "fixture-home", " Stanza inventata ").unwrap();
        assert!(body.get("roomName").is_none());
        assert_eq!(body["room"]["roomName"], "Stanza inventata");
        assert!(body["room"].is_object());
        let rooms = json!({"list":[{"roomID":7,"roomName":"Stanza inventata"}]});
        let a = project(&rooms, &json!({}), &json!({"list":[device()]})).unwrap();
        let mut moved = device();
        moved["roomID"] = json!(0);
        let b = project(&rooms, &json!({}), &json!({"list":[moved]})).unwrap();
        assert_ne!(a.rooms[0].revision, b.rooms[0].revision);
        assert!(validate_members(&["missing".into()], &a.devices, true).is_err());
    }
}
