"use strict";

import * as assert from "assert";
import * as net from "net";
import { MqttServer, ServerConfig } from "../index";
import { waitForPort } from './helpers';

describe("MQTT Server", () => {
    // Use a unique port per test to avoid interference between cases
    let currentPort: number = 0;
    let portCounter = 0;
    const nextPort = () => 19400 + (portCounter++);
    it("should create and start basic TCP server", async () => {
        const server = new MqttServer();
        currentPort = nextPort();
        const config = MqttServer.createBasicConfig(currentPort); // Use unique port for testing

        assert.strictEqual(server.running, false);

        await server.start(config);
        assert.strictEqual(server.running, true);

    // Wait for server to be ready
    await waitForPort('127.0.0.1', currentPort);

        // Test that the port is actually listening
    const isListening = await checkPortListening("127.0.0.1", currentPort);
    assert.strictEqual(isListening, true, `Server should be listening on port ${currentPort}`);

        await server.stop();
        assert.strictEqual(server.running, false);

        server.close();
    });

    it("should allow publishing messages when server is running", async () => {
        const server = new MqttServer();
        currentPort = nextPort();
        const config = MqttServer.createBasicConfig(currentPort); // Use unique port

        await server.start(config);

        // Test publishing a message (should not throw)
        await server.publish("test/topic", "Hello, World!", { qos: 0, retain: false });
        await server.publish("test/topic", Buffer.from("Binary data"), { qos: 1 });

        await server.stop();
        server.close();
    });

    it("should reject publish when server is not running", async () => {
        const server = new MqttServer();

        await assert.rejects(
            () => server.publish("test/topic", "message"),
            /Server must be running to publish messages/
        );

        server.close();
    });

    it("should reject invalid configuration", async () => {
        const server = new MqttServer();

        // Test empty config
        await assert.rejects(
            () => server.start({} as ServerConfig),
            /Configuration must include at least one listener/
        );

        // Test invalid protocol
        await assert.rejects(
            () => server.start({
                listeners: [{
                    name: "test",
                    port: 1883,
                    protocol: "invalid" as any
                }]
            }),
            /protocol must be one of: tcp, tls, ws, wss/
        );

        // Test missing TLS configuration
        await assert.rejects(
            () => server.start({
                listeners: [{
                    name: "test",
                    port: 8883,
                    protocol: "tls"
                }]
            }),
            /TLS\/WSS listeners require both tlsCert and tlsKey/
        );

        server.close();
    });

    it("should prevent starting server twice", async () => {
        const server = new MqttServer();
        currentPort = nextPort();
        const config = MqttServer.createBasicConfig(currentPort);

        await server.start(config);

        await assert.rejects(
            () => server.start(config),
            /Server is already running/
        );

        await server.stop();
        server.close();
    });

    it("should handle stop on non-running server gracefully", async () => {
        const server = new MqttServer();

        // Should not throw
        await server.stop();

        server.close();
    });

    it("should create multi-protocol configuration", () => {
        const config = MqttServer.createMultiProtocolConfig({
            tcpPort: 1883,
            wsPort: 8080,
            tlsCert: "./test.pem",
            tlsKey: "./test.key"
        });

        assert.strictEqual(config.listeners.length, 4);
        assert.strictEqual(config.listeners[0].protocol, "tcp");
        assert.strictEqual(config.listeners[1].protocol, "ws");
        assert.strictEqual(config.listeners[2].protocol, "tls");
        assert.strictEqual(config.listeners[3].protocol, "wss");
    });

    it("should reject calls to stopped server", async () => {
        const server = new MqttServer();

        server.close();

        currentPort = nextPort();
        await assert.rejects(
            () => server.start(MqttServer.createBasicConfig(currentPort)),
            /Deferred.*dropped.*without.*being.*settled/
        );
    });
});

/**
 * Helper function to check if a port is listening
 * @param host - Host address to check
 * @param port - Port number to check
 * @returns Promise that resolves to true if port is listening
 */
function checkPortListening(host: string, port: number): Promise<boolean> {
    return new Promise((resolve) => {
        const socket = new net.Socket();
        
        const timeout = setTimeout(() => {
            socket.destroy();
            resolve(false);
        }, 1000);

        socket.setTimeout(1000);
        
        socket.on('connect', () => {
            clearTimeout(timeout);
            socket.destroy();
            resolve(true);
        });

        socket.on('timeout', () => {
            clearTimeout(timeout);
            socket.destroy();
            resolve(false);
        });

        socket.on('error', () => {
            clearTimeout(timeout);
            resolve(false);
        });

        socket.connect(port, host);
    });
}