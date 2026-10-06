import { invoke, isTauri } from "@tauri-apps/api/core";

export type Target = {
  deviceId: string;
  on: boolean;
  brightness: number | null;
  white: number | null;
};
export type Scene = { id: string; name: string; targets: Target[] };
type Choice = {
  id: string;
  name: string;
  state?: { on: boolean; brightness: number; mode: number } | null;
};

export function scenesEditor(root: HTMLElement, lamps: () => Choice[]) {
  const native = isTauri();
  root.innerHTML = `<h3>Scene locali</h3><p class="field-help">Scegli cosa deve fare ogni lampadina. Salva la scena e richiamala dalla tray o da “Zone e scene”. Salvare non cambia le luci.</p><div id="scene-list"></div><p id="scenes-empty" class="field-help">Non hai ancora creato scene.</p><form id="scene-form"><label for="scene-name">Nome della scena</label><input id="scene-name" maxlength="80" required placeholder="Nome della scena"><fieldset><legend>Azioni per lampadina</legend><div id="scene-targets"></div></fieldset><p id="scene-error" class="error" role="alert" hidden></p><div class="form-actions"><button id="save-scene" type="submit" class="primary-button">Crea scena</button><button id="cancel-scene" type="button" class="secondary-button" hidden>Annulla modifica</button></div></form>`;
  const find = <T extends HTMLElement>(id: string) =>
    root.querySelector<T>(`#${id}`)!;
  const name = find<HTMLInputElement>("scene-name");
  const error = find<HTMLElement>("scene-error");
  const save = find<HTMLButtonElement>("save-scene");
  const cancel = find<HTMLButtonElement>("cancel-scene");
  let scenes: Scene[] = [];
  let editing: string | null = null;
  let ready = false;
  let saving = false;
  const report = (message: unknown) => {
    error.hidden = false;
    error.textContent = String(message);
  };

  function readTargets(): Target[] {
    return Array.from(root.querySelectorAll<HTMLElement>(".scene-target"))
      .filter(
        (row) =>
          row.querySelector<HTMLInputElement>(".include-target")!.checked,
      )
      .map((row) => {
        const on =
          row.querySelector<HTMLSelectElement>(".target-power")!.value === "on";
        return {
          deviceId: row.dataset.id!,
          on,
          brightness: on
            ? Number(
                row.querySelector<HTMLInputElement>(".target-brightness")!
                  .value,
              )
            : null,
          white: on
            ? Number(
                row.querySelector<HTMLSelectElement>(".target-white")!.value,
              )
            : null,
        };
      });
  }
  function targets(values: Target[] = []) {
    const container = find<HTMLElement>("scene-targets");
    container.replaceChildren();
    const choices = [...lamps()];
    for (const target of values)
      if (!choices.some((lamp) => lamp.id === target.deviceId))
        choices.push({
          id: target.deviceId,
          name: "Lampadina non disponibile",
        });
    for (const lamp of choices) {
      const target = values.find((target) => target.deviceId === lamp.id);
      const row = document.createElement("div");
      row.className = "scene-target";
      row.dataset.id = lamp.id;
      row.innerHTML = `<label class="check-row"><input type="checkbox" class="include-target"><span></span></label><div class="target-controls"><label>Stato<select class="target-power"><option value="on">Accesa</option><option value="off">Spenta</option></select></label><label>Bianco<select class="target-white"><option value="160">Caldo</option><option value="161">Neutro</option><option value="162">Freddo</option></select></label><label>Luminosità %<input class="target-brightness" type="number" min="1" max="100" step="1" required></label></div>`;
      row.querySelector(".check-row span")!.textContent = lamp.name;
      const include = row.querySelector<HTMLInputElement>(".include-target")!;
      const power = row.querySelector<HTMLSelectElement>(".target-power")!;
      const white = row.querySelector<HTMLSelectElement>(".target-white")!;
      const brightness =
        row.querySelector<HTMLInputElement>(".target-brightness")!;
      include.checked = !!target;
      power.value = (target?.on ?? lamp.state?.on ?? true) ? "on" : "off";
      white.value = String(
        target?.white ??
          (lamp.state && [160, 161, 162].includes(lamp.state.mode)
            ? lamp.state.mode
            : 160),
      );
      brightness.value = String(
        target?.brightness ?? Math.max(1, lamp.state?.brightness ?? 50),
      );
      const sync = () => {
        include.disabled = !native || !ready || saving;
        power.disabled = include.disabled || !include.checked;
        white.disabled = brightness.disabled =
          power.disabled || power.value === "off";
      };
      include.addEventListener("change", sync);
      power.addEventListener("change", sync);
      sync();
      container.append(row);
    }
    name.disabled = cancel.disabled = !native || !ready || saving;
    save.disabled = name.disabled || !choices.length;
  }
  function edit(scene?: Scene) {
    editing = scene?.id ?? null;
    name.value = scene?.name ?? "";
    save.textContent = editing ? "Salva modifiche" : "Crea scena";
    cancel.hidden = !editing;
    error.hidden = true;
    targets(scene?.targets);
  }
  function render() {
    const list = find<HTMLElement>("scene-list");
    list.replaceChildren();
    for (const scene of scenes) {
      const row = document.createElement("div");
      row.className = "zone-entry";
      const text = document.createElement("span");
      text.textContent = `${scene.name} · ${scene.targets.length} luci`;
      const change = document.createElement("button");
      change.type = "button";
      change.className = "secondary-button";
      change.textContent = "Modifica";
      change.disabled = saving || !ready;
      change.addEventListener("click", () => {
        edit(scene);
        name.focus();
      });
      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "secondary-button";
      remove.textContent = "Elimina";
      remove.disabled = saving || !ready;
      remove.addEventListener("click", async () => {
        if (!native || !ready || saving) return;
        const draft = readTargets();
        let cleared = false;
        saving = true;
        targets(draft);
        render();
        error.hidden = true;
        try {
          scenes = await invoke<Scene[]>("delete_scene", { id: scene.id });
          if (editing === scene.id) {
            cleared = true;
            edit();
          }
        } catch (message) {
          report(message);
        } finally {
          saving = false;
          targets(cleared ? [] : draft);
          render();
        }
      });
      row.append(text, change, remove);
      list.append(row);
    }
    find<HTMLElement>("scenes-empty").hidden = !!scenes.length;
  }
  find<HTMLFormElement>("scene-form").addEventListener(
    "submit",
    async (event) => {
      event.preventDefault();
      if (!native || !ready || saving) return;
      const selected = readTargets();
      if (
        !name.value.trim() ||
        !selected.length ||
        selected.some(
          (t) =>
            t.on &&
            (!Number.isInteger(t.brightness) ||
              t.brightness! < 1 ||
              t.brightness! > 100),
        )
      ) {
        report(
          "Scegli un nome, almeno una lampadina e luminosità da 1 a 100%.",
        );
        return;
      }
      saving = true;
      targets(selected);
      render();
      error.hidden = true;
      let saved = false;
      try {
        scenes = await invoke<Scene[]>("save_scene", {
          id: editing,
          name: name.value.trim(),
          targets: selected,
        });
        saved = true;
        edit();
      } catch (message) {
        report(message);
      } finally {
        saving = false;
        targets(saved ? [] : selected);
        render();
      }
    },
  );
  cancel.addEventListener("click", () => edit());
  return async () => {
    if (saving) return;
    ready = false;
    edit();
    render();
    if (!native) return;
    try {
      scenes = await invoke<Scene[]>("list_scenes");
      ready = true;
    } catch (message) {
      report(message);
    }
    targets();
    render();
  };
}
