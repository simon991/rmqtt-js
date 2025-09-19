"use strict";

import { HookCallbacks } from "../api/hooks";

interface NativeModule {
  mqttServerNew(): unknown;
  mqttServerClose(this: unknown): void;
  mqttServerStart(this: unknown, config: any): Promise<void>;
  mqttServerStop(this: unknown): Promise<void>;
  mqttServerPublish(this: unknown, topic: string, payload: Buffer, qos: number, retain: boolean): Promise<void>;
  mqttServerSetHooks(this: unknown, callbacks: HookCallbacks): void;
}

// CJS require is necessary for neon native addon
// When compiled, this file lives at dist/src/ts/native/bridge.js
// The native addon is emitted to dist/index.node, hence the '../../../'
// eslint-disable-next-line @typescript-eslint/no-var-requires
const nativeModule = require("../../../index.node") as NativeModule;

export const {
  mqttServerNew,
  mqttServerClose,
  mqttServerStart,
  mqttServerStop,
  mqttServerPublish,
  mqttServerSetHooks,
} = nativeModule;
