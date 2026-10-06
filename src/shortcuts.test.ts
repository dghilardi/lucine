import { beforeEach, describe, expect, it, vi } from "vitest";
const native = vi.hoisted(() => ({ value: true }));
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  isTauri: () => native.value,
}));
import { shortcuts } from "./shortcuts";
let root: HTMLDetailsElement;
const run = vi.fn();
const flush = async () => {
  for (let i = 0; i < 10; i++) await Promise.resolve();
};
async function start(blocked = () => false) {
  const panel = shortcuts(root, blocked, run);
  root.open = true;
  root.dispatchEvent(new Event("toggle"));
  await flush();
  return panel;
}
beforeEach(() => {
  native.value = true;
  invoke.mockReset();
  run.mockReset().mockResolvedValue(undefined);
  invoke.mockImplementation(async (cmd: string) =>
    cmd === "list_zones"
      ? [{ id: "demo-zone", name: "Zona demo", deviceIds: ["demo-1"] }]
      : [{ id: "demo-scene", name: "Scena demo", targets: [] }],
  );
  document.body.innerHTML = "<details></details>";
  root = document.querySelector("details")!;
});
describe("zone and scene shortcuts", () => {
  it("loads saved groups without mutation and runs only the selected zone", async () => {
    await start();
    expect(run).not.toHaveBeenCalled();
    root.querySelector<HTMLInputElement>("#zone-brightness")!.value = "37";
    root.querySelector<HTMLButtonElement>("#apply-zone-brightness")!.click();
    expect(run).toHaveBeenCalledWith("run_zone", {
      id: "demo-zone",
      kind: "brightness",
      value: 37,
    });
  });
  it("invokes scenes by saved ID and blocks all shortcuts while another action is pending", async () => {
    let blocked = false;
    const panel = await start(() => blocked);
    root.querySelector<HTMLButtonElement>("#scene-shortcuts button")!.click();
    expect(run).toHaveBeenCalledWith("run_scene", { id: "demo-scene" });
    run.mockClear();
    blocked = true;
    panel.sync();
    root.querySelector<HTMLButtonElement>("[data-power]")!.click();
    expect(run).not.toHaveBeenCalled();
    expect(
      root.querySelector<HTMLSelectElement>("#shortcut-zone")!.disabled,
    ).toBe(true);
  });
  it("keeps scene shortcuts available when zones fail to load", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_zones") throw "Zone non disponibili";
      return [{ id: "demo-scene", name: "<img src=x>", targets: [] }];
    });
    await start();
    expect(root.querySelector("img")).toBeNull();
    expect(
      root.querySelector<HTMLButtonElement>("#scene-shortcuts button")!
        .disabled,
    ).toBe(false);
    expect(
      root.querySelector<HTMLButtonElement>("[data-power]")!.disabled,
    ).toBe(true);
  });
  it("does not send invalid brightness or preview actions", async () => {
    await start();
    root.querySelector<HTMLInputElement>("#zone-brightness")!.value = "0";
    root.querySelector<HTMLButtonElement>("#apply-zone-brightness")!.click();
    expect(run).not.toHaveBeenCalled();
    native.value = false;
    invoke.mockClear();
    run.mockClear();
    await start();
    expect(invoke).not.toHaveBeenCalled();
    root.querySelector<HTMLButtonElement>("[data-power]")!.click();
    expect(run).not.toHaveBeenCalled();
  });
});
