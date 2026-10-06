use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use tokio::sync::Mutex;

const SUPPORTED_PRODUCT_ID: &str = "12";

#[derive(Clone, Deserialize, Serialize)]
pub struct Session {
    #[serde(rename = "AmToken")]
    token: String,
    #[serde(rename = "amDomain")]
    domain: String,
    #[serde(rename = "amPort", deserialize_with = "number_or_string")]
    port: u16,
    #[serde(rename = "UserId")]
    user_id: String,
    #[serde(rename = "userDB")]
    user_db: String,
    #[serde(rename = "homeID")]
    home_id: u64,
    #[serde(rename = "homeDB")]
    home_db: String,
}

fn number_or_string<'de, D: serde::Deserializer<'de>>(de: D) -> Result<u16, D::Error> {
    let value = Value::deserialize(de)?;
    let n = value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()));
    n.and_then(|n| u16::try_from(n).ok())
        .ok_or_else(|| serde::de::Error::custom("Porta non valida"))
}

#[derive(Clone, Deserialize)]
pub struct Device {
    #[serde(rename = "ID")]
    pub id: String,
    product_id: String,
    alias: String,
    mqtt: Broker,
}

#[derive(Clone, Deserialize)]
struct Broker {
    domain: String,
    port: u16,
    token: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LampState {
    pub on: bool,
    pub mode: u64,
    pub brightness: u8,
    pub white: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lamp {
    pub id: String,
    pub name: String,
    pub state: Option<LampState>,
    pub error: Option<String>,
}

pub struct Backend {
    session_path: PathBuf,
    session: Mutex<Option<Session>>,
    devices: Mutex<HashMap<String, Device>>,
    gate: Mutex<()>,
    http: reqwest::Client,
}

impl Backend {
    pub fn new(session_path: PathBuf) -> Self {
        let session = read_session(&session_path)
            .ok()
            .filter(|session| validate_session(session).is_ok());
        Self {
            session_path,
            session: Mutex::new(session),
            devices: Mutex::new(HashMap::new()),
            gate: Mutex::new(()),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(15))
                .build()
                .expect("HTTP client"),
        }
    }

    pub async fn import(&self, source: &str) -> Result<(), String> {
        let _guard = self.gate.lock().await;
        let session = read_session(std::path::Path::new(source))?;
        validate_session(&session)?;
        // Authenticate before replacing a working session. Network errors never include URLs.
        self.fetch_devices(&session).await?;
        save_session(&self.session_path, &session)?;
        *self.session.lock().await = Some(session);
        self.devices.lock().await.clear();
        Ok(())
    }

    async fn get_session(&self) -> Result<Session, String> {
        self.session
            .lock()
            .await
            .clone()
            .ok_or_else(|| "Importa una sessione per collegare le lampadine.".into())
    }

    async fn fetch_devices(&self, session: &Session) -> Result<Vec<Device>, String> {
        let url = format!(
            "https://{}:{}/v2/user/device/list",
            session.domain, session.port
        );
        let response = self
            .http
            .get(url)
            .query(&[
                ("homeDB", session.home_db.clone()),
                ("homeID", session.home_id.to_string()),
            ])
            .header("Token", &session.token)
            .send()
            .await
            .map_err(|_| "Il cloud DreamCatcher non è raggiungibile. Controlla la connessione.")?;
        if !response.status().is_success() {
            return Err(format!(
                "Sessione rifiutata dal server (HTTP {}). Importa una sessione aggiornata.",
                response.status().as_u16()
            ));
        }
        let value: Value = response
            .json()
            .await
            .map_err(|_| "Risposta del server non valida.")?;
        let list = value
            .get("list")
            .and_then(Value::as_array)
            .ok_or("La sessione potrebbe essere scaduta. Importa una sessione aggiornata.")?;
        list.iter()
            .filter(|v| v["product_id"] == SUPPORTED_PRODUCT_ID)
            .map(|v| {
                let device: Device = serde_json::from_value(v.clone())
                    .map_err(|_| "Parametri della lampadina non validi.".to_string())?;
                validate_device(&device)?;
                Ok(device)
            })
            .collect()
    }

    pub async fn refresh(self: &Arc<Self>) -> Result<Vec<Lamp>, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let devices = self.fetch_devices(&session).await?;
        *self.devices.lock().await = devices.iter().map(|d| (d.id.clone(), d.clone())).collect();
        let mut jobs = Vec::new();
        for device in devices {
            let session = session.clone();
            jobs.push(tokio::spawn(async move {
                let result = exchange(&session, &device, None).await;
                match result {
                    Ok(raw) => Lamp {
                        id: device.id,
                        name: device.alias,
                        state: Some(decode_state(&raw)),
                        error: None,
                    },
                    Err(error) => Lamp {
                        id: device.id,
                        name: device.alias,
                        state: None,
                        error: Some(error),
                    },
                }
            }));
        }
        let mut lamps = Vec::new();
        for job in jobs {
            lamps.push(job.await.map_err(|_| "Lettura dello stato interrotta.")?);
        }
        Ok(lamps)
    }

    pub async fn control(&self, id: &str, kind: &str, value: u64) -> Result<LampState, String> {
        let _guard = self.gate.lock().await;
        let session = self.get_session().await?;
        let device = self
            .devices
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or("Aggiorna l’elenco prima di inviare un comando.")?;
        let current = exchange(&session, &device, None).await?;
        let request = build_command(kind, value, &current)?;
        let result = exchange(&session, &device, Some(request)).await?;
        Ok(decode_state(&result))
    }
}

// Only manufacturer DNS names are accepted: an imported file must not redirect tokens.
fn vendor_domain(domain: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    domain.len() <= 253
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && ["iotdreamcatcher.net", "iotdreamcatcher.net.cn"]
            .iter()
            .any(|suffix| domain == *suffix || domain.ends_with(&format!(".{suffix}")))
}

fn validate_session(session: &Session) -> Result<(), String> {
    if !vendor_domain(&session.domain) {
        return Err("Il server della sessione non è un dominio DreamCatcher consentito.".into());
    }
    if [
        &session.token,
        &session.user_id,
        &session.user_db,
        &session.home_db,
    ]
    .iter()
    .any(|value| value.trim().is_empty())
        || session.port == 0
    {
        return Err("La sessione è incompleta.".into());
    }
    Ok(())
}

fn validate_device(device: &Device) -> Result<(), String> {
    if device.product_id != SUPPORTED_PRODUCT_ID
        || device.id.is_empty()
        || device.id.len() > 128
        || !device
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || !vendor_domain(&device.mqtt.domain)
        || device.mqtt.port == 0
        || device.mqtt.token.trim().is_empty()
    {
        return Err("Parametri della lampadina non validi o broker non supportato.".into());
    }
    Ok(())
}

fn read_session(path: &std::path::Path) -> Result<Session, String> {
    const LIMIT: u64 = 64 * 1024;
    let file =
        std::fs::File::open(path).map_err(|_| "Non riesco a leggere il file della sessione.")?;
    let metadata = file
        .metadata()
        .map_err(|_| "File della sessione non valido.")?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err("La sessione deve essere un file JSON di massimo 64 KiB.".into());
    }
    let mut raw = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut raw)
        .map_err(|_| "Non riesco a leggere il file della sessione.")?;
    if raw.len() as u64 > LIMIT {
        return Err("Il file della sessione è troppo grande.".into());
    }
    serde_json::from_slice(&raw)
        .map_err(|_| "Il file non contiene una sessione DreamCatcher valida.".into())
}

fn save_session(path: &std::path::Path, session: &Session) -> Result<(), String> {
    let parent = path.parent().ok_or("Percorso della sessione non valido.")?;
    let mut directory = std::fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory
        .create(parent)
        .map_err(|_| "Non riesco a creare la cartella delle impostazioni.")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Non riesco a salvare la sessione.")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| "Non riesco a proteggere il file della sessione.")?;
    }
    serde_json::to_writer(&mut temporary, session)
        .map_err(|_| "Non riesco a salvare la sessione.")?;
    temporary
        .flush()
        .map_err(|_| "Non riesco a salvare la sessione.")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "Non riesco a salvare la sessione.")?;
    temporary
        .persist(path)
        .map_err(|_| "Non riesco a salvare la sessione.")?;
    Ok(())
}

fn valid_state(raw: &Value) -> bool {
    match raw["mo"].as_u64() {
        Some(129) => {
            raw["lc"].as_u64().is_some_and(|v| v <= 255)
                && ["wh", "wv"]
                    .iter()
                    .all(|key| raw[*key].as_u64().is_some_and(|v| v <= 1000))
        }
        Some(160..=168) => raw["ls"].as_u64().is_some_and(|v| v <= 175),
        Some(224 | 225) => true,
        _ => false,
    }
}

fn build_command(kind: &str, value: u64, current: &Value) -> Result<Value, String> {
    match kind {
        "power" if value <= 1 => {
            Ok(json!({"a":"value_set","mo":if value == 1 {225} else {224},"rand":0}))
        }
        "white" if (160..=162).contains(&value) => Ok(json!({"a":"value_set","mo":value,"rand":0})),
        "brightness" if (1..=100).contains(&value) => {
            let mode = current["mo"]
                .as_u64()
                .ok_or("Stato della lampadina incompleto.")?;
            if (160..=168).contains(&mode) {
                let level = 20 + ((value * 155 + 50) / 100);
                Ok(json!({"a":"value_set","mo":mode,"ls":level,"rand":0}))
            } else if mode == 129 {
                if !["wh", "wv"]
                    .iter()
                    .all(|key| current[*key].as_u64().is_some_and(|v| v <= 1000))
                {
                    return Err("Coordinate del bianco non valide.".into());
                }
                Ok(
                    json!({"a":"value_set","mo":129,"lc":5 + ((value * 250 + 50) / 100),"wh":current["wh"],"wv":current["wv"],"rand":0}),
                )
            } else {
                Err("Accendi la lampadina in una modalità bianco supportata prima di regolare la luminosità.".into())
            }
        }
        _ => Err("Comando non valido.".into()),
    }
}

fn confirms(raw: &Value, command: &Value) -> bool {
    let mode = raw["mo"].as_u64();
    let target = command["mo"].as_u64();
    let power = if target == Some(225) {
        mode.is_some_and(|m| m != 224)
    } else {
        mode == target
    };
    power
        && ["lc", "ls", "wh", "wv"].iter().all(|key| {
            command
                .get(key)
                .is_none_or(|value| raw.get(key) == Some(value))
        })
}

async fn exchange(
    session: &Session,
    device: &Device,
    command: Option<Value>,
) -> Result<Value, String> {
    validate_device(device)?;
    let host = &device.mqtt.domain;
    let mut options = MqttOptions::new(
        format!(
            "and_{}_{}",
            device.id,
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ),
        host,
        device.mqtt.port,
    );
    options.set_credentials(
        format!("{}_{}{}", device.id, session.user_db, session.user_id),
        device.mqtt.token.clone(),
    );
    options.set_transport(Transport::tls_with_config(TlsConfiguration::Native));
    options.set_keep_alive(Duration::from_secs(30));
    let (client, mut events) = AsyncClient::new(options, 10);
    let topic = format!("smart/{}/dc/{}/din/config", device.id, device.product_id);
    let state_topic = format!("smart/{}/dc/{}/dout/#", device.id, device.product_id);
    let query = json!({"m":{"req":{"a":"bulb_conf"}}}).to_string();
    let result = tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let event = events
                .poll()
                .await
                .map_err(|_| "Connessione MQTT non riuscita; verifica la rete e la sessione.")?;
            match event {
                Event::Incoming(Packet::ConnAck(ack)) => {
                    if ack.code != rumqttc::ConnectReturnCode::Success {
                        return Err("Il broker MQTT ha rifiutato la sessione.".into());
                    }
                    client
                        .subscribe(&state_topic, QoS::AtMostOnce)
                        .await
                        .map_err(|_| "Sottoscrizione MQTT non riuscita.")?;
                }
                Event::Incoming(Packet::SubAck(ack)) => {
                    if ack
                        .return_codes
                        .iter()
                        .any(|code| matches!(code, rumqttc::SubscribeReasonCode::Failure))
                    {
                        return Err("Sottoscrizione MQTT rifiutata.".into());
                    }
                    if let Some(ref request) = command {
                        client
                            .publish(
                                &topic,
                                QoS::AtMostOnce,
                                false,
                                json!({"m":{"req":request}}).to_string(),
                            )
                            .await
                            .map_err(|_| "Invio del comando non riuscito.")?;
                    }
                    client
                        .publish(&topic, QoS::AtMostOnce, false, query.as_str())
                        .await
                        .map_err(|_| "Lettura dello stato non riuscita.")?;
                }
                Event::Incoming(Packet::Publish(message)) => {
                    if let Ok(raw) = serde_json::from_slice::<Value>(&message.payload) {
                        if let Some(state) = raw.get("m").and_then(|m| m.get("res")) {
                            if state["a"] == "bulb_conf" && valid_state(state) {
                                if command.as_ref().is_none_or(|cmd| confirms(state, cmd)) {
                                    return Ok(state.clone());
                                }
                                tokio::time::sleep(Duration::from_millis(150)).await;
                                client
                                    .publish(&topic, QoS::AtMostOnce, false, query.as_str())
                                    .await
                                    .map_err(|_| "Verifica del comando non riuscita.")?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    })
    .await
    .map_err(|_| "La lampadina non risponde. Potrebbe essere offline.")?;
    // Il drop chiude la connessione, senza mantenere client o credenziali nella UI.
    result
}

fn decode_state(raw: &Value) -> LampState {
    let mode = raw["mo"].as_u64().unwrap_or(224);
    let (level, min, range) = if (160..=168).contains(&mode) {
        (raw["ls"].as_f64().unwrap_or(20.0), 20.0, 155.0)
    } else {
        (raw["lc"].as_f64().unwrap_or(5.0), 5.0, 250.0)
    };
    LampState {
        on: mode != 224,
        mode,
        brightness: (((level - min) / range * 100.0).round().clamp(1.0, 100.0)) as u8,
        white: match mode {
            160 => Some("warm".into()),
            161 => Some("neutral".into()),
            162 => Some("cool".into()),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn power_on_confirmation_uses_restored_mode() {
        assert!(confirms(&json!({"mo":163}), &json!({"mo":225})));
        assert!(!confirms(&json!({"mo":224}), &json!({"mo":225})));
    }
    #[test]
    fn brightness_preserves_scene_and_requires_confirmation() {
        let command = build_command("brightness", 50, &json!({"mo":163})).unwrap();
        assert_eq!(command["mo"], 163);
        assert!(!confirms(&json!({"mo":163,"ls":10}), &command));
        assert!(confirms(&json!({"mo":163,"ls":command["ls"]}), &command));
    }
    #[test]
    fn invalid_controls_do_not_generate_messages() {
        assert!(build_command("white", 224, &json!({})).is_err());
        assert!(build_command("brightness", 0, &json!({"mo":129})).is_err());
    }

    fn demo_session() -> Session {
        serde_json::from_value(json!({
            "AmToken": "synthetic-test-value", "amDomain": "test.iotdreamcatcher.net",
            "amPort": "443", "UserId": "demo-user", "userDB": "demo-db",
            "homeID": 1, "homeDB": "demo-home"
        }))
        .unwrap()
    }

    #[test]
    fn imported_servers_cannot_redirect_credentials() {
        for domain in [
            "localhost",
            "127.0.0.1",
            "iotdreamcatcher.net.evil.example",
            "eviliotdreamcatcher.net",
            "user@test.iotdreamcatcher.net",
            "test.iotdreamcatcher.net/path",
            "-bad.iotdreamcatcher.net",
        ] {
            let mut session = demo_session();
            session.domain = domain.into();
            assert!(validate_session(&session).is_err(), "{domain}");
        }
        assert!(vendor_domain("TEST.iotdreamcatcher.net.cn"));
    }

    #[test]
    fn session_ports_accept_legacy_strings_but_reject_overflow() {
        let session = demo_session();
        assert_eq!(session.port, 443);
        let mut value = serde_json::to_value(session).unwrap();
        value["amPort"] = json!(65536);
        assert!(serde_json::from_value::<Session>(value).is_err());
        let mut session = demo_session();
        session.user_id.clear();
        assert!(validate_session(&session).is_err());
    }

    #[test]
    fn session_replacement_is_private_and_drops_unneeded_fields() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config/session.json");
        let mut value = serde_json::to_value(demo_session()).unwrap();
        value["unneeded_private_data"] = json!("must-not-be-saved");
        let session: Session = serde_json::from_value(value).unwrap();
        save_session(&path, &session).unwrap();
        let mut replacement = demo_session();
        replacement.home_id = 2;
        save_session(&path, &replacement).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let saved: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(saved["homeID"], 2);
        assert!(saved.get("unneeded_private_data").is_none());
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn malformed_or_unsupported_states_are_not_actionable() {
        for raw in [
            json!({"mo":129}),
            json!({"mo":162}),
            json!({"mo":999,"ls":50}),
            json!({"mo":129,"lc":50,"wh":1001,"wv":0}),
        ] {
            assert!(!valid_state(&raw));
        }
        assert!(valid_state(&json!({"mo":163,"ls":10})));
        assert!(valid_state(&json!({"mo":224})));
    }

    #[test]
    fn custom_brightness_preserves_and_confirms_white_coordinates() {
        let command =
            build_command("brightness", 100, &json!({"mo":129,"wh":400,"wv":600})).unwrap();
        assert_eq!(command["lc"], 255);
        assert_eq!(command["wh"], 400);
        assert!(confirms(
            &json!({"mo":129,"lc":255,"wh":400,"wv":600}),
            &command
        ));
        assert!(!confirms(
            &json!({"mo":129,"lc":255,"wh":0,"wv":600}),
            &command
        ));
        assert!(build_command("brightness", 50, &json!({"mo":129})).is_err());
        assert!(build_command("brightness", 50, &json!({"mo":224})).is_err());
    }

    #[test]
    fn state_display_clamps_night_and_maps_brightness_endpoints() {
        assert_eq!(decode_state(&json!({"mo":163,"ls":10})).brightness, 1);
        assert_eq!(decode_state(&json!({"mo":160,"ls":175})).brightness, 100);
        assert_eq!(decode_state(&json!({"mo":129,"lc":255})).brightness, 100);
        assert!(!decode_state(&json!({"mo":224})).on);
    }

    #[test]
    fn device_topics_and_brokers_are_scoped() {
        let mut device: Device = serde_json::from_value(json!({"ID":"demo-device","product_id":"12","alias":"Demo","mqtt":{"domain":"test.iotdreamcatcher.net","port":8883,"token":"synthetic-test-value"}})).unwrap();
        assert!(validate_device(&device).is_ok());
        device.id = "other/#".into();
        assert!(validate_device(&device).is_err());
        device.id = "demo-device".into();
        device.product_id = "13".into();
        assert!(validate_device(&device).is_err());
        device.product_id = "12".into();
        device.mqtt.domain = "untrusted.example".into();
        assert!(validate_device(&device).is_err());
    }
    #[test]
    fn session_import_rejects_large_and_malformed_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("demo.json");
        std::fs::write(&path, vec![b' '; 65537]).unwrap();
        assert!(read_session(&path).is_err());
        std::fs::write(&path, b"not-json").unwrap();
        assert!(read_session(&path).is_err());
        std::fs::write(&path, serde_json::to_vec(&demo_session()).unwrap()).unwrap();
        assert!(read_session(&path).is_ok());
    }
}
