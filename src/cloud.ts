import { invoke, isTauri } from "@tauri-apps/api/core";

export type CloudDevice = { id: string; name: string; roomId: number };
export type CloudRoom = {
  id: number;
  name: string;
  deviceIds: string[];
  otherDevices: number;
  revision: string;
};
export type PowerTarget = { deviceId: string; on: boolean };
export type CloudScene = {
  id: string;
  name: string;
  targets: PowerTarget[];
  editable: boolean;
  reason: string | null;
  revision: string;
};
export type Catalog = {
  rooms: CloudRoom[];
  scenes: CloudScene[];
  devices: CloudDevice[];
};
type Run = (
  command: "run_cloud_room" | "run_cloud_scene",
  args: Record<string, unknown>,
) => Promise<void>;
const empty = (): Catalog => ({ rooms: [], scenes: [], devices: [] });

export function cloudManager(
  settings: HTMLElement,
  shortcuts: HTMLDetailsElement,
  blocked: () => boolean,
  run: Run,
) {
  const native = isTauri();
  settings.innerHTML = `<h3>Condivisi con Android</h3><p class="field-help">Stanze e scene dell’account DreamCatcher Life. Le modifiche vengono salvate nel cloud e sono visibili anche nell’app Android. Salvare non cambia lo stato delle luci.</p><button type="button" id="cloud-refresh" class="secondary-button">Aggiorna dal cloud</button><p id="cloud-status" class="field-help" role="status"></p><p id="cloud-error" class="error" role="alert" hidden></p><details class="cloud-editor"><summary>Stanze Android</summary><div id="cloud-room-list"></div><form id="cloud-room-form"><label for="cloud-room-name">Nome della stanza</label><input id="cloud-room-name" maxlength="80" required><fieldset><legend>Lampadine nella stanza</legend><div id="cloud-room-members"></div></fieldset><p class="field-help">Una lampadina può appartenere a una sola stanza Android. Se la scegli qui, viene spostata dalla stanza precedente.</p><div class="form-actions"><button type="submit" class="primary-button" id="cloud-room-save">Crea stanza su Android</button><button type="button" class="secondary-button" id="cloud-room-cancel" hidden>Annulla modifica</button></div></form></details><details class="cloud-editor"><summary>Scene Android</summary><p class="field-help">Scene manuali con azioni accendi/spegni. Timer, regole e altre azioni si gestiscono nell’app Android.</p><div id="cloud-scene-list"></div><form id="cloud-scene-form"><label for="cloud-scene-name">Nome della scena</label><input id="cloud-scene-name" maxlength="80" required><fieldset><legend>Azioni per lampadina</legend><div id="cloud-scene-targets"></div></fieldset><div class="form-actions"><button type="submit" class="primary-button" id="cloud-scene-save">Crea scena su Android</button><button type="button" class="secondary-button" id="cloud-scene-cancel" hidden>Annulla modifica</button></div></form></details>`;
  shortcuts.innerHTML = `<summary>Stanze e scene Android</summary><p class="field-help">Usa la configurazione condivisa con l’app. Aggiorna e modifica dalle impostazioni.</p><div class="zone-shortcuts"><label for="cloud-room-select">Stanza Android</label><select id="cloud-room-select"></select><div class="form-actions"><button type="button" data-cloud-power="1" class="secondary-button">Accendi stanza</button><button type="button" data-cloud-power="0" class="secondary-button">Spegni stanza</button></div><div class="zone-adjust"><label>Luminosità %<input id="cloud-brightness" type="number" min="1" max="100" step="1" value="50"></label><button type="button" id="cloud-apply-brightness" class="secondary-button">Applica luminosità</button><label>Bianco<select id="cloud-white"><option value="160">Caldo</option><option value="162">Freddo</option></select></label><button type="button" id="cloud-apply-white" class="secondary-button">Applica bianco</button></div><p class="field-help">Luminosità e bianco si applicano solo alle luci accese.</p></div><div id="cloud-scene-actions" class="form-actions" aria-label="Scene Android"></div><p id="cloud-action-error" class="error" role="alert" hidden></p>`;
  const find = <T extends HTMLElement>(id: string) =>
    settings.querySelector<T>(`#${id}`)!;
  const roomName = find<HTMLInputElement>("cloud-room-name");
  const sceneName = find<HTMLInputElement>("cloud-scene-name");
  const roomSelect =
    shortcuts.querySelector<HTMLSelectElement>("#cloud-room-select")!;
  const error = find<HTMLElement>("cloud-error");
  let catalog = empty();
  let ready = false;
  let loading = false;
  let saving = false;
  let room: CloudRoom | null = null;
  let scene: CloudScene | null = null;
  let armed: string | null = null;
  const unavailable = () => !native || !ready || loading || saving || blocked();
  const report = (message: unknown) => {
    error.textContent = String(message);
    error.hidden = false;
  };
  const button = (
    label: string,
    action: () => void,
    disabled = unavailable(),
  ) => {
    const element = document.createElement("button");
    element.type = "button";
    element.className = "secondary-button";
    element.textContent = label;
    element.disabled = disabled;
    element.addEventListener("click", action);
    return element;
  };
  function checked(container: string): string[] {
    return Array.from(
      find<HTMLElement>(container).querySelectorAll<HTMLInputElement>(
        "input:checked",
      ),
    ).map((input) => input.value);
  }
  function targets(): PowerTarget[] {
    return checked("cloud-scene-targets").map((id) => ({
      deviceId: id,
      on:
        Array.from(
          find<HTMLElement>(
            "cloud-scene-targets",
          ).querySelectorAll<HTMLSelectElement>("select"),
        ).find((select) => select.dataset.id === id)!.value === "on",
    }));
  }
  function members(
    container: string,
    selected: string[],
    values?: PowerTarget[],
  ) {
    const root = find<HTMLElement>(container);
    root.replaceChildren();
    for (const device of catalog.devices) {
      const label = document.createElement("label");
      label.className = "check-row";
      const include = document.createElement("input");
      include.type = "checkbox";
      include.value = device.id;
      include.checked = selected.includes(device.id);
      const text = document.createElement("span");
      text.textContent = device.name;
      label.append(include, text);
      if (values) {
        const row = document.createElement("div");
        row.className = "cloud-target";
        const powerLabel = document.createElement("label");
        powerLabel.textContent = `Stato di ${device.name}`;
        const select = document.createElement("select");
        select.dataset.id = device.id;
        for (const [value, name] of [
          ["on", "Accesa"],
          ["off", "Spenta"],
        ]) {
          const option = document.createElement("option");
          option.value = value;
          option.textContent = name;
          select.append(option);
        }
        select.value =
          values.find((t) => t.deviceId === device.id)?.on === false
            ? "off"
            : "on";
        powerLabel.append(select);
        row.append(label, powerLabel);
        root.append(row);
        include.addEventListener("change", sync);
      } else root.append(label);
    }
  }
  function editRoom(value: CloudRoom | null = null) {
    room = value;
    roomName.value = value?.name ?? "";
    find<HTMLElement>("cloud-room-save").textContent = value
      ? "Salva stanza su Android"
      : "Crea stanza su Android";
    find<HTMLElement>("cloud-room-cancel").hidden = !value;
    members("cloud-room-members", value?.deviceIds ?? []);
    sync();
  }
  function editScene(value: CloudScene | null = null) {
    scene = value;
    sceneName.value = value?.name ?? "";
    find<HTMLElement>("cloud-scene-save").textContent = value
      ? "Salva scena su Android"
      : "Crea scena su Android";
    find<HTMLElement>("cloud-scene-cancel").hidden = !value;
    members(
      "cloud-scene-targets",
      value?.targets.map((t) => t.deviceId) ?? [],
      value?.targets ?? [],
    );
    sync();
  }
  function accept(value: Catalog) {
    if (
      !value ||
      !Array.isArray(value.rooms) ||
      !Array.isArray(value.scenes) ||
      !Array.isArray(value.devices)
    )
      throw new Error("Configurazione cloud non valida. Aggiorna e riprova.");
    catalog = value;
    ready = true;
    armed = null;
  }
  async function mutate(command: string, args: Record<string, unknown>) {
    if (unavailable()) return;
    saving = true;
    error.hidden = true;
    sync();
    lists();
    try {
      accept(await invoke<Catalog>(command, args));
      if (
        command === "save_cloud_room" ||
        (command === "delete_cloud_room" && room?.id === args.id)
      )
        editRoom();
      if (
        command === "save_cloud_scene" ||
        (command === "delete_cloud_scene" && scene?.id === args.id)
      )
        editScene();
      renderActions();
    } catch (message) {
      ready = false;
      report(message);
    } finally {
      saving = false;
      lists();
      sync();
    }
  }
  function deleteButtons(
    row: HTMLElement,
    kind: "room" | "scene",
    id: number | string,
    revision: string,
    name: string,
  ) {
    const key = `${kind}:${id}`;
    row.dataset.cloudKey = key;
    const focusAction = (label: string) => {
      const root = find<HTMLElement>(`cloud-${kind}-list`);
      const replacement = Array.from(root.children).find(
        (entry) => (entry as HTMLElement).dataset.cloudKey === key,
      );
      Array.from(replacement?.querySelectorAll("button") ?? [])
        .find((action) => action.textContent === label)
        ?.focus();
    };
    if (armed === key) {
      const text = document.createElement("span");
      text.className = "field-help";
      text.textContent = `Eliminare “${name}” anche da Android?`;
      row.append(
        text,
        button("Conferma eliminazione", async () => {
          await mutate(`delete_cloud_${kind}`, { id, revision });
          if (
            !document.activeElement ||
            document.activeElement === document.body
          )
            (kind === "room" ? roomName : sceneName).focus();
        }),
        button("Annulla", () => {
          armed = null;
          lists();
          focusAction("Elimina");
        }),
      );
    } else
      row.append(
        button("Elimina", () => {
          armed = key;
          lists();
          focusAction("Conferma eliminazione");
        }),
      );
  }
  function lists() {
    const rooms = find<HTMLElement>("cloud-room-list");
    rooms.replaceChildren();
    for (const value of catalog.rooms) {
      const row = document.createElement("div");
      row.className = "zone-entry";
      const text = document.createElement("span");
      text.textContent = `${value.name} · ${value.deviceIds.length} luci${value.otherDevices ? ` · ${value.otherDevices} altri dispositivi (non spostati)` : ""}`;
      row.append(
        text,
        button("Modifica", () => {
          editRoom(value);
          roomName.focus();
        }),
      );
      if (!value.otherDevices)
        deleteButtons(row, "room", value.id, value.revision, value.name);
      rooms.append(row);
    }
    const scenes = find<HTMLElement>("cloud-scene-list");
    scenes.replaceChildren();
    for (const value of catalog.scenes) {
      const row = document.createElement("div");
      row.className = "zone-entry";
      const text = document.createElement("span");
      text.textContent = value.editable
        ? `${value.name} · ${value.targets.length} luci`
        : `${value.name} · ${value.reason}`;
      row.append(text);
      if (value.editable) {
        row.append(
          button("Modifica", () => {
            editScene(value);
            sceneName.focus();
          }),
        );
        deleteButtons(row, "scene", value.id, value.revision, value.name);
      }
      scenes.append(row);
    }
    for (const [root, count, message] of [
      [rooms, catalog.rooms.length, "Nessuna stanza Android."],
      [scenes, catalog.scenes.length, "Nessuna scena Android."],
    ] as const) {
      if (!count) {
        const text = document.createElement("p");
        text.className = "field-help";
        text.textContent = message;
        root.append(text);
      }
    }
  }
  function renderActions() {
    const previous = roomSelect.value;
    roomSelect.replaceChildren();
    for (const room of catalog.rooms) {
      const option = document.createElement("option");
      option.value = String(room.id);
      option.textContent = `${room.name} · ${room.deviceIds.length} luci`;
      roomSelect.append(option);
    }
    if (catalog.rooms.some((r) => String(r.id) === previous))
      roomSelect.value = previous;
    if (!catalog.rooms.length) {
      const option = document.createElement("option");
      option.textContent = "Nessuna stanza Android";
      roomSelect.append(option);
    }
    const scenes = shortcuts.querySelector<HTMLElement>(
      "#cloud-scene-actions",
    )!;
    scenes.replaceChildren();
    for (const scene of catalog.scenes) {
      if (scene.editable)
        scenes.append(
          button(`Attiva ${scene.name}`, () => {
            if (!unavailable()) void run("run_cloud_scene", { id: scene.id });
          }),
        );
      else {
        const text = document.createElement("p");
        text.className = "field-help";
        text.textContent = `${scene.name}: ${scene.reason}`;
        scenes.append(text);
      }
    }
    if (!catalog.scenes.length) {
      const text = document.createElement("p");
      text.className = "field-help";
      text.textContent = "Nessuna scena Android.";
      scenes.append(text);
    }
  }
  function sync() {
    for (const input of Array.from(
      settings.querySelectorAll<
        HTMLInputElement | HTMLSelectElement | HTMLButtonElement
      >("form input, form select, form button"),
    ))
      input.disabled = unavailable();
    for (const select of Array.from(
      find<HTMLElement>(
        "cloud-scene-targets",
      ).querySelectorAll<HTMLSelectElement>("select"),
    ))
      select.disabled =
        unavailable() ||
        !checked("cloud-scene-targets").includes(select.dataset.id!);
    for (const button of Array.from(
      settings.querySelectorAll<HTMLButtonElement>(
        "#cloud-room-list button, #cloud-scene-list button",
      ),
    ))
      button.disabled = unavailable();
    find<HTMLButtonElement>("cloud-scene-save").disabled =
      unavailable() || !catalog.devices.length;
    find<HTMLButtonElement>("cloud-refresh").disabled =
      !native || loading || saving || blocked();
    for (const input of Array.from(
      shortcuts.querySelectorAll<
        HTMLInputElement | HTMLSelectElement | HTMLButtonElement
      >("input, select, button"),
    ))
      input.disabled = unavailable();
    const selected = catalog.rooms.find(
      (r) => String(r.id) === roomSelect.value,
    );
    for (const button of Array.from(
      shortcuts.querySelectorAll<HTMLButtonElement>(".zone-shortcuts button"),
    ))
      button.disabled = unavailable() || !selected?.deviceIds.length;
    find<HTMLElement>("cloud-status").textContent = !native
      ? "Anteprima: i controlli cloud sono disabilitati."
      : loading
        ? "Lettura da Android…"
        : saving
          ? "Salvataggio nel cloud…"
          : ready
            ? `${catalog.rooms.length} stanze e ${catalog.scenes.length} scene dal cloud.`
            : "Aggiorna dal cloud per usare stanze e scene.";
  }
  async function load(reset = false) {
    if (loading || saving || blocked()) return;
    loading = true;
    ready = false;
    error.hidden = true;
    sync();
    lists();
    try {
      accept(native ? await invoke<Catalog>("cloud_catalog") : empty());
      if (
        reset ||
        (!room && !roomName.value && !checked("cloud-room-members").length)
      )
        editRoom();
      if (
        reset ||
        (!scene && !sceneName.value && !checked("cloud-scene-targets").length)
      )
        editScene();
      renderActions();
    } catch (message) {
      catalog = empty();
      report(message);
      renderActions();
    } finally {
      loading = false;
      lists();
      sync();
    }
  }
  find<HTMLButtonElement>("cloud-refresh").addEventListener(
    "click",
    () => void load(true),
  );
  find<HTMLButtonElement>("cloud-room-cancel").addEventListener("click", () =>
    editRoom(),
  );
  find<HTMLButtonElement>("cloud-scene-cancel").addEventListener("click", () =>
    editScene(),
  );
  find<HTMLFormElement>("cloud-room-form").addEventListener(
    "submit",
    (event) => {
      event.preventDefault();
      if (!roomName.value.trim()) return;
      void mutate("save_cloud_room", {
        draft: {
          id: room?.id ?? null,
          revision: room?.revision ?? null,
          name: roomName.value.trim(),
          deviceIds: checked("cloud-room-members"),
        },
      });
    },
  );
  find<HTMLFormElement>("cloud-scene-form").addEventListener(
    "submit",
    (event) => {
      event.preventDefault();
      if (unavailable()) return;
      const values = targets();
      if (!sceneName.value.trim() || !values.length) {
        report("Scegli un nome e almeno una lampadina per la scena Android.");
        return;
      }
      void mutate("save_cloud_scene", {
        draft: {
          id: scene?.id ?? null,
          revision: scene?.revision ?? null,
          name: sceneName.value.trim(),
          targets: values,
          timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        },
      });
    },
  );
  roomSelect.addEventListener("change", sync);
  function action(kind: string, value: number) {
    const room = catalog.rooms.find((r) => String(r.id) === roomSelect.value);
    if (unavailable() || !room?.deviceIds.length) return;
    const error = shortcuts.querySelector<HTMLElement>("#cloud-action-error")!;
    if (
      !Number.isInteger(value) ||
      (kind === "brightness" && (value < 1 || value > 100))
    ) {
      error.textContent = "Scegli una luminosità da 1 a 100%.";
      error.hidden = false;
      return;
    }
    error.hidden = true;
    void run("run_cloud_room", { id: room.id, kind, value });
  }
  for (const button of Array.from(
    shortcuts.querySelectorAll<HTMLButtonElement>("[data-cloud-power]"),
  ))
    button.addEventListener("click", () =>
      action("power", Number(button.dataset.cloudPower)),
    );
  shortcuts
    .querySelector("#cloud-apply-brightness")!
    .addEventListener("click", () =>
      action(
        "brightness",
        Number(
          shortcuts.querySelector<HTMLInputElement>("#cloud-brightness")!.value,
        ),
      ),
    );
  shortcuts
    .querySelector("#cloud-apply-white")!
    .addEventListener("click", () =>
      action(
        "white",
        Number(
          shortcuts.querySelector<HTMLSelectElement>("#cloud-white")!.value,
        ),
      ),
    );
  editRoom();
  editScene();
  lists();
  renderActions();
  sync();
  return {
    load,
    sync,
    invalidate: () => {
      ready = false;
      catalog = empty();
      lists();
      renderActions();
      sync();
    },
  };
}
