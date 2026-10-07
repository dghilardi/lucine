import { beforeEach, describe, expect, it, vi } from "vitest";
const native = vi.hoisted(() => ({ value: true }));
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  isTauri: () => native.value,
}));
import { cloudManager, type Catalog } from "./cloud";

const fixture = (): Catalog => ({
  rooms: [
    {
      id: 7,
      name: "Stanza inventata",
      deviceIds: ["fixture-1"],
      otherDevices: 0,
      revision: "room-revision",
    },
  ],
  scenes: [
    {
      id: "fixture-scene",
      name: "Scena inventata",
      targets: [{ deviceId: "fixture-1", on: false }],
      editable: true,
      reason: null,
      revision: "scene-revision",
    },
  ],
  devices: [
    { id: "fixture-1", name: "Lampadina inventata", roomId: 7 },
    { id: "fixture-2", name: "Altra lampadina inventata", roomId: 0 },
  ],
});
const flush = async () => {
  for (let i = 0; i < 12; i++) await Promise.resolve();
};
let settings: HTMLElement;
let shortcuts: HTMLDetailsElement;
let blocked: boolean;
const run = vi.fn(
  async (
    _command: "run_cloud_room" | "run_cloud_scene",
    _args: Record<string, unknown>,
  ): Promise<void> => {},
);
const button = (root: HTMLElement, text: string) =>
  Array.from(root.querySelectorAll<HTMLButtonElement>("button")).find(
    (b) => b.textContent === text,
  )!;
const submit = (id: string) =>
  settings
    .querySelector<HTMLFormElement>(`#${id}`)!
    .dispatchEvent(new Event("submit", { cancelable: true }));
beforeEach(() => {
  native.value = true;
  blocked = false;
  invoke.mockReset().mockResolvedValue(fixture());
  run.mockReset().mockResolvedValue(undefined);
  document.body.innerHTML =
    '<section id="settings"></section><details id="shortcuts"></details>';
  settings = document.querySelector("#settings")!;
  shortcuts = document.querySelector("#shortcuts")!;
});
describe("Android cloud integration", () => {
  it("reads dynamic metadata as text without saving or changing bulbs", async () => {
    const values = fixture();
    values.rooms[0].name = "<img src=x onerror=alert(1)>";
    values.devices[0].name = "<script>bad()</script>";
    invoke.mockResolvedValue(values);
    await cloudManager(settings, shortcuts, () => blocked, run).load();
    expect(invoke.mock.calls).toEqual([["cloud_catalog"]]);
    expect(run).not.toHaveBeenCalled();
    expect(settings.querySelector("#cloud-room-list")!.textContent).toContain(
      "<img",
    );
    expect(settings.querySelector("img,script")).toBeNull();
  });
  it("creates a cloud room only on submit and forwards selected account IDs", async () => {
    await cloudManager(settings, shortcuts, () => blocked, run).load();
    settings.querySelector<HTMLInputElement>("#cloud-room-name")!.value =
      " Nuova stanza inventata ";
    settings.querySelector<HTMLInputElement>(
      "#cloud-room-members input",
    )!.checked = true;
    submit("cloud-room-form");
    await flush();
    expect(invoke).toHaveBeenCalledWith("save_cloud_room", {
      draft: {
        id: null,
        revision: null,
        name: "Nuova stanza inventata",
        deviceIds: ["fixture-1"],
      },
    });
    expect(run).not.toHaveBeenCalled();
  });
  it("keeps the original revision and draft after a rejected scene edit until explicit refresh", async () => {
    const controls = cloudManager(settings, shortcuts, () => blocked, run);
    await controls.load();
    button(settings.querySelector("#cloud-scene-list")!, "Modifica").click();
    settings.querySelector<HTMLInputElement>("#cloud-scene-name")!.value =
      "Nome modificato";
    invoke.mockRejectedValue("La scena è cambiata nell’app Android.");
    submit("cloud-scene-form");
    await flush();
    expect(invoke).toHaveBeenCalledWith("save_cloud_scene", {
      draft: {
        id: "fixture-scene",
        revision: "scene-revision",
        name: "Nome modificato",
        targets: [{ deviceId: "fixture-1", on: false }],
        timezone: expect.any(String),
      },
    });
    expect(
      settings.querySelector<HTMLInputElement>("#cloud-scene-name")!.value,
    ).toBe("Nome modificato");
    expect(
      settings.querySelector<HTMLButtonElement>("#cloud-scene-save")!.disabled,
    ).toBe(true);
    expect(button(settings, "Aggiorna dal cloud").disabled).toBe(false);
    expect(run).not.toHaveBeenCalled();
  });
  it("requires a second explicit click before deleting shared metadata", async () => {
    await cloudManager(settings, shortcuts, () => blocked, run).load();
    const list = settings.querySelector<HTMLElement>("#cloud-room-list")!;
    button(list, "Elimina").click();
    expect(invoke.mock.calls).toEqual([["cloud_catalog"]]);
    expect(list.textContent).toContain("anche da Android");
    button(list, "Conferma eliminazione").click();
    await flush();
    expect(invoke).toHaveBeenCalledWith("delete_cloud_room", {
      id: 7,
      revision: "room-revision",
    });
    expect(run).not.toHaveBeenCalled();
  });
  it("keeps unrelated scene and room drafts after successful writes", async () => {
    await cloudManager(settings, shortcuts, () => blocked, run).load();
    button(settings.querySelector("#cloud-scene-list")!, "Modifica").click();
    const sceneName =
      settings.querySelector<HTMLInputElement>("#cloud-scene-name")!;
    sceneName.value = "Scena in lavorazione";
    const roomName =
      settings.querySelector<HTMLInputElement>("#cloud-room-name")!;
    roomName.value = "Stanza nuova";
    submit("cloud-room-form");
    await flush();
    expect(roomName.value).toBe("");
    expect(sceneName.value).toBe("Scena in lavorazione");
    roomName.value = "Stanza in lavorazione";
    submit("cloud-scene-form");
    await flush();
    expect(sceneName.value).toBe("");
    expect(roomName.value).toBe("Stanza in lavorazione");
    expect(invoke).toHaveBeenLastCalledWith("save_cloud_scene", {
      draft: {
        id: "fixture-scene",
        revision: "scene-revision",
        name: "Scena in lavorazione",
        targets: [{ deviceId: "fixture-1", on: false }],
        timezone: expect.any(String),
      },
    });
    const list = settings.querySelector<HTMLElement>("#cloud-scene-list")!;
    button(list, "Elimina").click();
    button(list, "Conferma eliminazione").click();
    await flush();
    expect(roomName.value).toBe("Stanza in lavorazione");
  });
  it("preserves new drafts during periodic cloud refresh", async () => {
    const controls = cloudManager(settings, shortcuts, () => blocked, run);
    await controls.load();
    const roomName =
      settings.querySelector<HTMLInputElement>("#cloud-room-name")!;
    const sceneName =
      settings.querySelector<HTMLInputElement>("#cloud-scene-name")!;
    roomName.value = "Bozza nuova stanza";
    sceneName.value = "Bozza nuova scena";
    const member = settings.querySelector<HTMLInputElement>(
      "#cloud-room-members input",
    )!;
    member.checked = true;
    await controls.load();
    expect(roomName.value).toBe("Bozza nuova stanza");
    expect(sceneName.value).toBe("Bozza nuova scena");
    expect(member.checked).toBe(true);
    await controls.load(true);
    expect(roomName.value).toBe("");
    expect(sceneName.value).toBe("");
  });
  it("moves keyboard focus through delete confirmation, cancellation and completion", async () => {
    await cloudManager(settings, shortcuts, () => blocked, run).load();
    const list = settings.querySelector<HTMLElement>("#cloud-room-list")!;
    button(list, "Elimina").focus();
    button(list, "Elimina").click();
    expect(document.activeElement).toBe(button(list, "Conferma eliminazione"));
    button(list, "Annulla").click();
    expect(document.activeElement).toBe(button(list, "Elimina"));
    button(list, "Elimina").click();
    const removed = fixture();
    removed.rooms = [];
    invoke.mockResolvedValue(removed);
    button(list, "Conferma eliminazione").click();
    await flush();
    expect(document.activeElement).toBe(
      settings.querySelector("#cloud-room-name"),
    );
  });
  it("keeps unsupported scenes visible without edit, delete or activation controls", async () => {
    const values = fixture();
    values.scenes[0].editable = false;
    values.scenes[0].reason = "Automazione non supportata";
    values.rooms[0].otherDevices = 2;
    invoke.mockResolvedValue(values);
    await cloudManager(settings, shortcuts, () => blocked, run).load();
    expect(settings.querySelector("#cloud-scene-list button")).toBeNull();
    expect(shortcuts.querySelector("#cloud-scene-actions button")).toBeNull();
    expect(shortcuts.textContent).toContain("Automazione non supportata");
    expect(
      button(settings.querySelector("#cloud-room-list")!, "Elimina"),
    ).toBeUndefined();
  });
  it("runs only explicit room/scene actions and blocks them during other work", async () => {
    const controls = cloudManager(settings, shortcuts, () => blocked, run);
    await controls.load();
    button(shortcuts, "Spegni stanza").click();
    expect(run).toHaveBeenCalledWith("run_cloud_room", {
      id: 7,
      kind: "power",
      value: 0,
    });
    blocked = true;
    controls.sync();
    button(shortcuts, "Attiva Scena inventata").click();
    expect(run).toHaveBeenCalledTimes(1);
    blocked = false;
    controls.sync();
    button(shortcuts, "Attiva Scena inventata").click();
    expect(run).toHaveBeenCalledWith("run_cloud_scene", {
      id: "fixture-scene",
    });
  });
  it("never uses cloud credentials or mutations in browser preview", async () => {
    native.value = false;
    await cloudManager(settings, shortcuts, () => blocked, run).load();
    submit("cloud-room-form");
    submit("cloud-scene-form");
    button(settings, "Aggiorna dal cloud").click();
    expect(invoke).not.toHaveBeenCalled();
    expect(run).not.toHaveBeenCalled();
    expect(settings.textContent).toContain("Anteprima");
  });
});
