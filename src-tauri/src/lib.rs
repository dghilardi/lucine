mod protocol;

use protocol::{Backend, Lamp, LampState};
use std::{path::PathBuf, sync::Arc};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State,
};

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
    tauri::Builder::default()
        .manage(backend)
        .invoke_handler(tauri::generate_handler![refresh, control, import_session])
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Apri Lucine", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Lucine · DreamCatcher Life")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
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
