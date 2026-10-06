import { invoke, isTauri } from "@tauri-apps/api/core";

export type Zone = { id: string; name: string; deviceIds: string[] };
type LampChoice = { id: string; name: string };

export function settings(dialog: HTMLDialogElement, lamps: () => LampChoice[]) {
  const native = isTauri();
  const find = <T extends HTMLElement>(id: string) =>
    dialog.querySelector<T>(`#${id}`)!;
  const autostart = find<HTMLInputElement>("autostart");
  const autoError = find<HTMLElement>("autostart-error");
  const zoneError = find<HTMLElement>("zone-error");
  const form = find<HTMLFormElement>("zone-form");
  const name = find<HTMLInputElement>("zone-name");
  const submit = find<HTMLButtonElement>("save-zone");
  let zones: Zone[] = [];
  let editing: string | null = null;
  let enabled = false;
  let ready = false;
  let saving = false;

  function report(element: HTMLElement, error: unknown) {
    element.hidden = false;
    element.textContent = String(error);
  }
  function members(ids: string[] = []) {
    const container = find<HTMLElement>("zone-members");
    container.replaceChildren();
    const choices = [...lamps()];
    for (const id of ids)
      if (!choices.some((lamp) => lamp.id === id))
        choices.push({ id, name: "Lampadina non disponibile" });
    for (const lamp of choices) {
      const label = document.createElement("label");
      label.className = "check-row";
      const input = document.createElement("input");
      input.type = "checkbox";
      input.value = lamp.id;
      input.checked = ids.includes(lamp.id);
      input.disabled = !native || !ready || saving;
      const text = document.createElement("span");
      text.textContent = lamp.name;
      label.append(input, text);
      container.append(label);
    }
    name.disabled = !native || !ready || saving;
    submit.disabled = !native || !ready || saving || !choices.length;
  }
  function selectedIds() {
    return Array.from(
      find<HTMLElement>("zone-members").querySelectorAll<HTMLInputElement>(
        "input:checked",
      ),
    ).map((input) => input.value);
  }
  function edit(zone?: Zone) {
    editing = zone?.id ?? null;
    name.value = zone?.name ?? "";
    submit.textContent = editing ? "Salva modifiche" : "Crea zona";
    find<HTMLButtonElement>("cancel-zone").hidden = !editing;
    members(zone?.deviceIds);
    zoneError.hidden = true;
  }
  function render() {
    find<HTMLButtonElement>("cancel-zone").disabled = saving;
    const container = find<HTMLElement>("zone-list");
    container.replaceChildren();
    for (const zone of zones) {
      const row = document.createElement("div");
      row.className = "zone-entry";
      const title = document.createElement("span");
      title.textContent = `${zone.name} · ${zone.deviceIds.length} luci`;
      const change = document.createElement("button");
      change.type = "button";
      change.className = "secondary-button";
      change.textContent = "Modifica";
      change.disabled = saving;
      change.addEventListener("click", () => {
        edit(zone);
        name.focus();
      });
      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "secondary-button";
      remove.textContent = "Elimina";
      remove.disabled = saving;
      remove.addEventListener("click", async () => {
        if (saving || !native || !ready) return;
        const draft = selectedIds();
        let deletedEditing = false;
        saving = true;
        render();
        members(draft);
        try {
          zones = await invoke<Zone[]>("delete_zone", { id: zone.id });
          if (editing === zone.id) {
            deletedEditing = true;
            edit();
          }
        } catch (error) {
          report(zoneError, error);
        } finally {
          saving = false;
          render();
          members(deletedEditing ? [] : draft);
        }
      });
      row.append(title, change, remove);
      container.append(row);
    }
    find<HTMLElement>("zones-empty").hidden = zones.length > 0;
  }
  autostart.addEventListener("change", async () => {
    if (!native || autostart.disabled) return;
    const previous = enabled;
    const desired = autostart.checked;
    autostart.disabled = true;
    autoError.hidden = true;
    try {
      enabled = await invoke<boolean>("set_autostart", { enabled: desired });
    } catch (error) {
      enabled = previous;
      report(autoError, error);
    } finally {
      autostart.checked = enabled;
      autostart.disabled = false;
    }
  });
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!native || !ready || saving) return;
    const deviceIds = selectedIds();
    if (!name.value.trim() || !deviceIds.length) {
      report(zoneError, "Scegli un nome e almeno una lampadina.");
      return;
    }
    saving = true;
    members(deviceIds);
    render();
    zoneError.hidden = true;
    let saved = false;
    try {
      zones = await invoke<Zone[]>("save_zone", {
        id: editing,
        name: name.value.trim(),
        deviceIds,
      });
      saved = true;
      edit();
    } catch (error) {
      report(zoneError, error);
    } finally {
      saving = false;
      render();
      members(saved ? [] : deviceIds);
    }
  });
  find<HTMLButtonElement>("cancel-zone").addEventListener("click", () =>
    edit(),
  );

  return async function load() {
    autostart.disabled = true;
    ready = false;
    autoError.hidden = true;
    zoneError.hidden = true;
    edit();
    if (!native) {
      render();
      return;
    }
    const results = await Promise.allSettled([
      invoke<boolean>("autostart_enabled"),
      invoke<Zone[]>("list_zones"),
    ]);
    if (results[0].status === "fulfilled") {
      enabled = results[0].value;
      autostart.checked = enabled;
      autostart.disabled = false;
    } else report(autoError, results[0].reason);
    if (results[1].status === "fulfilled") {
      zones = results[1].value;
      ready = true;
    } else report(zoneError, results[1].reason);
    render();
    members();
  };
}
