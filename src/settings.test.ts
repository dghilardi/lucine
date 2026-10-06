import { beforeEach, describe, expect, it, vi } from "vitest";
const native = vi.hoisted(() => ({ value: true }));
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  isTauri: () => native.value,
}));
import { settings } from "./settings";

const demoZone = {
  id: "00000000-0000-4000-8000-000000000001",
  name: "Zona demo",
  deviceIds: ["demo-1"],
};
const flush = async () => {
  for (let i = 0; i < 10; i++) await Promise.resolve();
};
let dialog: HTMLDialogElement;
beforeEach(() => {
  native.value = true;
  invoke.mockReset();
  document.body.innerHTML =
    '<dialog><input id="autostart" type="checkbox"><p id="autostart-error" hidden></p><div id="zone-list"></div><p id="zones-empty"></p><form id="zone-form"><input id="zone-name"><div id="zone-members"></div><p id="zone-error" hidden></p><button id="save-zone"></button><button id="cancel-zone" type="button"></button></form></dialog>';
  dialog = document.querySelector("dialog")!;
  invoke.mockImplementation(async (command: string) =>
    command === "autostart_enabled" ? false : [],
  );
});
const checkbox = () => dialog.querySelector<HTMLInputElement>("#autostart")!;
const start = async () => {
  await settings(dialog, () => [{ id: "demo-1", name: "Lampadina demo" }])();
};

describe("desktop settings", () => {
  it("reads autostart without enabling it", async () => {
    await start();
    expect(checkbox().checked).toBe(false);
    expect(invoke.mock.calls).toEqual([["autostart_enabled"], ["list_zones"]]);
  });
  it("enables autostart only after user interaction and restores it on failure", async () => {
    await start();
    invoke.mockRejectedValue("Permessi insufficienti");
    checkbox().checked = true;
    checkbox().dispatchEvent(new Event("change"));
    await flush();
    expect(invoke).toHaveBeenCalledWith("set_autostart", { enabled: true });
    expect(checkbox().checked).toBe(false);
    expect(checkbox().disabled).toBe(false);
    expect(dialog.querySelector("#autostart-error")!.textContent).toContain(
      "Permessi",
    );
  });
  it("creates a zone with selected dynamic device IDs", async () => {
    await start();
    invoke.mockResolvedValue([demoZone]);
    dialog.querySelector<HTMLInputElement>("#zone-name")!.value = " Zona demo ";
    dialog.querySelector<HTMLInputElement>("#zone-members input")!.checked =
      true;
    dialog
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(invoke).toHaveBeenCalledWith("save_zone", {
      id: null,
      name: "Zona demo",
      deviceIds: ["demo-1"],
    });
    expect(dialog.querySelector("#zone-list")!.textContent).toContain(
      "Zona demo",
    );
  });
  it("does not send an empty zone and renders zone names as text", async () => {
    invoke.mockImplementation(async (command: string) =>
      command === "autostart_enabled"
        ? false
        : [{ ...demoZone, name: "<img src=x>" }],
    );
    await start();
    expect(dialog.querySelector("#zone-list img")).toBeNull();
    dialog.querySelector<HTMLInputElement>("#zone-name")!.value = "Demo";
    dialog
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(
      invoke.mock.calls.every(([command]) => command !== "save_zone"),
    ).toBe(true);
  });
  it("allows zones to load when reading autostart fails", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "autostart_enabled") throw "Unavailable";
      return [demoZone];
    });
    await start();
    expect(checkbox().disabled).toBe(true);
    expect(dialog.querySelector("#zone-list")!.textContent).toContain(
      "Zona demo",
    );
  });
  it("never changes system settings from the browser preview", async () => {
    native.value = false;
    await start();
    checkbox().dispatchEvent(new Event("change"));
    dialog
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(invoke).not.toHaveBeenCalled();
    expect(checkbox().disabled).toBe(true);
  });
  it("preserves selected lamps and the draft name after a failed save", async () => {
    await start();
    invoke.mockRejectedValue("Scrittura non riuscita");
    dialog.querySelector<HTMLInputElement>("#zone-name")!.value = "Zona demo";
    dialog.querySelector<HTMLInputElement>("#zone-members input")!.checked =
      true;
    dialog
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(dialog.querySelector<HTMLInputElement>("#zone-name")!.value).toBe(
      "Zona demo",
    );
    expect(
      dialog.querySelector<HTMLInputElement>("#zone-members input")!.checked,
    ).toBe(true);
    expect(
      dialog.querySelector<HTMLButtonElement>("#save-zone")!.disabled,
    ).toBe(false);
  });
  it("edits an existing zone without creating a second group", async () => {
    invoke.mockImplementation(async (command: string) =>
      command === "autostart_enabled" ? false : [demoZone],
    );
    await start();
    dialog.querySelector<HTMLButtonElement>("#zone-list button")!.click();
    dialog.querySelector<HTMLInputElement>("#zone-name")!.value =
      "Zona rinominata";
    dialog
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { cancelable: true }));
    await flush();
    expect(invoke).toHaveBeenCalledWith("save_zone", {
      id: demoZone.id,
      name: "Zona rinominata",
      deviceIds: ["demo-1"],
    });
  });
  it("preserves a new draft while deleting another zone", async () => {
    invoke.mockImplementation(async (command: string) =>
      command === "autostart_enabled" ? false : [demoZone],
    );
    await start();
    dialog.querySelector<HTMLInputElement>("#zone-name")!.value = "Nuova zona";
    dialog.querySelector<HTMLInputElement>("#zone-members input")!.checked =
      true;
    invoke.mockResolvedValue([]);
    dialog.querySelectorAll<HTMLButtonElement>("#zone-list button")[1].click();
    await flush();
    expect(invoke).toHaveBeenCalledWith("delete_zone", { id: demoZone.id });
    expect(dialog.querySelector<HTMLInputElement>("#zone-name")!.value).toBe(
      "Nuova zona",
    );
    expect(
      dialog.querySelector<HTMLInputElement>("#zone-members input")!.checked,
    ).toBe(true);
    expect(dialog.querySelector("#zones-empty")!.hasAttribute("hidden")).toBe(
      false,
    );
  });
});
