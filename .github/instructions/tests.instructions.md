---
applyTo: "test/**/*.test.ts"
---

## Test authoring guidelines for Copilot

Follow these rules to keep the suite fast, reliable, and aligned with our runtime model (Rust + Neon + Node.js):

- Use unique TCP ports per test
  - Generate a fresh port for each case to avoid interference. See existing helpers in `test/server.test.ts`.
  - After starting the server, wait for readiness using `waitForPort(host, port)` from `test/helpers.ts`.

- Always clean up
  - In `afterEach`, if `server.running`, `await server.stop()` then call `server.close()`.
  - Never leave intervals/timeouts running in tests; cancel them before finishing the case.

- Prefer event-driven readiness over sleeps
  - Avoid arbitrary `setTimeout` waits. Use `waitForPort` for server readiness and library callbacks/events.

- Build assumptions
  - The test script runs `npm run build:native` automatically. If running a single test manually, ensure `dist/index.node` exists.
  - Do not mutate `src/ts/native/bridge.ts` loader paths; tests rely on the dual-candidate loading mechanism.

- MQTT client flows
  - Use the `mqtt` package for real client interactions where helpful.
  - Keep timeouts tight (5–10s) and close client connections in `finally` blocks.

- Server API usage
  - Use `MqttServer.createBasicConfig(port)` unless a custom listener is required.
  - For publish tests, assert no throws when server is running; assert reject when not running.

- Hooks
  - Register hooks via `server.setHooks({...})` before `start()`.
  - For auth/ACL tests, remember that the Rust side denies on callback timeout (>5s) or channel errors.

- Performance & flake prevention
  - Avoid global state; keep each test self-contained.
  - Keep console noise minimal; only print on failure paths if needed.
