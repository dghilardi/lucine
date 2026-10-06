import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  createIcons,
  Lightbulb,
  RefreshCw,
  Settings2,
  X,
  ArrowRight,
} from "lucide";
import "./style.css";
import { settings } from "./settings";
import { listen } from "@tauri-apps/api/event";

type State = {
  on: boolean;
  mode: number;
  brightness: number;
  white: string | null;
};
type Lamp = {
  id: string;
  name: string;
  state: State | null;
  error: string | null;
};
const native = isTauri();
let lamps: Lamp[] = [];
let refreshing = false;
let zoneBusy = false;
let manipulating = false;
let nextRowId = 0;
const busy = new Set<string>();
const rows = new Map<string, HTMLElement>();
const timers = new Map<string, ReturnType<typeof setTimeout>>();

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <header class="header">
    <div class="identity"><span class="app-mark"><i data-lucide="lightbulb"></i></span><div><h1>Lucine</h1><p>Le luci di casa, a portata di mano.</p></div></div>
    <div class="toolbar"><button id="refresh" class="icon-button" aria-label="Aggiorna lo stato" title="Aggiorna lo stato"><i data-lucide="refresh-cw"></i></button><button id="settings" class="icon-button" aria-label="Impostazioni" title="Impostazioni"><i data-lucide="settings-2"></i></button></div>
  </header>
  <main>
    <div class="section-heading"><h2>Le tue lampadine</h2><span id="summary" role="status">Collegamento in corso…</span></div>
    <div id="notice" class="notice" role="status" hidden></div>
    <div id="lamps" aria-label="Lampadine"></div>
    <div id="empty" class="empty" hidden><span class="empty-icon"><i data-lucide="lightbulb"></i></span><h3>Collega le tue luci</h3><p>Importa una sessione DreamCatcher Life dalle impostazioni per ritrovare le lampadine associate al tuo account.</p><button id="connect" class="primary-button">Collega account <i data-lucide="arrow-right"></i></button></div>
  </main>
  <footer><span class="connection"><span id="connection-dot" class="dot"></span><span id="connection-text">Connessione al cloud</span></span><span id="updated">In attesa dello stato</span></footer>
  <dialog id="settings-dialog" aria-labelledby="settings-title">
    <div class="dialog-heading"><h2 id="settings-title">Impostazioni</h2><button type="button" id="close-settings" class="icon-button" aria-label="Chiudi impostazioni"><i data-lucide="x"></i></button></div>
    <section class="settings-section"><h3>Avvio</h3><label class="check-row"><input id="autostart" type="checkbox" disabled><span>Avvia Lucine all’accesso a Linux</span></label><p class="field-help">Si apre nella tray, senza cambiare lo stato delle luci. Se sposti l’AppImage, disattiva e riattiva questa opzione dal nuovo percorso.</p><p id="autostart-error" class="error" role="alert" hidden></p></section>
    <section class="settings-section"><h3>Zone nella tray</h3><p class="field-help">Raggruppa le lampadine per accenderle e spegnerle dal menu della tray.</p><div id="zone-list"></div><p id="zones-empty" class="field-help">Non hai ancora creato zone.</p><form id="zone-form"><label for="zone-name">Nome della zona</label><input id="zone-name" maxlength="80" placeholder="Nome della zona" required><fieldset><legend>Lampadine della zona</legend><div id="zone-members"></div></fieldset><p id="zone-error" class="error" role="alert" hidden></p><div class="form-actions"><button id="save-zone" class="primary-button" type="submit" disabled>Crea zona</button><button id="cancel-zone" class="secondary-button" type="button" hidden>Annulla modifica</button></div></form></section>
    <section class="settings-section"><h3>Account DreamCatcher Life</h3><form id="session-form"><p>Importa una sessione autorizzata dell’app Android. Dopo l’importazione puoi controllare le luci senza tenere aperto l’emulatore.</p><label for="session-path">File della sessione</label><input id="session-path" type="text" placeholder="/percorso/session.json" required autocomplete="off" spellcheck="false"><p class="field-help">Il token resta sul tuo PC. Se la sessione scade, importa un file aggiornato.</p><p id="session-error" class="error" role="alert" hidden></p><button id="import" class="primary-button" type="submit">Importa sessione <i data-lucide="arrow-right"></i></button></form></section>
  </dialog>
  <template id="lamp-template"><article class="lamp-row"><div class="lamp-top"><div class="lamp-title"><span class="lamp-icon"><i data-lucide="lightbulb"></i></span><div><h3></h3><span class="lamp-description">Lettura dello stato…</span></div></div><button class="power-button" type="button" role="switch" aria-checked="false"><span class="switch-track"><span></span></span><span class="power-label">In attesa</span></button></div><div class="lamp-controls"><div class="brightness-control"><div class="control-label"><label>Luminosità</label><output>—</output></div><input class="brightness" type="range" min="1" max="100" step="1" value="1"></div><fieldset class="white-control"><legend>Bianco</legend><div class="segmented"><button type="button" data-mode="160">Caldo</button><button type="button" data-mode="161">Neutro</button><button type="button" data-mode="162">Freddo</button></div></fieldset></div><p class="lamp-error error" role="alert" hidden></p></article></template>`;

const $ = <T extends HTMLElement = HTMLElement>(selector: string) =>
  document.querySelector<T>(selector)!;
const dialog = $<HTMLDialogElement>("#settings-dialog");
function icons() {
  createIcons({
    icons: { Lightbulb, RefreshCw, Settings2, X, ArrowRight },
    attrs: { "aria-hidden": "true", "stroke-width": 1.8 },
  });
}
const loadSettings = settings(dialog, () => lamps);
function openSettings() {
  dialog.showModal();
  void loadSettings();
}
$("#settings").addEventListener("click", openSettings);
$("#connect").addEventListener("click", openSettings);
$("#close-settings").addEventListener("click", () => dialog.close());
$("#refresh").addEventListener("click", () => refresh());
dialog.addEventListener("click", (event) => {
  if (event.target === dialog) dialog.close();
});

function describe(state: State | null) {
  if (!state) return "Non raggiungibile";
  if (!state.on) return "Spenta";
  return (
    (
      {
        129: "Bianco personalizzato",
        160: "Bianco caldo",
        161: "Bianco neutro",
        162: "Bianco freddo",
        163: "Modalità notte",
        164: "Comfort",
        165: "Resa naturale",
        166: "Relax",
        167: "Concentrazione",
        168: "TV",
      } as Record<number, string>
    )[state.mode] || "Accesa"
  );
}

function render() {
  const container = $("#lamps");
  for (const [id, row] of rows)
    if (!lamps.some((lamp) => lamp.id === id)) {
      clearTimeout(timers.get(id));
      timers.delete(id);
      row.remove();
      rows.delete(id);
    }
  lamps.forEach((lamp) => {
    let row = rows.get(lamp.id);
    if (!row) {
      row = $<HTMLTemplateElement>(
        "#lamp-template",
      ).content.firstElementChild!.cloneNode(true) as HTMLElement;
      rows.set(lamp.id, row);
      container.append(row);
      const range = row.querySelector<HTMLInputElement>(".brightness")!;
      range.id = `brightness-${nextRowId++}`;
      row.querySelector("label")!.htmlFor = range.id;
      range.setAttribute("aria-label", `Luminosità di ${lamp.name}`);
      row.querySelector("h3")!.textContent = lamp.name;
      const power = row.querySelector<HTMLButtonElement>(".power-button")!;
      power.setAttribute("aria-label", `Accensione di ${lamp.name}`);
      power.addEventListener("click", () => {
        const current = lamps.find((item) => item.id === lamp.id);
        if (current?.state)
          void control(lamp.id, "power", current.state.on ? 0 : 1);
      });
      range.addEventListener("pointerdown", () => {
        manipulating = true;
      });
      range.addEventListener("pointerup", () => {
        manipulating = false;
      });
      range.addEventListener("blur", () => {
        manipulating = false;
      });
      range.addEventListener("pointercancel", () => {
        manipulating = false;
      });
      range.addEventListener("input", () => {
        row!.querySelector("output")!.textContent = `${range.value}%`;
      });
      range.addEventListener("change", () => {
        manipulating = false;
        clearTimeout(timers.get(lamp.id));
        timers.set(
          lamp.id,
          setTimeout(() => {
            timers.delete(lamp.id);
            void control(lamp.id, "brightness", Number(range.value));
          }, 300),
        );
      });
      row
        .querySelectorAll<HTMLButtonElement>("[data-mode]")
        .forEach((button) =>
          button.addEventListener(
            "click",
            () => void control(lamp.id, "white", Number(button.dataset.mode)),
          ),
        );
    }
    row.querySelector("h3")!.textContent = lamp.name;
    row
      .querySelector<HTMLInputElement>(".brightness")!
      .setAttribute("aria-label", `Luminosità di ${lamp.name}`);
    row
      .querySelector(".power-button")!
      .setAttribute("aria-label", `Accensione di ${lamp.name}`);
    const pending = busy.has(lamp.id);
    row.classList.toggle("is-on", Boolean(lamp.state?.on));
    row.classList.toggle("is-offline", !lamp.state);
    row.setAttribute("aria-busy", String(pending));
    row.querySelector(".lamp-description")!.textContent = pending
      ? "Aggiornamento…"
      : describe(lamp.state);
    const power = row.querySelector<HTMLButtonElement>(".power-button")!;
    power.setAttribute("aria-checked", String(Boolean(lamp.state?.on)));
    row.querySelector(".power-label")!.textContent = pending
      ? "Attendi…"
      : lamp.state
        ? lamp.state.on
          ? "Accesa"
          : "Spenta"
        : "Offline";
    const range = row.querySelector<HTMLInputElement>(".brightness")!;
    if (
      !timers.has(lamp.id) &&
      !pending &&
      !(manipulating && document.activeElement === range)
    )
      range.value = String(lamp.state?.brightness || 1);
    if (!timers.has(lamp.id))
      row.querySelector("output")!.textContent = lamp.state
        ? `${lamp.state.brightness}%`
        : "—";
    row.querySelectorAll<HTMLButtonElement>("[data-mode]").forEach((button) => {
      const active = lamp.state?.mode === Number(button.dataset.mode);
      button.setAttribute("aria-pressed", String(active));
    });
    row
      .querySelectorAll<HTMLButtonElement | HTMLInputElement>("button,input")
      .forEach((control) => {
        control.disabled =
          !native ||
          zoneBusy ||
          refreshing ||
          pending ||
          !lamp.state ||
          (!lamp.state.on && !control.classList.contains("power-button"));
      });
    const error = row.querySelector<HTMLElement>(".lamp-error")!;
    error.hidden = !lamp.error;
    error.textContent = lamp.error;
  });
  $("#empty").hidden = lamps.length > 0 || refreshing;
  const reachable = lamps.filter((lamp) => lamp.state).length;
  $("#summary").textContent = refreshing
    ? "Aggiornamento…"
    : lamps.length
      ? `${reachable} di ${lamps.length} collegate`
      : "Nessuna lampadina";
  $("#connection-dot").classList.toggle("connected", reachable > 0 && native);
  $("#connection-text").textContent = native
    ? reachable > 0
      ? "Cloud collegato"
      : "Cloud non collegato"
    : "Anteprima dell’interfaccia";
  $<HTMLButtonElement>("#refresh").disabled =
    refreshing || zoneBusy || busy.size > 0;
  icons();
}

async function loadPreview(): Promise<Lamp[]> {
  const response = await fetch("/preview-state.json");
  if (!response.ok) throw new Error("Anteprima non disponibile.");
  return response.json();
}

async function refresh() {
  if (refreshing || zoneBusy || busy.size || timers.size || manipulating)
    return;
  refreshing = true;
  render();
  try {
    lamps = native ? await invoke<Lamp[]>("refresh") : await loadPreview();
    $("#notice").hidden = native;
    $("#notice").textContent =
      "Anteprima con dati dimostrativi inventati. Apri Lucine sul desktop per usare i controlli.";
    $("#updated").textContent = native
      ? `Aggiornato alle ${new Date().toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" })}`
      : "Dati dimostrativi";
  } catch (error) {
    $("#notice").hidden = false;
    $("#notice").textContent = String(error);
    // Uno stato vecchio non resta azionabile dopo un errore di sessione/rete.
    lamps = lamps.map((lamp) => ({
      ...lamp,
      state: null,
      error: "Aggiorna la connessione prima di controllare questa luce.",
    }));
  } finally {
    refreshing = false;
    render();
  }
}

async function control(id: string, kind: string, value: number) {
  if (!native || refreshing || zoneBusy || busy.has(id)) return;
  const lamp = lamps.find((item) => item.id === id);
  if (!lamp?.state) return;
  clearTimeout(timers.get(id));
  timers.delete(id);
  const focused = document.activeElement as HTMLElement | null;
  busy.add(id);
  lamp.error = null;
  render();
  try {
    lamp.state = await invoke<State>("control", { id, kind, value });
  } catch (error) {
    lamp.error = String(error);
    lamp.state = null;
  } finally {
    busy.delete(id);
    render();
    focused?.focus({ preventScroll: true });
  }
}

$("#session-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const error = $("#session-error");
  const button = $<HTMLButtonElement>("#import");
  if (!native) {
    error.hidden = false;
    error.textContent = "Importa la sessione dalla finestra desktop di Lucine.";
    return;
  }
  button.disabled = true;
  error.hidden = true;
  try {
    await invoke("import_session", {
      path: $<HTMLInputElement>("#session-path").value.trim(),
    });
    dialog.close();
    await refresh();
  } catch (message) {
    error.hidden = false;
    error.textContent = String(message);
  } finally {
    button.disabled = false;
  }
});
if (native) {
  void listen<boolean>("zone-busy", (event) => {
    zoneBusy = event.payload;
    if (zoneBusy) {
      for (const timer of timers.values()) clearTimeout(timer);
      timers.clear();
    }
    render();
  }).catch(() => {});
  void listen<{ message: string; lamps: Lamp[] | null }>(
    "zone-result",
    (event) => {
      const result = event.payload;
      if (result.lamps) {
        for (const updated of result.lamps) {
          const existing = lamps.find((lamp) => lamp.id === updated.id);
          if (existing) Object.assign(existing, updated);
        }
        render();
      }
      $("#notice").hidden = false;
      $("#notice").textContent = result.message;
    },
  ).catch(() => {
    $("#notice").hidden = false;
    $("#notice").textContent =
      "Non riesco a ricevere gli aggiornamenti dalla tray. Usa Aggiorna per leggere lo stato.";
  });
}
icons();
void refresh();
setInterval(() => {
  if (!document.hidden && !dialog.open) void refresh();
}, 60000);
