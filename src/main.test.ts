import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({ value: true }));
const invoke = vi.hoisted(() => vi.fn());
const listeners = vi.hoisted(
  () => new Map<string, (event: { payload: unknown }) => void>(),
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  isTauri: () => native.value,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi
    .fn()
    .mockImplementation(
      async (name: string, callback: (event: { payload: unknown }) => void) => {
        listeners.set(name, callback);
        return () => {};
      },
    ),
}));
vi.mock("lucide", () => ({
  createIcons: vi.fn(),
  Lightbulb: {},
  RefreshCw: {},
  Settings2: {},
  X: {},
  ArrowRight: {},
}));

const demo = (name = "Lampadina demo") => [
  {
    id: "demo-device",
    name,
    state: { on: true, mode: 160, brightness: 40, white: "warm" },
    error: null,
  },
];
const flush = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};
const range = () => document.querySelector<HTMLInputElement>(".brightness")!;
const power = () => document.querySelector<HTMLButtonElement>(".power-button")!;

beforeEach(() => {
  vi.resetModules();
  listeners.clear();
  vi.useFakeTimers();
  document.body.innerHTML = '<div id="app"></div>';
  native.value = true;
  invoke.mockReset().mockResolvedValue(demo());
});
afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

async function start() {
  await import("./main");
  await flush();
}

describe("lamp controls", () => {
  it("loads state without sending commands and renders names as text", async () => {
    invoke.mockResolvedValue(demo("<img src=x onerror=alert(1)>"));
    await start();
    expect(invoke.mock.calls).toEqual([["refresh"]]);
    expect(document.querySelector("h3")!.textContent).toContain("<img");
    expect(document.querySelector(".lamp-row img")).toBeNull();
  });

  it("debounces brightness and sends only the final value", async () => {
    await start();
    invoke.mockResolvedValue({
      on: true,
      mode: 160,
      brightness: 70,
      white: "warm",
    });
    for (const value of ["50", "70"]) {
      range().value = value;
      range().dispatchEvent(new Event("change"));
    }
    await vi.advanceTimersByTimeAsync(300);
    expect(invoke.mock.calls).toEqual([
      ["refresh"],
      ["control", { id: "demo-device", kind: "brightness", value: 70 }],
    ]);
  });

  it("cancels a queued brightness command when power is changed", async () => {
    await start();
    range().value = "60";
    range().dispatchEvent(new Event("change"));
    invoke.mockResolvedValue({
      on: false,
      mode: 224,
      brightness: 40,
      white: null,
    });
    power().click();
    await flush();
    await vi.advanceTimersByTimeAsync(400);
    expect(invoke.mock.calls).toEqual([
      ["refresh"],
      ["control", { id: "demo-device", kind: "power", value: 0 }],
    ]);
    expect(range().disabled).toBe(true);
    expect(power().disabled).toBe(false);
  });

  it("requires a fresh state after a command failure", async () => {
    await start();
    invoke.mockRejectedValue("Connessione non disponibile");
    power().click();
    await flush();
    expect(power().disabled).toBe(true);
    expect(document.querySelector(".lamp-error")!.textContent).toContain(
      "Connessione non disponibile",
    );
  });

  it("unlocks refresh after an unchanged pointer interaction", async () => {
    await start();
    range().dispatchEvent(new Event("pointerdown"));
    range().dispatchEvent(new Event("pointerup"));
    document.querySelector<HTMLButtonElement>("#refresh")!.click();
    await flush();
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("updates a renamed lamp without replacing its controls", async () => {
    await start();
    const oldRange = range();
    invoke.mockResolvedValue(demo("Nome demo aggiornato"));
    document.querySelector<HTMLButtonElement>("#refresh")!.click();
    await flush();
    expect(range()).toBe(oldRange);
    expect(document.querySelector("h3")!.textContent).toBe(
      "Nome demo aggiornato",
    );
    expect(range().getAttribute("aria-label")).toContain(
      "Nome demo aggiornato",
    );
  });

  it("never invokes native controls from the browser preview", async () => {
    native.value = false;
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => demo() }),
    );
    await start();
    power().click();
    expect(invoke).not.toHaveBeenCalled();
    expect(power().disabled).toBe(true);
    expect(document.querySelector("#notice")!.textContent).toContain(
      "inventati",
    );
  });

  it("disables stale controls when refresh fails", async () => {
    await start();
    invoke.mockRejectedValue("Sessione rifiutata");
    document.querySelector<HTMLButtonElement>("#refresh")!.click();
    await flush();
    expect(power().disabled).toBe(true);
    expect(range().disabled).toBe(true);
  });
  it("blocks individual controls and cancels pending brightness during tray operations", async () => {
    await start();
    range().value = "60";
    range().dispatchEvent(new Event("change"));
    listeners.get("zone-busy")!({ payload: true });
    expect(power().disabled).toBe(true);
    await vi.advanceTimersByTimeAsync(400);
    expect(invoke.mock.calls).toEqual([["refresh"]]);
    listeners.get("zone-busy")!({ payload: false });
    expect(power().disabled).toBe(false);
  });
  it("applies device-confirmed tray results to visible rows", async () => {
    await start();
    listeners.get("zone-result")!({
      payload: {
        message: "Zona aggiornata",
        lamps: [
          {
            ...demo()[0],
            state: { on: false, mode: 224, brightness: 40, white: null },
          },
        ],
      },
    });
    expect(power().getAttribute("aria-checked")).toBe("false");
    expect(document.querySelector("#notice")!.textContent).toBe(
      "Zona aggiornata",
    );
    expect(invoke.mock.calls).toEqual([["refresh"]]);
  });
  it("blocks lamp controls immediately when a zone shortcut starts", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "list_zones")
        return [
          { id: "demo-zone", name: "Zona demo", deviceIds: ["demo-device"] },
        ];
      if (command === "list_scenes") return [];
      return demo();
    });
    await start();
    const details =
      document.querySelector<HTMLDetailsElement>("#group-shortcuts")!;
    details.open = true;
    details.dispatchEvent(new Event("toggle"));
    await flush();
    range().value = "60";
    range().dispatchEvent(new Event("change"));
    let finish!: (value: unknown) => void;
    invoke.mockImplementation((command: string) =>
      command === "run_zone"
        ? new Promise((resolve) => {
            finish = resolve;
          })
        : Promise.resolve(demo()),
    );
    document.querySelector<HTMLButtonElement>("[data-power='0']")!.click();
    expect(power().disabled).toBe(true);
    await vi.advanceTimersByTimeAsync(400);
    expect(invoke).toHaveBeenCalledWith("run_zone", {
      id: "demo-zone",
      kind: "power",
      value: 0,
    });
    expect(
      invoke.mock.calls.filter(([command]) => command === "control"),
    ).toHaveLength(0);
    finish([
      {
        ...demo()[0],
        state: { on: false, mode: 224, brightness: 40, white: null },
      },
    ]);
    await flush();
    expect(power().disabled).toBe(false);
    expect(power().getAttribute("aria-checked")).toBe("false");
  });
  it("keeps safe white recovery available for legacy unsupported mode without a neutral control", async () => {
    invoke.mockResolvedValue([
      {
        ...demo()[0],
        state: { on: true, mode: 161, brightness: 40, white: null },
      },
    ]);
    await start();
    expect(range().disabled).toBe(true);
    const buttons = Array.from(
      document.querySelectorAll<HTMLButtonElement>(".white-control button"),
    );
    expect(buttons.map((button) => button.dataset.mode)).toEqual([
      "160",
      "162",
    ]);
    expect(buttons.every((button) => !button.disabled)).toBe(true);
    expect(document.querySelector(".lamp-description")!.textContent).toContain(
      "non supportata",
    );
  });
});
