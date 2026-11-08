import { vi } from "vitest";

import type { UpdatesContextValue } from "@/lib/updates";

/** A complete `UpdatesContextValue` for tests: idle, installed through an installer. */
export function makeUpdates(overrides: Partial<UpdatesContextValue> = {}): UpdatesContextValue {
    return {
        appVersion: "0.1.0",
        installInfo: { channel: "installer", selfUpdate: true },
        phase: "idle",
        update: null,
        progress: null,
        error: null,
        check: vi.fn().mockResolvedValue(undefined),
        install: vi.fn().mockResolvedValue(undefined),
        restart: vi.fn().mockResolvedValue(undefined),
        ...overrides,
    };
}
