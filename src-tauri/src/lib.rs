mod protocol;
mod zones;

use protocol::{Backend, Lamp, LampState};
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
    update_saved_zones(&app, &values);
    Ok(values)
}

#[tauri::command]
async fn delete_zone(
    app: tauri::AppHandle,
    zones: State<'_, Zones>,
    id: String,
) -> Result<Vec<Zone>, String> {
    let values = zones.delete(&id).await?;
    update_saved_zones(&app, &values);
    Ok(values)
}

#[derive(Default)]
struct TrayStatus {
    busy: AtomicBool,
    item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

fn tray_menu(app: &tauri::AppHandle, zones: &[Zone]) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    let show = MenuItem::with_id(app, "show", "Apri Lucine", true, None::<&str>)?;
    menu.append(&show)?;
    let available = !app.state::<TrayStatus>().busy.load(Ordering::SeqCst);
    for zone in zones {
        let on = MenuItem::with_id(
            app,
            format!("zone-on:{}", zone.id),
            "Accendi",
            available,
            None::<&str>,
        )?;
        let off = MenuItem::with_id(
            app,
            format!("zone-off:{}", zone.id),
            "Spegni",
            available,
            None::<&str>,
        )?;
        let submenu = Submenu::with_items(app, &zone.name, true, &[&on, &off])?;
        menu.append(&submenu)?;
    }
    if zones.is_empty() {
        menu.append(&MenuItem::new(
            app,
            "Crea una zona nelle impostazioni",
            false,
            None::<&str>,
        )?)?;
    }
    let status = MenuItem::with_id(app, "status", "Pronto", false, None::<&str>)?;
    menu.append(&status)?;
    *app.state::<TrayStatus>().item.lock().unwrap() = Some(status);
    menu.append(&MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?)?;
    Ok(menu)
}

fn update_tray(app: &tauri::AppHandle, zones: &[Zone]) -> Result<(), String> {
    let tray = app
        .tray_by_id("main-tray")
        .ok_or("La tray non è disponibile.")?;
    tray.set_menu(Some(
        tray_menu(app, zones).map_err(|_| "Non riesco ad aggiornare il menu della tray.")?,
    ))
    .map_err(|_| "Non riesco ad aggiornare il menu della tray.".into())
}

fn update_saved_zones(app: &tauri::AppHandle, zones: &[Zone]) {
    // Persistence has succeeded: do not report a failed save and invite duplicates.
    if update_tray(app, zones).is_err() {
        let _ = app.emit(
            "zone-result",
            serde_json::json!({
                "message": "Zone salvate, ma il menu della tray non è aggiornato. Riavvia Lucine.",
                "lamps": null
            }),
        );
        show_window(app);
    }
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn zone_event(app: &tauri::AppHandle, id: &str, on: bool) {
    if app.state::<TrayStatus>().busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = app.emit("zone-busy", true);
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let result = async {
            let zones = app.state::<Zones>().list().await?;
            let zone = zones
                .iter()
                .find(|zone| zone.id == id)
                .ok_or("Zona non trovata.")?;
            update_tray(&app, &zones)?;
            if let Some(item) = app.state::<TrayStatus>().item.lock().unwrap().as_ref() {
                let _ = item.set_text("Operazione in corso…");
            }
            app.state::<Arc<Backend>>()
                .power_zone(&zone.device_ids, on)
                .await
        }
        .await;
        app.state::<TrayStatus>()
            .busy
            .store(false, Ordering::SeqCst);
        let _ = app.emit("zone-busy", false);
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
        if let Ok(zones) = app.state::<Zones>().list().await {
            let _ = update_tray(&app, &zones);
        }
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
        if failed {
            show_window(&app);
        }
    });
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
        .manage(TrayStatus::default())
        .manage(backend)
        .invoke_handler(tauri::generate_handler![
            refresh,
            control,
            import_session,
            autostart_enabled,
            set_autostart,
            list_zones,
            save_zone,
            delete_zone
        ])
        .setup(move |app| {
            let zones =
                tauri::async_runtime::block_on(app.state::<Zones>().list()).unwrap_or_default();
            let menu = tray_menu(app.handle(), &zones)?;
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
                    } else if let Some(zone) = id.strip_prefix("zone-on:") {
                        zone_event(app, zone, true);
                    } else if let Some(zone) = id.strip_prefix("zone-off:") {
                        zone_event(app, zone, false);
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
