mod cloud;
mod local_store;
mod protocol;
mod scenes;
mod zones;

use cloud::{Catalog, RoomDraft, SceneDraft};
use protocol::{Backend, Lamp, LampState};
use scenes::{Scene, Scenes, Target};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem, Submenu},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};
use tauri_plugin_autostart::ManagerExt;
use zones::{Zone, Zones};

#[tauri::command]
async fn refresh(backend: State<'_, Arc<Backend>>) -> Result<Vec<Lamp>, String> {
    backend.inner().clone().refresh().await
}

#[tauri::command]
async fn control(
    backend: State<'_, Arc<Backend>>,
    id: String,
    kind: String,
    value: u64,
) -> Result<LampState, String> {
    backend.control(&id, &kind, value).await
}

#[tauri::command]
async fn import_session(backend: State<'_, Arc<Backend>>, path: String) -> Result<(), String> {
    backend.import(&path).await
}

#[tauri::command]
async fn cloud_catalog(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
) -> Result<Catalog, String> {
    let result = backend.cloud_catalog().await;
    let _ = update_tray(&app).await;
    result
}
#[tauri::command]
async fn save_cloud_room(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
    draft: RoomDraft,
) -> Result<Catalog, String> {
    let result = backend.save_cloud_room(draft).await;
    update_saved_groups(&app).await;
    result
}
#[tauri::command]
async fn delete_cloud_room(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
    id: u64,
    revision: String,
) -> Result<Catalog, String> {
    let result = backend.delete_cloud_room(id, &revision).await;
    update_saved_groups(&app).await;
    result
}
#[tauri::command]
async fn save_cloud_scene(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
    draft: SceneDraft,
) -> Result<Catalog, String> {
    let result = backend.save_cloud_scene(draft).await;
    update_saved_groups(&app).await;
    result
}
#[tauri::command]
async fn delete_cloud_scene(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
    id: String,
    revision: String,
) -> Result<Catalog, String> {
    let result = backend.delete_cloud_scene(&id, &revision).await;
    update_saved_groups(&app).await;
    result
}
#[tauri::command]
async fn run_cloud_room(
    app: tauri::AppHandle,
    id: u64,
    kind: String,
    value: u64,
) -> Result<Vec<Lamp>, String> {
    run_action(&app, DesktopAction::CloudRoom { id, kind, value }).await
}
#[tauri::command]
async fn run_cloud_scene(app: tauri::AppHandle, id: String) -> Result<Vec<Lamp>, String> {
    run_action(&app, DesktopAction::CloudScene { id }).await
}

#[tauri::command]
fn autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|_| "Non riesco a leggere l’avvio automatico.".into())
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|_| {
        "Non riesco a modificare l’avvio automatico. Controlla i permessi della sessione Linux."
    })?;
    manager
        .is_enabled()
        .map_err(|_| "Non riesco a verificare l’avvio automatico.".into())
}

#[tauri::command]
async fn list_zones(zones: State<'_, Zones>) -> Result<Vec<Zone>, String> {
    zones.list().await
}

#[tauri::command]
async fn save_zone(
    app: tauri::AppHandle,
    zones: State<'_, Zones>,
    id: Option<String>,
    name: String,
    device_ids: Vec<String>,
) -> Result<Vec<Zone>, String> {
    let values = zones.save(id, name, device_ids).await?;
    update_saved_groups(&app).await;
    Ok(values)
}

#[tauri::command]
async fn delete_zone(
    app: tauri::AppHandle,
    zones: State<'_, Zones>,
    id: String,
) -> Result<Vec<Zone>, String> {
    let values = zones.delete(&id).await?;
    update_saved_groups(&app).await;
    Ok(values)
}

#[derive(Default)]
struct TrayStatus {
    busy: AtomicBool,
    item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

#[tauri::command]
async fn list_scenes(scenes: State<'_, Scenes>) -> Result<Vec<Scene>, String> {
    scenes.list().await
}

#[tauri::command]
async fn save_scene(
    app: tauri::AppHandle,
    scenes: State<'_, Scenes>,
    id: Option<String>,
    name: String,
    targets: Vec<Target>,
) -> Result<Vec<Scene>, String> {
    let values = scenes.save(id, name, targets).await?;
    update_saved_groups(&app).await;
    Ok(values)
}

#[tauri::command]
async fn delete_scene(
    app: tauri::AppHandle,
    scenes: State<'_, Scenes>,
    id: String,
) -> Result<Vec<Scene>, String> {
    let values = scenes.delete(&id).await?;
    update_saved_groups(&app).await;
    Ok(values)
}

#[tauri::command]
async fn run_zone(
    app: tauri::AppHandle,
    id: String,
    kind: String,
    value: u64,
) -> Result<Vec<Lamp>, String> {
    run_action(&app, DesktopAction::Zone { id, kind, value }).await
}

#[tauri::command]
async fn run_scene(app: tauri::AppHandle, id: String) -> Result<Vec<Lamp>, String> {
    run_action(&app, DesktopAction::Scene { id }).await
}

enum DesktopAction {
    CloudRoom {
        id: u64,
        kind: String,
        value: u64,
    },
    CloudScene {
        id: String,
    },
    Zone {
        id: String,
        kind: String,
        value: u64,
    },
    Scene {
        id: String,
    },
}

fn tray_menu(
    app: &tauri::AppHandle,
    zones: &[Zone],
    scenes: &[Scene],
    cloud: Option<&Catalog>,
) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "show",
        "Apri Lucine",
        true,
        None::<&str>,
    )?)?;
    let available = !app.state::<TrayStatus>().busy.load(Ordering::SeqCst);
    for zone in zones {
        let group = Submenu::new(app, &zone.name, true)?;
        for (kind, choices) in [
            (
                "power",
                vec![(1, "Accendi".to_string()), (0, "Spegni".to_string())],
            ),
            (
                "brightness",
                [25, 50, 75, 100]
                    .into_iter()
                    .map(|v| (v, format!("{v}%")))
                    .collect(),
            ),
            ("white", vec![(160, "Caldo".into()), (162, "Freddo".into())]),
        ] {
            let submenu = Submenu::new(
                app,
                if kind == "brightness" {
                    "Luminosità"
                } else {
                    "Bianco"
                },
                true,
            )?;
            for (value, label) in choices {
                let item = MenuItem::with_id(
                    app,
                    format!("zone:{}:{kind}:{value}", zone.id),
                    label,
                    available,
                    None::<&str>,
                )?;
                if kind == "power" {
                    group.append(&item)?;
                } else {
                    submenu.append(&item)?;
                }
            }
            if kind != "power" {
                group.append(&submenu)?;
            }
        }
        menu.append(&group)?;
    }
    if zones.is_empty() {
        menu.append(&MenuItem::new(
            app,
            "Crea una zona nelle impostazioni",
            false,
            None::<&str>,
        )?)?;
    }
    let scene_menu = Submenu::new(app, "Scene locali", true)?;
    for scene in scenes {
        scene_menu.append(&MenuItem::with_id(
            app,
            format!("scene:{}", scene.id),
            &scene.name,
            available,
            None::<&str>,
        )?)?;
    }
    if scenes.is_empty() {
        scene_menu.append(&MenuItem::new(
            app,
            "Crea una scena nelle impostazioni",
            false,
            None::<&str>,
        )?)?;
    }
    menu.append(&scene_menu)?;
    if let Some(cloud) = cloud {
        let rooms = Submenu::new(app, "Stanze Android", true)?;
        for room in &cloud.rooms {
            let group = Submenu::new(app, &room.name, true)?;
            for (kind, value, label) in [
                ("power", 1, "Accendi"),
                ("power", 0, "Spegni"),
                ("white", 160, "Bianco caldo"),
                ("white", 162, "Bianco freddo"),
            ] {
                group.append(&MenuItem::with_id(
                    app,
                    format!("cloud-room:{}:{kind}:{value}", room.id),
                    label,
                    available && !room.device_ids.is_empty(),
                    None::<&str>,
                )?)?;
            }
            rooms.append(&group)?;
        }
        if cloud.rooms.is_empty() {
            rooms.append(&MenuItem::new(
                app,
                "Nessuna stanza cloud",
                false,
                None::<&str>,
            )?)?;
        }
        menu.append(&rooms)?;
        let scenes = Submenu::new(app, "Scene Android", true)?;
        for scene in &cloud.scenes {
            scenes.append(&MenuItem::with_id(
                app,
                format!("cloud-scene:{}", scene.id),
                &scene.name,
                available && scene.editable,
                None::<&str>,
            )?)?;
        }
        if cloud.scenes.is_empty() {
            scenes.append(&MenuItem::new(
                app,
                "Nessuna scena cloud",
                false,
                None::<&str>,
            )?)?;
        }
        menu.append(&scenes)?;
    }
    let status = MenuItem::with_id(app, "status", "Pronto", false, None::<&str>)?;
    menu.append(&status)?;
    *app.state::<TrayStatus>().item.lock().unwrap() = Some(status);
    menu.append(&MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?)?;
    Ok(menu)
}

async fn update_tray(app: &tauri::AppHandle) -> Result<(), String> {
    // An unreadable section must not remove the other section's valid shortcuts.
    let zones = app.state::<Zones>().list().await.unwrap_or_default();
    let scenes = app.state::<Scenes>().list().await.unwrap_or_default();
    let cloud = app.state::<Arc<Backend>>().cached_cloud_catalog().await;
    let tray = app
        .tray_by_id("main-tray")
        .ok_or("La tray non è disponibile.")?;
    tray.set_menu(Some(
        tray_menu(app, &zones, &scenes, cloud.as_ref())
            .map_err(|_| "Non riesco ad aggiornare il menu della tray.")?,
    ))
    .map_err(|_| "Non riesco ad aggiornare il menu della tray.".into())
}

async fn update_saved_groups(app: &tauri::AppHandle) {
    if update_tray(app).await.is_err() {
        let _ = app.emit("zone-result", serde_json::json!({
            "message": "Configurazione salvata, ma il menu della tray non è aggiornato. Riavvia Lucine.", "lamps": null
        }));
        show_window(app);
    }
    let _ = app.emit("groups-changed", ());
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn tray_action(id: &str) -> Option<DesktopAction> {
    if let Some(id) = id.strip_prefix("cloud-scene:") {
        return Some(DesktopAction::CloudScene { id: id.into() });
    }
    if let Some(id) = id.strip_prefix("cloud-room:") {
        let parts: Vec<_> = id.split(':').collect();
        if parts.len() != 3 {
            return None;
        }
        return Some(DesktopAction::CloudRoom {
            id: parts[0].parse().ok()?,
            kind: parts[1].into(),
            value: parts[2].parse().ok()?,
        });
    }
    if let Some(id) = id.strip_prefix("scene:") {
        return Some(DesktopAction::Scene { id: id.into() });
    }
    let mut parts = id.strip_prefix("zone:")?.split(':');
    let action = DesktopAction::Zone {
        id: parts.next()?.into(),
        kind: parts.next()?.into(),
        value: parts.next()?.parse().ok()?,
    };
    if parts.next().is_some() {
        None
    } else {
        Some(action)
    }
}

async fn run_action(app: &tauri::AppHandle, action: DesktopAction) -> Result<Vec<Lamp>, String> {
    if app.state::<TrayStatus>().busy.swap(true, Ordering::SeqCst) {
        return Err("Attendi il completamento dell’operazione in corso.".into());
    }
    let _ = app.emit("zone-busy", true);
    let result = async {
        update_tray(app).await?;
        if let Some(item) = app.state::<TrayStatus>().item.lock().unwrap().as_ref() {
            let _ = item.set_text("Operazione in corso…");
        }
        match action {
            DesktopAction::CloudRoom { id, kind, value } => {
                app.state::<Arc<Backend>>()
                    .control_cloud_room(id, &kind, value)
                    .await
            }
            DesktopAction::CloudScene { id } => {
                app.state::<Arc<Backend>>().apply_cloud_scene(&id).await
            }
            DesktopAction::Zone { id, kind, value } => {
                let zones = app.state::<Zones>().list().await?;
                let zone = zones
                    .iter()
                    .find(|zone| zone.id == id)
                    .ok_or("Zona non trovata.")?;
                app.state::<Arc<Backend>>()
                    .control_zone(&zone.device_ids, &kind, value)
                    .await
            }
            DesktopAction::Scene { id } => {
                let scenes = app.state::<Scenes>().list().await?;
                let scene = scenes
                    .iter()
                    .find(|scene| scene.id == id)
                    .ok_or("Scena non trovata.")?;
                app.state::<Arc<Backend>>()
                    .apply_scene(&scene.targets)
                    .await
            }
        }
    }
    .await;
    let (message, failed) = match &result {
        Ok(lamps) => {
            let confirmed = lamps.iter().filter(|lamp| lamp.error.is_none()).count();
            (
                format!("{} di {} lampadine confermate", confirmed, lamps.len()),
                confirmed != lamps.len(),
            )
        }
        Err(error) => (error.clone(), true),
    };
    app.state::<TrayStatus>()
        .busy
        .store(false, Ordering::SeqCst);
    let _ = update_tray(app).await;
    if let Some(item) = app.state::<TrayStatus>().item.lock().unwrap().as_ref() {
        let _ = item.set_text(&message);
    }
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(&message));
    }
    let _ = app.emit(
        "zone-result",
        serde_json::json!({"message":message,"lamps":result.as_ref().ok()}),
    );
    let _ = app.emit("zone-busy", false);
    if failed {
        show_window(app);
    }
    result
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    base.join("it.local.lucine").join("session.json")
}

pub fn run() {
    let backend = Arc::new(Backend::new(config_path()));
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|s| s == "--probe" || s == "--import-session")
    {
        let runtime = tokio::runtime::Runtime::new().expect("Runtime");
        let result = runtime.block_on(async {
            if let Some(index) = args.iter().position(|s| s == "--import-session") {
                let source = args.get(index + 1).ok_or("Specifica il percorso del file della sessione.")?;
                backend.import(source).await?;
            }
            if args.iter().any(|s| s == "--probe") {
                let lamps = backend.refresh().await?;
                // La verifica da terminale mostra soltanto nomi e stato, mai token.
                let states: Vec<_> = lamps.into_iter().map(|lamp| serde_json::json!({"name":lamp.name,"state":lamp.state,"error":lamp.error})).collect();
                println!("{}", serde_json::to_string_pretty(&states).unwrap());
            }
            Ok::<(), String>(())
        });
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        if args.iter().any(|s| s == "--probe") {
            return;
        }
    }
    let background = args.iter().any(|arg| arg == "--background");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|arg| arg == "--background") {
                show_window(app);
            }
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Lucine")
                .arg("--background")
                .build(),
        )
        .manage(Zones::new(config_path().with_file_name("zones.json")))
        .manage(Scenes::new(config_path().with_file_name("scenes.json")))
        .manage(TrayStatus::default())
        .manage(backend)
        .invoke_handler(tauri::generate_handler![
            cloud_catalog,
            save_cloud_room,
            delete_cloud_room,
            save_cloud_scene,
            delete_cloud_scene,
            run_cloud_room,
            run_cloud_scene,
            refresh,
            control,
            import_session,
            autostart_enabled,
            set_autostart,
            list_zones,
            save_zone,
            delete_zone,
            list_scenes,
            save_scene,
            delete_scene,
            run_zone,
            run_scene
        ])
        .setup(move |app| {
            let zones =
                tauri::async_runtime::block_on(app.state::<Zones>().list()).unwrap_or_default();
            let scenes =
                tauri::async_runtime::block_on(app.state::<Scenes>().list()).unwrap_or_default();
            let menu = tray_menu(app.handle(), &zones, &scenes, None)?;
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("Lucine · DreamCatcher Life")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    if id == "show" {
                        show_window(app);
                    } else if id == "quit" {
                        app.exit(0);
                    } else if let Some(action) = tray_action(id) {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = run_action(&app, action).await;
                        });
                    }
                })
                .build(app)?;
            if !background {
                show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("Impossibile avviare Lucine");
}
