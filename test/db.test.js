"use strict";

const assert = require("assert");
const net = require("net");

const MqttServer = require("..");

describe("MQTT Server", () => {
    it("should create and start basic TCP server", async () => {
        const server = new MqttServer();
        const config = MqttServer.createBasicConfig(18830); // Use non-standard port for testing

        assert.strictEqual(server.running, false);

        await server.start(config);
        assert.strictEqual(server.running, true);

        // Test that the port is actually listening
        const isListening = await checkPortListening("127.0.0.1", 18830);
        assert.strictEqual(isListening, true, "Server should be listening on port 18830");

        await server.stop();
        assert.strictEqual(server.running, false);

        server.close();
    });

    it("should reject invalid configuration", async () => {
        const server = new MqttServer();

        // Test empty config
        await assert.rejects(
            () => server.start({}),
            /Configuration must include at least one listener/
        );

        // Test invalid protocol
        await assert.rejects(
            () => server.start({
                listeners: [{
                    name: "test",
                    port: 1883,
                    protocol: "invalid"
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
        const config = MqttServer.createBasicConfig(18831);

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

        await assert.rejects(
            () => server.start(MqttServer.createBasicConfig(18832)),
            /Deferred.*dropped.*without.*being.*settled/
        );
    });
});

/**
 * Helper function to check if a port is listening
 * @param {string} host 
 * @param {number} port 
 * @returns {Promise<boolean>}
 */
function checkPortListening(host, port) {
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
