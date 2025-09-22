# Copilot Instructions for MQTT Server

## Project Overview

This is a **Neon.js hybrid Rust/TypeScript project** that wraps the high-performance RMQTT Rust MQTT server library for Node.js. The project provides a comprehensive TypeScript API for creating and managing MQTT brokers with support for multiple protocols (TCP, TLS, WebSocket, WSS), concurrent connections, **pub/sub functionality**, and **real-time event hooks**.

## Architecture & Key Patterns

### Rust-TypeScript Bridge Pattern
- **Rust side**: `src/lib.rs` exports MQTT server functions via `#[neon::main]` using naming convention `mqtt_server_*`
- **TypeScript side**: `index.ts` wraps native functions in an idiomatic `MqttServer` class with full type safety
- **Memory management**: Uses `JsBox<MqttServerWrapper>` to safely pass Rust server instances to JavaScript scope
- **Type definitions**: Native module types defined in `types/` directory for compile-time safety
- **Hook integration**: JavaScript callbacks stored with `neon::handle::Root<JsFunction>` for real-time MQTT event notifications

### Async Server Pattern with Threading
- **Core design**: RMQTT server runs on dedicated Tokio runtime to avoid blocking Node.js event loop
- **Communication**: Uses `mpsc::channel` for JavaScript→Rust commands and `neon::event::Channel` for Rust→JavaScript callbacks
- **Server lifecycle**: Supports start/stop operations with graceful shutdown and configuration validation
- **TypeScript promises**: All async operations return properly typed Promise objects
- **Context synchronization**: Uses `SharedServerState` with `wait_for_ready()` to ensure server context availability

```typescript
// Key pattern: Type-safe server configuration with pub/sub
const config: ServerConfig = {
  listeners: [{
    name: "tcp",
    port: 1883,
    protocol: "tcp",
    allowAnonymous: true
  }]
};
await server.start(config);

// Pub/Sub API usage
await server.publish("sensor/data", Buffer.from("temperature: 23.5"));

// Hook system for real-time events
server.setHooks({
  onMessagePublish: (session, from, message) => {
    console.log(`Message: ${message.topic} = ${message.payload.toString()}`);
  }
});
```

### MQTT Configuration Pattern
- **Strongly typed**: All configuration objects have TypeScript interfaces (`ServerConfig`, `ListenerConfig`)
- **Multi-listener support**: Each listener has protocol (tcp/tls/ws/wss), address, port, and TLS settings
- **Protocol enforcement**: TypeScript compiler ensures only valid protocols are used
- **TLS validation**: Runtime validation enforces certificate/key requirements for secure protocols

### Hook System Architecture
- **RMQTT Integration**: Uses RMQTT's native `Hook` trait with `JavaScriptHookHandler` implementation
- **Event Types (supported)**:
  - Decision hooks: `onClientAuthenticate`, `onClientSubscribeAuthorize`, `onClientPublishAuthorize`
  - Publish/subscribe notifications: `onMessagePublish`, `onClientSubscribe`, `onClientUnsubscribe`
  - Delivery notifications: `onMessageDelivered`, `onMessageAcked`, `onMessageDropped`
  - Client/session lifecycle: `onClientConnect`, `onClientConnack` (fires on both success and failure), `onClientConnected`, `onClientDisconnected`, `onSessionCreated`, `onSessionSubscribed`, `onSessionUnsubscribed`, `onSessionTerminated`
- **JavaScript Callbacks**: Stored in global `HookCallbackStorage` with `once_cell::Lazy<Mutex<>>`; only invoked if registered
- **Parameter Marshaling**: Converts Rust structures to JavaScript objects matching TypeScript interfaces (SessionInfo, MessageFrom, MessageInfo, SubscriptionInfo, UnsubscriptionInfo, ConnectInfo/ConnackInfo, AuthenticationRequest/Result)
- **Hook Registration**: Handlers registered with `hook_register.add()` and enabled with `hook_register.start()`
- **Auth Semantics**: If JS auth hook returns a decision, it's final; if no JS hook is registered, defer to RMQTT (`proceed = true`). JS auth evaluation uses an internal oneshot with a 5s timeout; on timeout or channel error, deny.
- **Notes**: `onClientConnack` provides a string reason (e.g., "Success", "BadUsernameOrPassword", "NotAuthorized"). `SessionInfo.node` is currently `1` (single-node placeholder) until clustering is introduced.

# Repository custom instructions for GitHub Copilot

These instructions tell Copilot how to work effectively in this repository. They apply to Copilot Chat, the Copilot coding agent, and Copilot code review.

Trust these instructions first. Only search the codebase or the web if the information here is incomplete or appears incorrect.

## Project overview

- Purpose: High-performance MQTT broker for Node.js with a typed TypeScript API, powered by Rust RMQTT and Neon.
- Stack: TypeScript (Node.js), Rust (Tokio, RMQTT), Neon for JS <-> Rust bridge.
- Output: Node package exposing `MqttServer` with multi-protocol listeners, publish API, and real-time hooks.
- Primary entry points:
  - TypeScript public API: `src/index.ts`, `src/ts/api/MqttServer.ts`
  - Rust native module: `src/lib.rs`, with implementation in `src/rs/*.rs`
  - Native bridge loader: `src/ts/native/bridge.ts` (loads `dist/index.node`)
- Example: `examples/simple-server.ts`
- Tests: `test/*.test.ts` use Mocha, run against real ports.

## How to build, test, and run

- Node: 18+ recommended (engines allow >=14). Rust toolchain required.
- Build all (native + TS):
  - `npm run build` (cleans, builds native via Cargo, then compiles TS to `dist/`)
- Build native only:
  - `npm run build:native` (produces `dist/index.node` via cargo-cp-artifact)
- Build TS only:
  - `npm run build:ts` (compiles TS under `src/` to `dist/`)
- Run tests:
  - `npm test` (builds native, then runs Mocha on `test/**/*.test.ts` with ts-node)
- Example app:
  - `npm run example` (build then run `examples/simple-server.ts`)

Important runtime artifact paths:
- Compiled JS: `dist/index.js`
- Native addon: `dist/index.node`
- The loader in `src/ts/native/bridge.ts` tries both `../../../index.node` and `../../../dist/index.node` relative to the compiled layout. Do not change these without updating both paths.

## Repository structure (what goes where)

- `src/index.ts` — Public API barrel (exports classes and types)
- `src/ts/api/MqttServer.ts` — TypeScript class that wraps native functions and enforces runtime validation
- `src/ts/api/*.ts` — Config, hooks, and public types for consumers
- `src/ts/native/bridge.ts` — Loads the Neon addon and exposes typed native calls
- `src/ts/utils/validators.ts` — Runtime config validation
- `src/lib.rs` — Neon module: exports functions to JS and marshals TS <-> Rust types
- `src/rs/server.rs` — RMQTT server lifecycle, Tokio runtime thread, publish pipeline
- `src/rs/hooks.rs` — Hook bridging to JS (auth, subscribe ACL, publish/subscribe/unsubscribe notifications)
- `test/*.test.ts` — Mocha tests (port readiness helpers; ensure cleanup)
- `examples/simple-server.ts` — End-to-end demo including hooks and server-side publish

## Architecture notes (keep these consistent)

- Rust <-> TS bridge pattern
  - Rust exports Neon functions from `lib.rs` (e.g., `mqttServerStart`, `mqttServerPublish`) bound to methods on `MqttServerWrapper`.
  - TypeScript imports these via `src/ts/native/bridge.ts` and wraps them in `MqttServer`.
  - Memory safety via `JsBox<MqttServerWrapper>`; JS holds a box that owns the server channel/runtime.
- Async + threading
  - A dedicated Tokio runtime runs the RMQTT server on a background thread.
  - A channel (`mpsc`) handles JS→Rust commands; a Neon event `Channel` handles Rust→JS callbacks.
  - `SharedServerState.wait_for_ready()` gates publish calls until context is available (5s timeout in publish path).
- Hooks
  - Supported: `onClientAuthenticate`, `onClientSubscribeAuthorize`, `onClientPublishAuthorize`, `onMessagePublish`, `onClientSubscribe`, `onClientUnsubscribe`, `onMessageDelivered`, `onMessageAcked`, `onMessageDropped`, `onClientConnect`, `onClientConnack`, `onClientConnected`, `onClientDisconnected`, `onSessionCreated`, `onSessionSubscribed`, `onSessionUnsubscribed`, `onSessionTerminated`.
  - JS callback registration lives in `lib.rs::js_set_hooks` and stored in `HOOK_CALLBACKS` (see `src/rs/hooks.rs`).
  - Auth/Subscribe ACL decisions use oneshot with 5s timeout; timeout or channel error → deny with WARN. If no JS hook is registered, defer to RMQTT defaults.
- Logging & errors
  - Keep logs minimal and actionable: WARN/ERROR only for user‑significant events.
  - Prefer typed errors to `throw` with clear messages; TypeScript path rejects promises on failures.

## Coding standards and preferences for Copilot

- TypeScript
  - Target CommonJS, strict mode (see `tsconfig.json`). Prefer `enum` for protocol/QoS and `export interface` for configs and hook payloads.
  - Avoid reformatting unrelated code; keep diffs minimal and localized.
  - Validate inputs at API boundaries (`validateServerConfig`). Clear, user‑facing error messages.
  - When adding options/types, update both compile-time interfaces and runtime validation.
- Rust (Neon + RMQTT)
  - Add new JS-visible methods as `impl MqttServerWrapper { fn js_*(...) }` and export them in `#[neon::main]`.
  - Use the existing `SendResultExt` to surface channel errors to JS promises.
  - For JS callbacks, store `Root<JsFunction>` in `HOOK_CALLBACKS` and dispatch via `Channel::try_send`.
  - Maintain 5s safety timeouts on JS-dependent decisions; prefer deny-on-timeout for auth/ACL.
- Tests
  - Use unique ports per test and `waitForPort()` readiness probe.
  - Always `await server.stop()` and `server.close()` in `afterEach` to avoid hanging processes.
  - Favor real MQTT flows where practical; keep test runtime tight.
- General
  - Don’t invent commands or paths; follow the scripts above.
  - Keep public API stable; if changing behavior, update README and examples.

## Implementation playbooks

When you add functionality, follow these patterns to keep the code coherent.

### 1) Add a new native method accessible from TypeScript

- Rust
  - Implement `fn js_<name>(cx: FunctionContext) -> JsResult<...>` on `MqttServerWrapper`.
  - Use channels/async as needed; resolve/reject the `Deferred` via `Channel` for async.
  - Export in `#[neon::main]` with a matching name (e.g., `mqttServer<PascalName>`).
- TypeScript
  - Add the symbol to `src/ts/native/bridge.ts` `NativeModule` and to the exported destructuring.
  - Wrap it in `src/ts/api/MqttServer.ts` as an idiomatic method with proper types and runtime checks.
  - Update `src/index.ts` exports as needed.
- Tests & docs
  - Add a focused test in `test/*.test.ts` (use a fresh port, ensure cleanup).
  - Document in README if it affects public API.

### 2) Add a new hook or extend hook payloads

- Rust
  - Extend `HOOK_CALLBACKS` storage and `JavaScriptHookHandler` dispatch logic.
  - For decision hooks, use oneshot + 5s timeout; define a clear default on failure/timeout.
  - Map fields explicitly to JS objects; keep names/types aligned with TS interfaces.
- TypeScript
  - Update `src/ts/api/hooks.ts` with new callback type/signatures and result types.
  - Update `MqttServer.setHooks` call site mapping in `lib.rs::js_set_hooks` if you add a field.
- Tests
  - Add integration tests that trigger the hook via a real client when feasible.

### 3) Extend configuration

- TypeScript
  - Update `ServerConfig`/`ListenerConfig` (and related option builders) in `src/ts/api/config.ts` and `src/ts/api/types.ts` if relevant.
  - Update `validateServerConfig` to enforce new invariants (e.g., TLS requirements).
- Rust
  - Parse the new fields in `lib.rs::parse_config` and thread them into `ServerConfig` in `src/rs/server.rs`.
  - Apply them in RMQTT builder(s) within `start_server`.
- Tests
  - Add tests to cover invalid configs and valid multi-protocol configurations.

## Testing checklist (what “done” looks like)

- Unit/integration tests pass locally via `npm test`.
- Server lifecycle tests:
  - Start/stop/close without leaks; port actually listens (use `waitForPort`).
  - Publish with server running succeeds; with server stopped rejects clearly.
- Hook tests:
  - Auth allow/deny/timeout semantics are correct.
  - Subscribe ACL allow/deny with optional QoS override, including timeouts.
- Example(s) still run: `npm run example` completes setup and publishes at least once.

## Common pitfalls (avoid these)

- Not building native before running TS that expects `dist/index.node`. Fix by running `npm run build` or `npm run build:native`.
- Changing `bridge.ts` loader paths without updating both candidates. Keep both paths in sync.
- Missing `server.close()` after tests — can leave the process hanging.
- Publishing before the server context is ready — the Rust side guards with `wait_for_ready(5000)`, but prefer to wait for port readiness in tests/flows.
- Invalid QoS values cause a logged ERROR and message drop in Rust; validate QoS at the API call site when possible.

## Security and production notes

- Prefer TLS/WSS in production; both `tlsCert` and `tlsKey` are required for secure listeners.
- Implement your `onClientAuthenticate` in JS; if not set, RMQTT defaults apply.
- Use Subscribe ACL (`onClientSubscribeAuthorize`) to scope subscriptions (e.g., `<username>/*`) and cap QoS as needed.
- Keep logs minimal; avoid sensitive information in log messages.

## Acceptance criteria & quality gates for Copilot-generated changes

Before finishing a PR, Copilot should ensure:
- Build: `npm run build` succeeds (native + TS).
- Tests: `npm test` passes (Mocha, real ports, no hangs). Each new public behavior has tests.
- Types: TS compiles strictly (no new errors). Public types updated where behavior changed.
- Docs: README and examples updated for public API changes. Keep changelog-style notes in PR description.
- Diffs: Minimal and localized; no machinery reformat unless necessary for the change.

## When to search vs. when to trust these instructions

- Default: follow this document. Search the repo for exact symbols/paths if something doesn’t match.
- Web search only when implementing new 3rd‑party patterns or when RMQTT/Neon usage requires current upstream examples.

---

If you need path‑specific guidance (for certain file types), add `.github/instructions/*.instructions.md` files with an `applyTo` glob in the front matter. For now, this single file is authoritative for the whole repository.