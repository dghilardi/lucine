import { beforeEach, describe, expect, it, vi } from "vitest";
const native = vi.hoisted(() => ({ value: true }));
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  isTauri: () => native.value,
}));
import { scenesEditor, type Scene } from "./scenes";
let root: HTMLElement;
const demoScene: Scene = {
  id: "00000000-0000-4000-8000-000000000001",
  name: "Scena demo",
  targets: [{ deviceId: "demo-1", on: true, brightness: 30, white: 161 }],
};
const lamps = () => [
  {
    id: "demo-1",
    name: "Lampadina demo 1",
    state: { on: true, brightness: 50, mode: 160 },
  },
  { id: "demo-2", name: "Lampadina demo 2", state: null },
];
const flush = async () => {
  for (let i = 0; i < 10; i++) await Promise.resolve();
};
const name = () => root.querySelector<HTMLInputElement>("#scene-name")!;
const row = (index = 0) =>
  root.querySelectorAll<HTMLElement>(".scene-target")[index];
function include(index = 0) {
  row(index).querySelector<HTMLInputElement>(".include-target")!.click();
}
function submit() {
  root
    .querySelector("form")!
    .dispatchEvent(new Event("submit", { cancelable: true }));
}
beforeEach(() => {
  native.value = true;
  invoke.mockReset().mockResolvedValue([]);
  document.body.innerHTML = "<section></section>";
  root = document.querySelector("section")!;
});
describe("local scenes", () => {
  it("saves distinct per-bulb targets without running commands", async () => {
    await scenesEditor(root, lamps)();
    name().value = " Scena demo ";
    include(0);
    include(1);
    row(0).querySelector<HTMLInputElement>(".target-brightness")!.value = "30";
    row(0).querySelector<HTMLSelectElement>(".target-white")!.value = "161";
    const power = row(1).querySelector<HTMLSelectElement>(".target-power")!;
    power.value = "off";
    power.dispatchEvent(new Event("change"));
    expect(
      row(1).querySelector<HTMLInputElement>(".target-brightness")!.disabled,
    ).toBe(true);
    invoke.mockResolvedValue([demoScene]);
    submit();
    await flush();
    expect(invoke.mock.calls).toEqual([
      ["list_scenes"],
      [
        "save_scene",
        {
          id: null,
          name: "Scena demo",
          targets: [
            demoScene.targets[0],
            { deviceId: "demo-2", on: false, brightness: null, white: null },
          ],
        },
      ],
    ]);
  });
  it("preserves orphaned members when editing and renders names as text", async () => {
    invoke.mockResolvedValue([
      {
        ...demoScene,
        name: "<img src=x>",
        targets: [{ ...demoScene.targets[0], deviceId: "demo-removed" }],
      },
    ]);
    await scenesEditor(root, lamps)();
    expect(root.querySelector("img")).toBeNull();
    root.querySelector<HTMLButtonElement>("#scene-list button")!.click();
    expect(root.textContent).toContain("Lampadina non disponibile");
    name().value = "Scena modificata";
    submit();
    await flush();
    expect(invoke).toHaveBeenCalledWith("save_scene", {
      id: demoScene.id,
      name: "Scena modificata",
      targets: [{ ...demoScene.targets[0], deviceId: "demo-removed" }],
    });
  });
  it("rejects empty selections and out-of-range brightness before saving", async () => {
    await scenesEditor(root, lamps)();
    name().value = "Demo";
    submit();
    await flush();
    include();
    row().querySelector<HTMLInputElement>(".target-brightness")!.value = "101";
    submit();
    await flush();
    expect(invoke.mock.calls).toEqual([["list_scenes"]]);
    expect(root.querySelector("#scene-error")!.textContent).toContain("100%");
  });
  it("preserves the scene draft on failure and disables changes while saving", async () => {
    await scenesEditor(root, lamps)();
    name().value = "Demo";
    include();
    let reject!: (reason: string) => void;
    invoke.mockImplementation(
      () =>
        new Promise((_, fail) => {
          reject = fail;
        }),
    );
    submit();
    expect(name().disabled).toBe(true);
    reject("Scrittura non riuscita");
    await flush();
    expect(name().value).toBe("Demo");
    expect(name().disabled).toBe(false);
    expect(
      row().querySelector<HTMLInputElement>(".include-target")!.checked,
    ).toBe(true);
  });
  it("deletes a saved scene without losing a separate new draft", async () => {
    invoke.mockResolvedValue([demoScene]);
    await scenesEditor(root, lamps)();
    name().value = "Nuova scena";
    include();
    invoke.mockResolvedValue([]);
    root.querySelectorAll<HTMLButtonElement>("#scene-list button")[1].click();
    await flush();
    expect(invoke).toHaveBeenCalledWith("delete_scene", { id: demoScene.id });
    expect(name().value).toBe("Nuova scena");
    expect(
      row().querySelector<HTMLInputElement>(".include-target")!.checked,
    ).toBe(true);
  });
  it("never reads or writes native state from preview", async () => {
    native.value = false;
    await scenesEditor(root, lamps)();
    submit();
    await flush();
    expect(invoke).not.toHaveBeenCalled();
    expect(name().disabled).toBe(true);
  });
});
