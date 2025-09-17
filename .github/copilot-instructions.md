# Copilot Instructions for MQTT PubSub Adapter

## Project Overview

This is a **Neon.js hybrid Rust/JavaScript project** that wraps the high-performance RMQTT Rust MQTT server library for Node.js. The project provides a JavaScript API for creating and managing MQTT brokers with support for multiple protocols (TCP, TLS, WebSocket, WSS) and concurrent connections.

## Architecture & Key Patterns

### Rust-JavaScript Bridge Pattern
- **Rust side**: `src/lib.rs` exports MQTT server functions via `#[neon::main]` using naming convention `mqtt_server_*`
- **JavaScript side**: `index.js` wraps native functions in an idiomatic `MqttServer` class with lifecycle management
- **Memory management**: Uses `JsBox<MqttServerWrapper>` to safely pass Rust server instances to JavaScript scope

### Async Server Pattern with Threading
- **Core design**: RMQTT server runs on dedicated Tokio runtime to avoid blocking Node.js event loop
- **Communication**: Uses `mpsc::channel` for JavaScript→Rust commands and `neon::event::Channel` for Rust→JavaScript callbacks
- **Server lifecycle**: Supports start/stop operations with graceful shutdown and configuration validation

```rust
// Key pattern: Server lifecycle management
server.start_with_config(config, deferred, |channel, deferred| {
    deferred.settle_with(channel, |mut cx| {
        // Server started successfully
        Ok(cx.undefined())
    });
})
```

### MQTT Configuration Pattern
- **Multi-listener support**: Each listener has protocol (tcp/tls/ws/wss), address, port, and TLS settings
- **Protocol flexibility**: Automatic protocol handler selection based on configuration
- **TLS validation**: Enforces certificate/key requirements for secure protocols

## Development Workflow

### Build & Test Commands
```bash
# Build the native module (compiles Rust → index.node)
npm run build

# Run tests (includes port listening validation)
npm test

# Install/rebuild after Rust changes
npm install
```

### Key Dependencies
- **Rust**: `neon = "1"` (JS bindings), `rmqtt = "0.16"` (MQTT server), `tokio = "1"` (async runtime)
- **Node.js**: `cargo-cp-artifact` (build artifact copying), `mocha` (testing)

## Critical Implementation Details

### Server Lifecycle Management
- **Thread safety**: Each server instance runs on a dedicated Tokio runtime in a separate thread
- **Graceful shutdown**: `stop()` method properly terminates server before allowing restart
- **Resource cleanup**: Call `server.close()` in tests to prevent hanging processes
- **State tracking**: JavaScript layer tracks `isRunning` state to prevent double-start

### Configuration Validation
- **Required fields**: Each listener must have name, port, and valid protocol
- **TLS requirements**: TLS/WSS protocols require both certificate and key paths
- **Port validation**: Ensures port numbers are in valid range (1-65535)
- **Protocol enforcement**: Only allows tcp, tls, ws, wss protocols

### Error Handling Pattern
- **Configuration errors**: Thrown synchronously during `start()` call for immediate feedback
- **Runtime errors**: RMQTT errors converted to Promise rejections via `SendResultExt` trait
- **Channel errors**: Database-style error handling adapted for server message passing

### Testing Conventions
- **Port probing**: Tests verify actual port listening using `net.Socket` connections
- **Non-standard ports**: Use ports like 18830+ to avoid conflicts with standard MQTT ports
- **Async cleanup**: Always call `server.close()` after tests to prevent process hanging
- **Error scenarios**: Test invalid configurations and double-start prevention

## File Structure Patterns
- `src/lib.rs`: Core Rust implementation wrapping RMQTT with Neon exports
- `index.js`: JavaScript wrapper providing idiomatic Promise-based MQTT server API
- `test/db.test.js`: Server lifecycle tests covering startup, configuration validation, and port binding
- `docs/DOC_RMQTT_LIB_MODE.md`: Upstream RMQTT documentation and examples
- `Cargo.toml`: Rust dependencies with RMQTT features (ws, tls, plugin)
- `package.json`: NPM metadata for MQTT server package

## Common Modifications
- **Adding listener protocols**: Extend protocol validation in JavaScript and add new protocol handlers in Rust
- **Plugin support**: Extend `ServerConfig` to include plugin configuration and registration
- **Authentication**: Add listener-level authentication options beyond `allowAnonymous`
- **Monitoring**: Add server status and metrics endpoints by exposing RMQTT's monitoring features
- **Configuration presets**: Add more helper methods like `createBasicConfig()` for common setups

## RMQTT Integration Points
- **ServerContext**: Manages plugins and global configuration
- **Builder pattern**: Fluent API for configuring individual listeners
- **Multi-protocol**: Each listener independently configured for different protocols
- **Plugin system**: Optional plugin loading from configuration directory or inline config