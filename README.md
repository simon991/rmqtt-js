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
```

## API Reference

### `MqttServer`

#### Constructor
- `new MqttServer()` - Creates a new MQTT server instance

#### Methods
- `start(config)` - Start the server with the given configuration
- `stop()` - Stop the server gracefully  
- `close()` - Close and cleanup resources (call this to exit immediately)
- `running` - Property indicating if server is currently running

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

## Architecture

This package uses Neon bindings to bridge JavaScript and Rust:

- **JavaScript Layer**: Provides an idiomatic Node.js API with Promise-based async operations
- **Rust Layer**: Handles the actual MQTT server using RMQTT on a dedicated thread pool
- **Communication**: Uses channels for thread-safe communication between JS and Rust

The server runs on separate threads to avoid blocking the Node.js event loop, ensuring excellent performance for both MQTT operations and your Node.js application.

## Development

```bash
# Build the native module
npm run build

# Run tests
npm test

# After changing Rust code
npm install
```

## Requirements

- Node.js 14.0.0 or higher
- Rust toolchain (for building from source)

## License

MIT
