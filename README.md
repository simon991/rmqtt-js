<div align="center">

# mqtt-server

Blazing‑fast, experimental MQTT broker for Node.js — powered by Rust RMQTT and Neon.

</div>

## Table of contents

- Highlights
- Install
- Quick start
- Powerful hooks
- Configuration
- Pub/Sub API
- Example app
- Client cheat sheet
- TypeScript types
- Architecture (how it works)
- Testing
- Troubleshooting
- License

## Highlights

- High performance core: Built on Rust + Tokio with the battle‑tested RMQTT engine
- First‑class TypeScript API: Clean, promise‑based API with strong types
- Multi‑protocol listeners: TCP, TLS, WebSocket, and WSS — run them all at once
- Real‑time hooks: Observe publish/subscribe/unsubscribe events in JavaScript
- Pluggable auth: Implement authentication in JavaScript with a simple callback
- Subscribe ACLs: Allow/deny subscriptions in JS and optionally override granted QoS
- Full QoS and retention: QoS 0/1/2 and retained messages supported end‑to‑end
- Minimal, actionable logs: Only WARN/ERROR logs for user‑significant issues

## Install

```bash
npm install mqtt-server
```

Requirements:
- Node.js 18+ recommended (14+ supported)
- A Rust toolchain (the native module is built during install)
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `build-essential`, `pkg-config`

## Quick start

TypeScript
```ts
import { MqttServer, QoS } from "mqtt-server";

const server = new MqttServer();

// Optional: simple demo auth (password must be "demo")
server.setHooks({
  onClientAuthenticate: ({ username, password }) => ({
    allow: Boolean(username) && password === "demo",
    superuser: false,
  }),
});

await server.start(MqttServer.createBasicConfig(1883));

// Publish a message
await server.publish("sensor/temperature", "23.7", { qos: QoS.AtLeastOnce });
```

JavaScript (CommonJS)
```js
const { MqttServer, QoS } = require("mqtt-server");

(async () => {
  const server = new MqttServer();
  await server.start(MqttServer.createBasicConfig(1883));
  await server.publish("hello/world", Buffer.from("hi"), { qos: QoS.AtMostOnce });
})();
```

## Powerful hooks

Listen to broker events in real time — only invoked if you register them.

Auth (JS → Rust decision flow)
```ts
server.setHooks({
  onClientAuthenticate: ({ username, password, clientId, remoteAddr }) => {
    const allow = Boolean(username) && password === "demo";
    return { allow, superuser: false, reason: allow ? "ok" : "bad creds" };
  },
});
```

Subscribe ACLs with QoS override
```ts
server.setHooks({
  onClientSubscribeAuthorize: (session, sub) => {
    const user = session?.username ?? "";
    const allowed = sub.topicFilter.startsWith(`${user}/`) || sub.topicFilter.startsWith("server/public/");
    return allowed ? { allow: true, qos: 1 } : { allow: false, reason: "not authorized" };
  },
});
```

Publish/Subscribe notifications
```ts
server.setHooks({
  onMessagePublish: (_session, from, msg) => {
    console.log(`Message ${msg.topic}`, msg.payload.toString());
  },
  onClientSubscribe: (_session, sub) => console.log("SUB", sub),
  onClientUnsubscribe: (_session, unsub) => console.log("UNSUB", unsub),
});
```

Hook semantics:
- Not registered → not called (RMQTT defaults apply)
- Auth timeouts or callback errors → deny with WARN
- Subscribe ACL timeouts or callback errors → deny with WARN

## Configuration

Quick helpers
```ts
// Single TCP listener
const basic = MqttServer.createBasicConfig(1883);

// Multi‑protocol listeners (TLS/WSS require cert+key)
const multi = MqttServer.createMultiProtocolConfig({
  tcpPort: 1883,
  wsPort: 8080,
  tlsPort: 8883,
  wssPort: 8443,
  address: "0.0.0.0",
  // tlsCert: "/path/to/cert.pem",
  // tlsKey: "/path/to/key.pem",
  allowAnonymous: true,
});
```

Full shape (TypeScript)
```ts
interface ListenerConfig {
  name: string;
  address: string;      // e.g. "0.0.0.0"
  port: number;         // 1–65535
  protocol: "tcp" | "tls" | "ws" | "wss";
  tlsCert?: string;     // required for tls/wss
  tlsKey?: string;      // required for tls/wss
  allowAnonymous?: boolean; // default: true
}

interface ServerConfig {
  listeners: ListenerConfig[];
  pluginsConfigDir?: string;
  pluginsDefaultStartups?: string[];
}
```

## Pub/Sub API

```ts
await server.publish(topic: string, payload: string | Buffer, options?: {
  qos?: QoS;       // 0 | 1 | 2; default 0
  retain?: boolean // default false
});
```

Notes:
- Publishing requires the server to be running.
- QoS values are validated; invalid values are rejected with an ERROR log.
- Payloads are sent as bytes; strings are encoded as UTF‑8.

## Example app

This repo ships with a comprehensive example showing auth, ACLs, and server‑side publishing.

Run it:
```bash
npm run example
```

What it demonstrates:
- TCP server on port 1883
- JavaScript authentication (password = "demo")
- Subscribe ACL: only <username>/* and server/public/* are allowed; granted QoS capped at 1
- Real‑time logs for publish/subscribe/unsubscribe
- Periodic heartbeat publishing

The full source lives in `examples/simple-server.ts`.

See also: `examples/CHEATSHEET.md` for quick mqtt.js and Python client snippets (TCP/WS/TLS/WSS).

## TypeScript types

```ts
export enum QoS { AtMostOnce = 0, AtLeastOnce = 1, ExactlyOnce = 2 }

export interface SessionInfo { node: number; remoteAddr: string | null; clientId: string; username: string | null; }
export interface MessageFrom { type: "client" | "system" | "bridge" | "admin" | "lastwill" | "custom"; node: number; remoteAddr: string | null; clientId: string; username: string | null; }
export interface MessageInfo { dup: boolean; qos: QoS; retain: boolean; topic: string; payload: Buffer; createTime: number; }
export interface SubscriptionInfo { topicFilter: string; qos: QoS }
export interface UnsubscriptionInfo { topicFilter: string }

export interface AuthenticationRequest { clientId: string; username: string | null; password: string | null; protocolVersion: number; remoteAddr: string; keepAlive: number; cleanSession: boolean; }
export interface AuthenticationResult { allow: boolean; superuser?: boolean; reason?: string }
export interface SubscribeAuthorizeResult { allow: boolean; qos?: number; reason?: string }

export interface HookCallbacks {
  onClientAuthenticate?(auth: AuthenticationRequest): AuthenticationResult | Promise<AuthenticationResult>;
  onClientSubscribeAuthorize?(session: SessionInfo | null, sub: SubscriptionInfo): SubscribeAuthorizeResult | Promise<SubscribeAuthorizeResult>;
  onMessagePublish?(session: SessionInfo | null, from: MessageFrom, msg: MessageInfo): void;
  onClientSubscribe?(session: SessionInfo | null, sub: SubscriptionInfo): void;
  onClientUnsubscribe?(session: SessionInfo | null, unsub: UnsubscriptionInfo): void;
}
```

## Architecture (how it works)

- Rust core runs RMQTT on a dedicated Tokio runtime thread, separate from Node’s event loop
- Neon bridges expose a minimal, typed API to Node.js
- JS→Rust commands use an internal channel; Rust→JS hooks use a Neon event Channel
- Hook behavior: if a JS hook is registered, it’s authoritative; if not, RMQTT defaults apply
- Safety valves: 5s timeouts for JS decisions (auth/subscribe ACL). Timeout or channel error → deny with WARN
- Resource lifecycle: start/stop/close are graceful; a small shared state waits for server readiness before publishing

## Testing

```bash
npm test
```

What the suite covers:
- Auth allow/deny, timeouts, and missing‑hook behavior
- Subscribe ACL allow/deny with QoS override and session marshalling
- Publish path and client round‑trips using a real MQTT client
- Robust readiness using event‑driven port checks (no arbitrary sleeps)

## Security best practices

- Always enable TLS in production (tls/wss) and use strong certificates
  - Generate or provision certs from a trusted CA; rotate regularly
  - In `createMultiProtocolConfig`, only enable `tls/wss` when both `tlsCert` and `tlsKey` are present
- Never hard‑code secrets (passwords, tokens); pull them from env/secret managers
- Implement `onClientAuthenticate` on the JS side to enforce your auth model
  - Consider rate limiting and account lockouts outside the hook as needed
- Restrict subscription scope with `onClientSubscribeAuthorize` (e.g., `<username>/*`)
- Prefer unique client IDs; reject anonymous if your threat model requires it
- Log only WARN/ERROR and avoid sensitive data in logs
- Keep Node and Rust dependencies up to date; rebuild native module on updates

## Troubleshooting

- ERROR: Attempted to publish while server is not running → Call `start()` before `publish()`
- ERROR: Invalid configuration → Ensure at least one listener; TLS/WSS require both `tlsCert` and `tlsKey`
- WARN: Hook timeout/channel error → Your JS hook may be slow or threw; decisions fall back to deny
- Native build fails during install → Ensure Rust toolchain and platform build tools are available

## License

MIT
