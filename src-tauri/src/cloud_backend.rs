use super::{
    apply_zone, control_device_checked, select_zone, validate_session, validate_zone_command,
    Backend, Lamp, Session,
};
use crate::cloud::{self, Catalog, RoomDraft, SceneDraft};
use serde_json::{json, Value};
use std::sync::Arc;

impl Backend {
    fn cloud_request(
        &self,
        session: &Session,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<reqwest::RequestBuilder, String> {
        validate_session(session)?;
        if ![
            "/v2/group/home/rooms",
            "/v2/group/room",
            "/v2/group/room/order",
            "/v2/user/device/list",
            "/v2/user/device/account",
            "/v2/smart/scene/setting/all",
            "/v2/smart/scene/setting",
        ]
        .contains(&path)
        {
            return Err("Operazione cloud non valida.".into());
        }
        let mut request = self
            .http
            .request(
                method.clone(),
                format!("https://{}:{}{}", session.domain, session.port, path),
            )
            .query(query)
            .header("Token", &session.token);
        // Observed legacy account-update endpoint needs query authentication as well.
        // Do not extend this exception to other endpoints; errors never include URLs.
        if method == reqwest::Method::PUT && path == "/v2/user/device/account" {
            request = request.query(&[("token", &session.token)]);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        Ok(request)
    }
    async fn cloud_http(
        &self,
        session: &Session,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, String> {
        let request = self.cloud_request(session, method, path, query, body)?;
        let mut response = request.send().await.map_err(|_| {
            "Risposta cloud non ricevuta. Aggiorna prima di riprovare: una modifica potrebbe essere già stata salvata."
        })?;
        if !response.status().is_success() {
            return Err(format!(
                "Operazione cloud rifiutata (HTTP {}).",
                response.status().as_u16()
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Risposta cloud incompleta. Aggiorna prima di riprovare.")?
        {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err("Risposta cloud troppo grande.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "Risposta cloud non valida. Aggiorna prima di riprovare.")?;
        if !value.is_object()
            || value
                .get("error")
                .is_some_and(|e| !e.is_null() && e != 0 && e != false)
        {
            return Err(
                "Il cloud ha rifiutato l’operazione. Aggiorna la configurazione e riprova.".into(),
            );
        }
        Ok(value)
    }
    async fn cloud_data(&self, session: &Session) -> Result<(Catalog, Value, Value), String> {
        *self.cloud_cache.lock().await = None;
        let query = [
            ("homeID", session.home_id.to_string()),
            ("homeDB", session.home_db.clone()),
        ];
        let scene_query = [("homeId", session.home_id.to_string())];
        let (rooms, devices, scenes) = tokio::try_join!(
            self.cloud_http(
                session,
                reqwest::Method::GET,
                "/v2/group/home/rooms",
                &query,
                None
            ),
            self.cloud_http(
                session,
                reqwest::Method::GET,
                "/v2/user/device/list",
                &query,
                None
            ),
            self.cloud_http(
                session,
                reqwest::Method::GET,
                "/v2/smart/scene/setting/all",
                &scene_query,
                None
            )
        )?;
        if cloud::list(&devices, "list", false)?
            .iter()
            .any(|d| d["homeID"].as_u64() != Some(session.home_id))
            || cloud::list(&scenes, "sceneList", true)?
                .iter()
                .any(|s| s["homeId"].as_u64() != Some(session.home_id))
        {
            return Err("Il cloud ha restituito una configurazione di un’altra casa. Nessuna modifica inviata.".into());
        }
        let catalog = cloud::project(&rooms, &scenes, &devices)?;
        *self.cloud_cache.lock().await = Some(catalog.clone());
        Ok((catalog, scenes, devices))
    }
    pub async fn cloud_catalog(&self) -> Result<Catalog, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let result = self.cloud_data(&session).await;
        if result.is_err() {
            *self.cloud_cache.lock().await = None;
        }
        result.map(|(catalog, _, _)| catalog)
    }
    pub async fn cached_cloud_catalog(&self) -> Option<Catalog> {
        self.cloud_cache.lock().await.clone()
    }
    pub async fn save_cloud_room(&self, draft: RoomDraft) -> Result<Catalog, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let name = cloud::name(&draft.name)?;
        let (catalog, _, _) = self.cloud_data(&session).await?;
        cloud::validate_members(&draft.device_ids, &catalog.devices, true)?;
        *self.cloud_cache.lock().await = None;
        let id = if let Some(id) = draft.id {
            let room = catalog
                .rooms
                .iter()
                .find(|r| r.id == id)
                .ok_or("Stanza non trovata nell’account.")?;
            if draft.revision.as_deref() != Some(&room.revision) {
                return Err(
                    "La stanza è cambiata nell’app Android. Aggiorna e riapri la modifica.".into(),
                );
            }
            self.cloud_http(
                &session,
                reqwest::Method::PUT,
                "/v2/group/room",
                &[],
                Some(json!({
                    "homeID": session.home_id,
                    "homeDB": session.home_db,
                    "roomID": id,
                    "roomName": name,
                })),
            )
            .await?;
            id
        } else {
            if catalog.rooms.iter().any(|r| r.name == name) {
                return Err("Esiste già una stanza con questo nome.".into());
            }
            self.cloud_http(
                &session,
                reqwest::Method::POST,
                "/v2/group/home/rooms",
                &[],
                Some(cloud::new_room_body(
                    session.home_id,
                    &session.home_db,
                    &name,
                )?),
            )
            .await?;
            let (fresh, _, _) = self.cloud_data(&session).await?;
            let added: Vec<_> = fresh
                .rooms
                .iter()
                .filter(|r| r.name == name && !catalog.rooms.iter().any(|old| old.id == r.id))
                .collect();
            if added.len() != 1 {
                return Err("Creazione stanza non confermata. Aggiorna prima di riprovare.".into());
            }
            added[0].id
        };
        // Assign only supported bulbs. Other categories are never moved.
        *self.cloud_cache.lock().await = None;
        for device in &catalog.devices {
            let selected = draft.device_ids.contains(&device.id);
            let destination = if selected {
                id
            } else if device.room_id == id {
                0
            } else {
                continue;
            };
            if destination == device.room_id {
                continue;
            }
            self.cloud_http(
                &session,
                reqwest::Method::PUT,
                "/v2/user/device/account",
                &[],
                Some(json!({
                    "homeID": session.home_id,
                    "homeDB": session.home_db,
                    "devIdInt": device.account_id,
                    "roomID": destination,
                })),
            ).await.map_err(|_| {
                "Modifica stanza parziale o non confermata. Aggiorna prima di riprovare: alcune associazioni potrebbero essere già salvate."
            })?;
        }
        let (fresh, _, _) = self.cloud_data(&session).await?;
        let saved = fresh
            .rooms
            .iter()
            .find(|r| r.id == id)
            .ok_or("Stanza salvata non trovata. Aggiorna prima di riprovare.")?;
        if saved.name != name
            || saved.device_ids.len() != draft.device_ids.len()
            || !draft
                .device_ids
                .iter()
                .all(|id| saved.device_ids.contains(id))
        {
            return Err(
                "Modifica stanza non confermata o parziale. Aggiorna prima di riprovare.".into(),
            );
        }
        Ok(fresh)
    }
    pub async fn delete_cloud_room(&self, id: u64, revision: &str) -> Result<Catalog, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let (catalog, _, _) = self.cloud_data(&session).await?;
        let room = catalog
            .rooms
            .iter()
            .find(|r| r.id == id)
            .ok_or("Stanza non trovata nell’account.")?;
        if room.revision != revision {
            return Err("La stanza è cambiata. Aggiorna prima di eliminarla.".into());
        }
        if room.other_devices > 0 {
            return Err(
                "La stanza contiene altri tipi di dispositivi. Eliminala dall’app Android.".into(),
            );
        }
        *self.cloud_cache.lock().await = None;
        self.cloud_http(
            &session,
            reqwest::Method::DELETE,
            "/v2/group/home/rooms",
            &[],
            Some(json!({"homeID":session.home_id,"homeDB":session.home_db,"roomIDs":[id]})),
        )
        .await?;
        let (fresh, _, _) = self.cloud_data(&session).await?;
        if fresh.rooms.iter().any(|r| r.id == id) {
            return Err("Eliminazione stanza non confermata. Aggiorna prima di riprovare.".into());
        }
        self.cloud_http(
            &session,
            reqwest::Method::PUT,
            "/v2/group/room/order",
            &[],
            Some(json!({
                "homeID": session.home_id,
                "homeDB": session.home_db,
                "orders": fresh.rooms.iter().map(|r| r.id).collect::<Vec<_>>(),
            })),
        )
        .await
        .map_err(|_| {
            "Stanza eliminata, ma l’ordine cloud non è confermato. Aggiorna prima di riprovare."
        })?;
        Ok(fresh)
    }
    pub async fn save_cloud_scene(&self, draft: SceneDraft) -> Result<Catalog, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let (catalog, scenes, devices) = self.cloud_data(&session).await?;
        let old = if let Some(id) = &draft.id {
            let scene = catalog
                .scenes
                .iter()
                .find(|s| &s.id == id)
                .ok_or("Scena non trovata nell’account.")?;
            if draft.revision.as_deref() != Some(&scene.revision) {
                return Err(
                    "La scena è cambiata nell’app Android. Aggiorna e riapri la modifica.".into(),
                );
            }
            if !scene.editable {
                return Err(scene.reason.clone().unwrap());
            }
            cloud::list(&scenes, "sceneList", true)?
                .iter()
                .find(|s| s["sceneId"].as_str() == Some(id.as_str()))
        } else {
            None
        };
        let body = cloud::scene_body(
            &draft,
            old,
            session.home_id,
            cloud::list(&devices, "list", false)?,
        )?;
        *self.cloud_cache.lock().await = None;
        self.cloud_http(
            &session,
            if old.is_some() {
                reqwest::Method::PUT
            } else {
                reqwest::Method::POST
            },
            "/v2/smart/scene/setting",
            &[],
            Some(body),
        )
        .await?;
        let (fresh, _, _) = self.cloud_data(&session).await?;
        let candidates: Vec<_> = fresh
            .scenes
            .iter()
            .filter(|scene| {
                if let Some(id) = &draft.id {
                    &scene.id == id
                } else {
                    !catalog.scenes.iter().any(|old| old.id == scene.id)
                        && scene.name == draft.name.trim()
                }
            })
            .collect();
        if candidates.len() != 1
            || candidates[0].name != draft.name.trim()
            || !candidates[0].editable
            || candidates[0].targets.len() != draft.targets.len()
            || !draft
                .targets
                .iter()
                .all(|target| candidates[0].targets.contains(target))
        {
            return Err("Modifica scena non confermata. Aggiorna prima di riprovare.".into());
        }
        Ok(fresh)
    }
    pub async fn delete_cloud_scene(&self, id: &str, revision: &str) -> Result<Catalog, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let (catalog, _, _) = self.cloud_data(&session).await?;
        let scene = catalog
            .scenes
            .iter()
            .find(|s| s.id == id)
            .ok_or("Scena non trovata nell’account.")?;
        if scene.revision != revision {
            return Err("La scena è cambiata. Aggiorna prima di eliminarla.".into());
        }
        if !scene.editable {
            return Err(scene.reason.clone().unwrap());
        }
        *self.cloud_cache.lock().await = None;
        self.cloud_http(
            &session,
            reqwest::Method::DELETE,
            "/v2/smart/scene/setting",
            &[("sceneId", id.into())],
            None,
        )
        .await?;
        let (fresh, _, _) = self.cloud_data(&session).await?;
        if fresh.scenes.iter().any(|s| s.id == id) {
            return Err("Eliminazione scena non confermata. Aggiorna prima di riprovare.".into());
        }
        Ok(fresh)
    }
    pub async fn control_cloud_room(
        &self,
        id: u64,
        kind: &str,
        value: u64,
    ) -> Result<Vec<Lamp>, String> {
        validate_zone_command(kind, value)?;
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let (catalog, _, _) = self.cloud_data(&session).await?;
        let ids = &catalog
            .rooms
            .iter()
            .find(|r| r.id == id)
            .ok_or("Stanza non trovata nell’account.")?
            .device_ids;
        if ids.is_empty() {
            return Err("La stanza non contiene lampadine supportate.".into());
        }
        let (selected, mut missing) = select_zone(self.fetch_devices(&session).await?, ids)?;
        let kind = kind.to_string();
        let session = Arc::new(session);
        let mut lamps = apply_zone(selected, move |device| {
            let kind = kind.clone();
            let session = session.clone();
            async move { control_device_checked(&session, &device, &kind, value, true).await }
        })
        .await;
        missing.append(&mut lamps);
        Ok(missing)
    }
    pub async fn apply_cloud_scene(&self, id: &str) -> Result<Vec<Lamp>, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let (catalog, _, _) = self.cloud_data(&session).await?;
        let scene = catalog
            .scenes
            .iter()
            .find(|s| s.id == id)
            .ok_or("Scena non trovata nell’account.")?;
        if !scene.editable {
            return Err(scene.reason.clone().unwrap());
        }
        let targets = Arc::new(scene.targets.clone());
        let ids = targets
            .iter()
            .map(|t| t.device_id.clone())
            .collect::<Vec<_>>();
        let (selected, mut missing) = select_zone(self.fetch_devices(&session).await?, &ids)?;
        let session = Arc::new(session);
        let mut lamps = apply_zone(selected, move |device| {
            let session = session.clone();
            let targets = targets.clone();
            async move {
                let target = targets.iter().find(|t| t.device_id == device.id).unwrap();
                control_device_checked(&session, &device, "power", u64::from(target.on), false)
                    .await
            }
        })
        .await;
        missing.append(&mut lamps);
        Ok(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_authentication_is_restricted_and_destinations_remain_validated() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend::new(dir.path().join("session.json"));
        let mut session:Session=serde_json::from_value(json!({"AmToken":"YOUR_TOKEN","amDomain":"region.iotdreamcatcher.net","amPort":443,"UserId":"YOUR_USER","userDB":"fixture-user-db","homeID":1,"homeDB":"fixture-home-db"})).unwrap();
        let membership = backend
            .cloud_request(
                &session,
                reqwest::Method::PUT,
                "/v2/user/device/account",
                &[],
                Some(json!({"devIdInt":6543,"roomID":7})),
            )
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(membership.headers()["Token"], "YOUR_TOKEN");
        assert_eq!(
            membership.url().query_pairs().collect::<Vec<_>>(),
            vec![("token".into(), "YOUR_TOKEN".into())]
        );
        for (method, path) in [
            (reqwest::Method::GET, "/v2/user/device/list"),
            (reqwest::Method::POST, "/v2/smart/scene/setting"),
            (reqwest::Method::PUT, "/v2/group/room"),
        ] {
            let request = backend
                .cloud_request(&session, method, path, &[], None)
                .unwrap()
                .build()
                .unwrap();
            assert!(request.url().query_pairs().all(|(key, _)| key != "token"));
            assert!(request.headers().get("brand").is_none());
        }
        session.domain = "outside.example".into();
        assert!(backend
            .cloud_request(
                &session,
                reqwest::Method::PUT,
                "/v2/user/device/account",
                &[],
                None
            )
            .is_err());
    }
}
