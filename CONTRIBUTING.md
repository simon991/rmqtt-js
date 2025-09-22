# Contributing to rmqtt-server

Thanks for your interest in contributing! This project is a hybrid Rust + TypeScript Neon addon that exposes a high‑performance MQTT broker (RMQTT) to Node.js with a clean TS API. This guide explains how to set up your environment, build and test the project, and submit high‑quality changes.

## Quick links

- Repository overview for AI: see `.github/copilot-instructions.md`
- Path‑specific guidance: `.github/instructions/*.instructions.md`
- Example app: `examples/simple-server.ts`
- Tests: `test/*.test.ts`

## Prerequisites

- Node.js ≥ 14 (18+ recommended)
- Rust toolchain (stable)
  - macOS: install Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `build-essential`, `pkg-config`

Optional but helpful:
- Mosquitto clients (for local manual testing): `mosquitto_pub`, `mosquitto_sub`

## Install & build

Install dependencies and build the native addon + TypeScript outputs:

```bash
npm install
```

Useful scripts:

```bash
# Build native addon (dist/index.node) + compile TS to dist/
npm run build

# Build native addon only (release build and copy to dist/)
npm run build:native:release

# Build TypeScript only (src/ → dist/)
npm run build:ts

# Clean dist/
npm run clean
```

## Run tests

The suite uses Mocha and runs against real TCP ports:

```bash
npm test
```

Tips:
- Use unique ports per test and `waitForPort()` from `test/helpers.ts` to avoid flakiness.
- Always stop and close the server in `afterEach`:
  - `if (server.running) await server.stop(); server.close();`
- To run a subset of tests:
  - Mocha grep: `npm test -- -g "MQTT Server"`

## Example app

A comprehensive demo lives in `examples/simple-server.ts`:

```bash
npm run example
```

What it shows:
- Auth hook in JS (password must be "demo")
- Subscribe ACL with QoS cap
- Server‑side publishing and periodic heartbeats

## Project structure

- `src/index.ts` — Public API barrel (exports classes and types)
- `src/ts/api/MqttServer.ts` — TypeScript class wrapping native functions with validation
- `src/ts/api/*.ts` — Config, hooks, and public types
- `src/ts/native/bridge.ts` — Loads the Neon addon (`dist/index.node`) with dual path candidates
- `src/ts/utils/validators.ts` — Runtime config validation
- `src/lib.rs` — Neon module: exports functions to JS and marshals TS <-> Rust types
- `src/rs/server.rs` — Server lifecycle, Tokio runtime, publish pipeline
- `src/rs/hooks.rs` — Hook bridging to JS (auth, subscribe ACL, publish/subscribe/unsubscribe)
- `test/*.test.ts` — Mocha tests (port readiness helpers; ensure cleanup)
- `examples/simple-server.ts` — End‑to‑end demo

## Coding standards

### TypeScript
- Target CommonJS, strict mode (see `tsconfig.json`).
- Use `enum` (e.g., `QoS`) and `export interface` for public types.
- Validate inputs at API boundaries (`validateServerConfig`). Use clear, user‑facing error messages.
- When adding new options/types, update both compile‑time interfaces and runtime validation.
- Keep diffs minimal; avoid reformatting unrelated code.

### Rust (Neon + RMQTT)
- Add new JS‑visible methods as `impl MqttServerWrapper { fn js_*(...) }` and export them in `#[neon::main]`.
- Use `SendResultExt` to surface channel errors to JS promises.
- Hooks:
  - Store JS callbacks in `HOOK_CALLBACKS` and dispatch via `Channel::try_send`.
  - Decision hooks (auth/subscribe ACL) must use oneshot + 5s timeout; timeout/channel error → deny with WARN.
- Use WARN/ERROR logs sparingly for user‑significant events.

### Logging & errors
- Minimal and actionable logs only (WARN/ERROR).
- Reject Promises with clear errors in TS; return errors to JS in Rust using `into_rejection`.

## Implementation playbooks

Follow these patterns to keep the code coherent and consistent.

### 1) Add a new native method accessible from TypeScript

Rust:
- Implement `fn js_<Name>(cx: FunctionContext) -> JsResult<...>` on `MqttServerWrapper`.
- Use channels/async as needed; resolve/reject the `Deferred` via `Channel` for async.
- Export in `#[neon::main]` with a matching name (e.g., `mqttServer<PascalName>`).

TypeScript:
- Add the symbol to `src/ts/native/bridge.ts` `NativeModule` and to the exported destructuring.
- Wrap it in `src/ts/api/MqttServer.ts` with proper types and runtime checks.
- Update `src/index.ts` to export the new method/types.

Tests & docs:
- Add a focused test in `test/*.test.ts` (use a fresh port; ensure cleanup).
- Update README if the public API changes.

### 2) Add a new hook or extend hook payloads

Rust:
- Extend `HOOK_CALLBACKS` storage and `JavaScriptHookHandler` dispatch logic in `src/rs/hooks.rs`.
- For decision hooks, use oneshot + 5s timeout; define a clear default on failure/timeout.
- Map fields explicitly to JS objects with names aligned to TS interfaces.

TypeScript:
- Update `src/ts/api/hooks.ts` with new callback signatures and result types.
- Ensure `MqttServer.setHooks` maps only the provided callbacks.

Tests:
- Add integration tests that trigger the hook (use the `mqtt` client where practical).

### 3) Extend configuration

TypeScript:
- Update `ServerConfig`/`ListenerConfig` (and related option builders) in `src/ts/api/config.ts` and/or `src/ts/api/types.ts`.
- Update `validateServerConfig` to enforce new invariants (e.g., TLS requirements).

Rust:
- Parse the new fields in `lib.rs::parse_config` and thread them into `ServerConfig` in `src/rs/server.rs`.
- Apply options in RMQTT builders within `start_server`.

Tests:
- Add cases for invalid configs and valid multi‑protocol configurations.

## Testing guidelines

- Unique ports per test; derive a new port per case to avoid conflicts.
- Use `waitForPort(host, port)` to ensure readiness; avoid arbitrary sleeps.
- Always stop/close servers and MQTT clients in `afterEach` or `finally`.
- Keep timeouts tight (5–10s) and reduce console noise.

## Submitting changes

1. Create a feature branch.
2. Ensure everything builds locally:
  - `npm run build`
  - `npm test`
3. Include tests for new behavior and update docs (README, examples) if the public API changes.
4. Keep PRs focused with minimal, localized diffs.
5. Use clear commit messages describing the “why” and “what”.

## Release notes

- Prebuilt binaries are published for major platforms. On install, the module loads a prebuild when available; otherwise it falls back to building from source via `scripts/install.js` (Cargo required).
- `prepublishOnly` compiles TypeScript only; native artifacts are provided by prebuilds in the release pipeline.

## Troubleshooting

- Native build fails: ensure Rust toolchain is installed and platform build tools are available.
- “Failed to load native addon” at runtime: run `npm run build` to produce `dist/index.node` and ensure `bridge.ts` loader paths remain unchanged.
- Publish errors: ensure the server is running; the TS API rejects `publish()` when not running.

## License & conduct

- License: MIT (see `LICENSE`)
- Please be respectful and constructive in discussions and code review.

Thanks for contributing!