---
applyTo: "src/lib.rs|src/index.ts|src/ts/api/**/*.ts|src/ts/native/**/*.ts"
---

## Neon (Rust) <-> TypeScript bridge guidance

- Loading the addon
  - Do not change `src/ts/native/bridge.ts` loader candidates unless you update both paths. It must try `../../../index.node` and `../../../dist/index.node`.

- Public API stability
  - Keep `src/index.ts` as the public barrel. Export new symbols explicitly and keep type exports aligned with implementation.
  - Validate inputs at `MqttServer` method boundaries and prefer typed options (e.g., `QoS`, `PublishOptions`).

- Hook registration
  - `MqttServer.setHooks` should pass only provided callbacks; missing ones are treated as “not registered.”
  - Do not invoke hooks on the JS side unless registered; the Rust handler already checks availability.

- Error semantics
  - `start/stop/publish` return Promises. Reject on failure with clear messages. Logging should be WARN/ERROR only.

- Config builders
  - Maintain `createBasicConfig` and `createMultiProtocolConfig`. Enforce TLS/WSS cert+key requirements in validation.
