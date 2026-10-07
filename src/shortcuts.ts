import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Zone } from "./settings";
import type { Scene } from "./scenes";

type Run = (
  command: "run_zone" | "run_scene",
  args: Record<string, unknown>,
) => Promise<void>;

export function shortcuts(
  root: HTMLDetailsElement,
  blocked: () => boolean,
  run: Run,
) {
  const native = isTauri();
  root.innerHTML = `<summary>Zone e scene</summary><p class="field-help">Luminosità e bianco si applicano alle luci accese. Crea zone e scene nelle impostazioni.</p><div class="zone-shortcuts"><label>Zona<select id="shortcut-zone"></select></label><div class="form-actions"><button type="button" data-power="1" class="secondary-button">Accendi zona</button><button type="button" data-power="0" class="secondary-button">Spegni zona</button></div><div class="zone-adjust"><label>Luminosità %<input id="zone-brightness" type="number" min="1" max="100" step="1" value="50"></label><button id="apply-zone-brightness" type="button" class="secondary-button">Applica luminosità</button><label>Bianco<select id="zone-white"><option value="160">Caldo</option><option value="162">Freddo</option></select></label><button id="apply-zone-white" type="button" class="secondary-button">Applica bianco</button></div></div><div id="scene-shortcuts" class="form-actions" aria-label="Scene locali"></div><p id="shortcut-error" class="error" role="alert" hidden></p>`;
  const select = root.querySelector<HTMLSelectElement>("#shortcut-zone")!;
  const error = root.querySelector<HTMLElement>("#shortcut-error")!;
  let loaded = false;
  let loading = false;
  let zones: Zone[] = [];
  let scenes: Scene[] = [];
  const report = (message: unknown) => {
    error.hidden = false;
    error.textContent = String(message);
  };
  function sync() {
    for (const input of Array.from(
      root.querySelectorAll<
        HTMLInputElement | HTMLSelectElement | HTMLButtonElement
      >(
        ".zone-shortcuts input, .zone-shortcuts select, .zone-shortcuts button",
      ),
    ))
      input.disabled = !native || blocked() || loading || !zones.length;
    for (const button of Array.from(
      root.querySelectorAll<HTMLButtonElement>("#scene-shortcuts button"),
    ))
      button.disabled = !native || blocked() || loading;
  }
  async function load() {
    if (loading || (loaded && !root.open)) return;
    loading = true;
    sync();
    error.hidden = true;
    const results = await Promise.allSettled(
      native
        ? [invoke<Zone[]>("list_zones"), invoke<Scene[]>("list_scenes")]
        : [Promise.resolve<Zone[]>([]), Promise.resolve<Scene[]>([])],
    );
    zones = results[0].status === "fulfilled" ? results[0].value : [];
    scenes = results[1].status === "fulfilled" ? results[1].value : [];
    for (const result of results)
      if (result.status === "rejected") report(result.reason);
    const previous = select.value;
    select.replaceChildren();
    for (const zone of zones) {
      const option = document.createElement("option");
      option.value = zone.id;
      option.textContent = zone.name;
      select.append(option);
    }
    if (zones.some((zone) => zone.id === previous)) select.value = previous;
    if (!zones.length) {
      const option = document.createElement("option");
      option.textContent = "Nessuna zona salvata";
      select.append(option);
    }
    const container = root.querySelector<HTMLElement>("#scene-shortcuts")!;
    container.replaceChildren();
    for (const scene of scenes) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "secondary-button";
      button.textContent = `Attiva ${scene.name}`;
      button.addEventListener("click", () => {
        if (native && !blocked()) void run("run_scene", { id: scene.id });
      });
      container.append(button);
    }
    if (!scenes.length) {
      const text = document.createElement("p");
      text.className = "field-help";
      text.textContent = "Nessuna scena salvata.";
      container.append(text);
    }
    loaded = true;
    loading = false;
    sync();
  }
  function action(kind: string, value: number) {
    if (!native || blocked() || loading || !zones.length) return;
    if (
      !Number.isInteger(value) ||
      (kind === "brightness" && (value < 1 || value > 100))
    ) {
      report("Scegli una luminosità da 1 a 100%.");
      return;
    }
    error.hidden = true;
    void run("run_zone", { id: select.value, kind, value });
  }
  for (const button of Array.from(
    root.querySelectorAll<HTMLButtonElement>("[data-power]"),
  ))
    button.addEventListener("click", () =>
      action("power", Number(button.dataset.power)),
    );
  root
    .querySelector("#apply-zone-brightness")!
    .addEventListener("click", () =>
      action(
        "brightness",
        Number(root.querySelector<HTMLInputElement>("#zone-brightness")!.value),
      ),
    );
  root
    .querySelector("#apply-zone-white")!
    .addEventListener("click", () =>
      action(
        "white",
        Number(root.querySelector<HTMLSelectElement>("#zone-white")!.value),
      ),
    );
  root.addEventListener("toggle", () => {
    if (root.open && !loaded) void load();
  });
  sync();
  return {
    sync,
    changed: () => {
      loaded = false;
      if (root.open) void load();
    },
  };
}
