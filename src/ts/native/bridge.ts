"use strict";

import { HookCallbacks } from "../api/hooks";
import path from "path";

interface NativeModule {
  mqttServerNew(): unknown;
  mqttServerClose(this: unknown): void;
  mqttServerStart(this: unknown, config: any): Promise<void>;
  mqttServerStop(this: unknown): Promise<void>;
  mqttServerPublish(this: unknown, topic: string, payload: Buffer, qos: number, retain: boolean): Promise<void>;
  mqttServerSetHooks(this: unknown, callbacks: HookCallbacks): void;
}

// CJS require is necessary for neon native addon
// This code must work in two scenarios:
// 1) Compiled output at dist/src/ts/native/bridge.js, with addon at dist/index.node
// 2) ts-node executing the source at src/ts/native/bridge.ts, with addon at dist/index.node
// We try both locations in order.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let nativeModule: any;
{
  const candidates = [
    path.join(__dirname, "../../../index.node"), // compiled layout (dist)
    path.join(__dirname, "../../../dist/index.node"), // ts-node layout (source)
  ];
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const tryRequire = (p: string) => { try { return require(p); } catch { return undefined; } };
  for (const c of candidates) { nativeModule = tryRequire(c); if (nativeModule) break; }
  if (!nativeModule) {
    throw new Error(`Failed to load native addon (index.node). Tried: ${candidates.join(", ")}`);
  }
}

export const {
  mqttServerNew,
  mqttServerClose,
  mqttServerStart,
  mqttServerStop,
  mqttServerPublish,
  mqttServerSetHooks,
} = nativeModule;
