# MQTT Server

A high-performance MQTT server for Node.js built with Rust and Neon bindings. This package wraps the powerful [RMQTT](https://github.com/rmqtt/rmqtt) Rust library to provide an easy-to-use MQTT broker that can handle thousands of concurrent connections.

## Features

- **High Performance**: Built on Tokio and RMQTT for excellent async performance
- **Multi-Protocol Support**: TCP, TLS, WebSocket, and WebSocket Secure (WSS)
- **Multiple Listeners**: Support for binding multiple listeners simultaneously
- **TLS Encryption**: Easy TLS configuration for secure connections
- **TypeScript Support**: Full TypeScript definitions and type safety
- **Lightweight**: Minimal overhead native bindings
- **Simple API**: Clean TypeScript/JavaScript interface with Promise-based async operations
- **Pub/Sub API**: Native publish/subscribe functionality through Rust MQTT server
- **Real-time Hook System**: Listen to MQTT events (publish, subscribe, unsubscribe) with JavaScript callbacks
- **Pluggable Authentication**: Implement auth in JavaScript with onClientAuthenticate
- **Subscribe Authorization**: Allow/deny subscriptions from JavaScript via onClientSubscribeAuthorize
- **QoS Support**: Full Quality of Service levels (0, 1, 2) for message delivery
- **Message Retention**: Support for retained messages
- **Buffer Support**: Native handling of binary message payloads

## Installation

```bash
npm install mqtt-server
```

## Quick Start

### TypeScript
```typescript
import { MqttServer } from "mqtt-server";

(async () => {
    const server = new MqttServer();
    // Optional: demo authentication hook (password must be "demo")
    server.setHooks({
        onClientAuthenticate: ({ username, password }) => ({
            allow: Boolean(username) && password === 'demo',
            superuser: false,
            reason: 'Demo credentials: password must be "demo"'
        })
    });
    
    // Create a basic TCP server on port 1883
    const config = MqttServer.createBasicConfig(1883);
    
    await server.start(config);
    console.log("MQTT server started on port 1883");
    
    // Server will run until stopped
    // await server.stop();
})();
```

### JavaScript
```javascript
const { MqttServer } = require("mqtt-server");

(async () => {
    const server = new MqttServer();
    
    // Create a basic TCP server on port 1883
    const config = MqttServer.createBasicConfig(1883);
    
    await server.start(config);
    console.log("MQTT server started on port 1883");
    
    // Server will run until stopped
    // await server.stop();
})();
```

## Pub/Sub API Usage

### Publishing Messages

The server provides a `publish()` method to send messages to topics:

```typescript
import { MqttServer, QoS } from "mqtt-server";

const server = new MqttServer();
await server.start(MqttServer.createBasicConfig(1883));

// Simple publish
await server.publish("sensor/temperature", "23.5");

// Publish with options
await server.publish("alerts/critical", Buffer.from("System failure"), {
    qos: QoS.AtLeastOnce,
    retain: true
});

// Publish JSON data
const data = { temperature: 23.5, humidity: 45.2 };
await server.publish("sensor/data", JSON.stringify(data));
```

### Hook System - Listening to MQTT Events

Use the hook system to listen for real-time MQTT events (and implement authentication if desired):

```typescript
import { MqttServer, HookCallbacks } from "mqtt-server";

const server = new MqttServer();

// Set up event listeners
const hooks: HookCallbacks = {
    onMessagePublish: (session, from, message) => {
        console.log(`Message published to ${message.topic}:`, message.payload.toString());
        console.log(`From: ${from.type}, QoS: ${message.qos}, Retain: ${message.retain}`);
    },
    
    onClientSubscribe: (session, subscription) => {
        console.log(`Client subscribed to: ${subscription.topicFilter}`);
        console.log(`QoS: ${subscription.qos}`);
    },
    
    onClientUnsubscribe: (session, unsubscription) => {
        console.log(`Client unsubscribed from: ${unsubscription.topicFilter}`);
    },
    // Optional: authorize subscriptions with fine-grained control
    onClientSubscribeAuthorize: (session, subscription) => {
        // Example policy: user may only subscribe within their namespace or a public area
        const user = session?.username ?? '';
        const tf = subscription.topicFilter;
        const allow = tf.startsWith(`${user}/`) || tf.startsWith('public/');
        return allow
            ? { allow: true, qos: QoS.AtLeastOnce } // cap granted QoS to 1
            : { allow: false, reason: 'Not authorized' };
    },
    onClientAuthenticate: (auth) => {
        // Allow username with password === 'demo'
        if (auth.username && auth.password === 'demo') return { allow: true, superuser: false };
        return { allow: false, superuser: false, reason: 'Invalid credentials' };
    }
};

// Register the hooks before starting the server
server.setHooks(hooks);
## Subscribe Authorization Hook

Use onClientSubscribeAuthorize to control whether a client may subscribe to a topic filter and optionally override the granted QoS:

```ts
server.setHooks({
    onClientSubscribeAuthorize: (session, subscription) => {
        const user = session?.username ?? '';
        const tf = subscription.topicFilter;
        if (tf.startsWith(`${user}/`) || tf.startsWith('server/public/')) {
            return { allow: true, qos: QoS.AtLeastOnce };
        }
        return { allow: false, reason: 'Not authorized for topic filter' };
    }
});
```

Behavior:
- If the hook returns allow: true, the broker will accept the subscription and grant the specified QoS (or the requested QoS if none is provided).
- If the hook returns allow: false, the subscription is rejected (clients may see an error or a failure code depending on protocol version).
- If no hook is registered, the decision is deferred to RMQTT’s defaults and configuration.
- The hook has a 5s internal timeout; timeouts or callback errors result in a deny with a WARN log.

await server.start(MqttServer.createBasicConfig(1883));

// Now the server will call your functions when events occur
```

### Real-time Message Processing

Combine publishing with hooks for real-time message processing:

```typescript
const server = new MqttServer();

server.setHooks({
    onMessagePublish: (session, from, message) => {
        // Process incoming messages from clients
        if (message.topic.startsWith("sensor/")) {
            const data = JSON.parse(message.payload.toString());
            
            // Republish processed data
            server.publish("processed/" + message.topic, JSON.stringify({
                ...data,
                processed_at: Date.now(),
                from_client: from.clientId
            }));
        }
    }
});

await server.start(MqttServer.createBasicConfig(1883));
```

### Integration with MQTT Clients

The server works seamlessly with any MQTT client library:

```typescript
// Server setup
const server = new MqttServer();
await server.start(MqttServer.createBasicConfig(1883));

// Using mqtt.js client
import * as mqtt from 'mqtt';
const client = mqtt.connect('mqtt://localhost:1883');

client.on('connect', () => {
    client.subscribe('sensor/+');
    
    // Server can publish to this client
    server.publish('sensor/temperature', '25.0');
});

client.on('message', (topic, payload) => {
    console.log(`Received: ${topic} = ${payload.toString()}`);
});
```

## Running the Example

This package includes a complete example server that demonstrates all features. You can run it in several ways:

### Using npm script (recommended)
```bash
npm run example
```

### Direct execution with Node.js (requires Node.js 20.6.0+)
```bash
# Build the project first
npm run build

# Run the TypeScript example directly
node examples/simple-server.ts --experimental-strip-types
```

### Running the compiled JavaScript
```bash
# Build the project
npm run build

# Run the compiled example
node dist/examples/simple-server.js
```

The example server will:
- Start an MQTT server on port 1883
- Register a demo authentication hook (password must be "demo")
- Show connection instructions for various clients
- Handle graceful shutdown with Ctrl+C
- Display helpful client connection examples

**Note**: The `--experimental-strip-types` flag allows Node.js to run TypeScript files directly without compilation. This feature is available in Node.js 20.6.0 and later.

## Multi-Protocol Example

```typescript
import { MqttServer, MultiProtocolOptions } from "mqtt-server";

(async () => {
    const server = new MqttServer();
    
    // Configure multiple protocols
    const options: MultiProtocolOptions = {
        tcpPort: 1883,      // Standard MQTT
        tlsPort: 8883,      // Secure MQTT
        wsPort: 8080,       // MQTT over WebSocket
        wssPort: 8443,      // MQTT over Secure WebSocket
        tlsCert: "./server.pem",
        tlsKey: "./server.key"
    };
    
    const config = MqttServer.createMultiProtocolConfig(options);
    
    await server.start(config);
    console.log("Multi-protocol MQTT server started");
})();
```

## Custom Configuration

```js
const server = new MqttServer();

const config = {
    listeners: [
        {
            name: "external-tcp",
            address: "0.0.0.0",
            port: 1883,
            protocol: "tcp",
            allowAnonymous: true
        },
        {
            name: "internal-tcp", 
            address: "127.0.0.1",
            port: 11883,
            protocol: "tcp",
            allowAnonymous: false
        },
        {
            name: "websocket",
            address: "0.0.0.0",
            port: 8080,
            protocol: "ws",
            allowAnonymous: true
        }
    ]
};

await server.start(config);

## Testing Notes

Tests use an event-driven readiness check (waiting for the TCP port to be listening) instead of arbitrary sleeps, which makes the suite faster and less flaky. If you add tests that start the server, wait for the port to be ready before connecting clients.

Example helper:

```ts
import * as net from 'net';

async function waitForPort(host: string, port: number, timeoutMs = 3000, intervalMs = 50) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
        const ok = await new Promise<boolean>(resolve => {
            const s = new net.Socket();
            s.setTimeout(300);
            s.once('connect', () => { s.destroy(); resolve(true); });
            s.once('timeout', () => { s.destroy(); resolve(false); });
            s.once('error', () => resolve(false));
            s.connect(port, host);
        });
        if (ok) return;
        await new Promise(r => setTimeout(r, intervalMs));
    }
    throw new Error(`Port not ready: ${host}:${port}`);
}
```
```

## API Reference

### `MqttServer`

#### Constructor
- `new MqttServer()` - Creates a new MQTT server instance

#### Methods
- `start(config)` - Start the server with the given configuration
- `stop()` - Stop the server gracefully  
- `close()` - Close and cleanup resources (call this to exit immediately)
- `publish(topic, payload, options)` - Publish a message to a topic
- `setHooks(callbacks)` - Register event listeners for MQTT events
- `running` - Property indicating if server is currently running

#### Pub/Sub Methods

##### `publish(topic, payload, options?)`
Publish a message to a topic.

**Parameters:**
- `topic: string` - The topic to publish to
- `payload: string | Buffer` - The message payload
- `options?: object` - Optional publish options
  - `qos?: QoS` - Quality of Service level (0, 1, or 2)
  - `retain?: boolean` - Whether to retain the message

**Returns:** `Promise<void>`

##### `setHooks(callbacks)`
Register event listeners for MQTT events.

**Parameters:**
- `callbacks: HookCallbacks` - Object containing callback functions
  - `onMessagePublish?: (session, from, message) => void` - Called when messages are published
  - `onClientSubscribe?: (session, subscription) => void` - Called when clients subscribe
  - `onClientUnsubscribe?: (session, unsubscription) => void` - Called when clients unsubscribe
    - `onClientSubscribeAuthorize?: (session, subscription) => { allow: boolean; qos?: QoS; reason?: string } | Promise<...>` - Decide subscribe ACLs

#### Static Methods
- `MqttServer.createBasicConfig(port, address)` - Create a simple TCP configuration
- `MqttServer.createMultiProtocolConfig(options)` - Create multi-protocol configuration

### Configuration Object

```js
{
    listeners: [
        {
            name: "string",           // Listener identifier
            address: "string",        // Bind address (default: "0.0.0.0")
            port: number,             // Port number
            protocol: "tcp|tls|ws|wss", // Protocol type
            tlsCert: "string",        // TLS certificate path (for tls/wss)
            tlsKey: "string",         // TLS key path (for tls/wss)
            allowAnonymous: boolean   // Allow anonymous connections (default: true)
        }
    ],
    pluginsConfigDir: "string"        // Optional: Plugin configuration directory
}
```

### TypeScript Interfaces

The library provides comprehensive TypeScript interfaces for type-safe development:

#### QoS Enum
```typescript
enum QoS {
  AtMostOnce = 0,    // At most once delivery
  AtLeastOnce = 1,   // At least once delivery  
  ExactlyOnce = 2    // Exactly once delivery
}
```

#### Message Interfaces
```typescript
interface MessageInfo {
  dup: boolean;           // Whether this is a duplicate message
  qos: QoS;              // Quality of Service level
  retain: boolean;        // Whether this message should be retained
  topic: string;         // Topic name
  payload: Buffer;       // Message payload
  createTime: number;    // When the message was created (milliseconds since epoch)
}

interface MessageFrom {
  type: "client" | "system" | "bridge" | "admin" | "lastwill" | "custom";
  node: number;          // Node ID of the source
  remoteAddr: string | null;  // Remote address of the source
  clientId: string;      // Client ID of the source
  username: string | null;     // Username of the source
}
```

#### Session and Subscription Interfaces
```typescript
interface SessionInfo {
  node: number;          // Node ID where the client is connected
  remoteAddr: string;    // Client's remote IP address and port
  clientId: string;      // Client identifier
  username: string | null;     // Username (may be null for anonymous connections)
}

interface SubscriptionInfo {
  topicFilter: string;   // Topic filter being subscribed to
  qos: QoS;             // Quality of Service level for the subscription
}

interface UnsubscriptionInfo {
  topicFilter: string;   // Topic filter being unsubscribed from
}
```

#### Hook Callbacks Interface
```typescript
interface HookCallbacks {
  onMessagePublish?: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => void;
  onClientSubscribe?: (session: SessionInfo | null, subscription: SubscriptionInfo) => void;
  onClientUnsubscribe?: (session: SessionInfo | null, unsubscription: UnsubscriptionInfo) => void;
}
```

## Architecture

This package uses Neon bindings to bridge JavaScript and Rust:

- **JavaScript Layer**: Provides an idiomatic Node.js API with Promise-based async operations and comprehensive TypeScript interfaces
- **Rust Layer**: Handles the actual MQTT server using RMQTT on a dedicated thread pool with native hook system integration
- **Communication**: Uses channels for thread-safe communication between JS and Rust, including real-time event callbacks
- **Hook System**: RMQTT's native hook system is exposed to JavaScript for real-time MQTT event monitoring
- **Message Routing**: Direct integration with RMQTT's message forwarding system for efficient pub/sub operations

The server runs on separate threads to avoid blocking the Node.js event loop, ensuring excellent performance for both MQTT operations and your Node.js application. The hook system provides real-time callbacks for all MQTT events without performance overhead.

## Development

```bash
# Build the native module
npm run build

# Run tests
npm test

# Run the example server
npm run example

# After changing Rust code
npm install
```

### Example Development
The example server in `examples/simple-server.ts` can be run directly with:
```bash
node examples/simple-server.ts --experimental-strip-types
```

This demonstrates:
- Basic MQTT server setup
- Configuration options
- Graceful shutdown handling
- Client connection examples

## Requirements

- Node.js 14.0.0 or higher
- Rust toolchain (for building from source)

## License

MIT
