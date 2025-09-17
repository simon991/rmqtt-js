"use strict";

const { mqttServerNew, mqttServerClose, mqttServerStart, mqttServerStop } = require("./index.node");

/**
 * High-performance MQTT server wrapper for Node.js
 * Built on top of the RMQTT Rust library using Neon bindings
 */
class MqttServer {
    constructor() {
        this.server = mqttServerNew();
        this.isRunning = false;
    }

    /**
     * Start the MQTT server with the given configuration
     * @param {Object} config - Server configuration
     * @param {Array} config.listeners - Array of listener configurations
     * @param {string} config.listeners[].name - Listener name
     * @param {string} config.listeners[].address - Bind address (default: "0.0.0.0")
     * @param {number} config.listeners[].port - Port number
     * @param {string} config.listeners[].protocol - Protocol: "tcp", "tls", "ws", "wss"
     * @param {string} [config.listeners[].tlsCert] - TLS certificate file path
     * @param {string} [config.listeners[].tlsKey] - TLS key file path
     * @param {boolean} [config.listeners[].allowAnonymous=true] - Allow anonymous connections
     * @param {string} [config.pluginsConfigDir] - Directory containing plugin configuration files
     * @returns {Promise<void>}
     */
    async start(config) {
        if (this.isRunning) {
            throw new Error("Server is already running");
        }

        // Validate configuration
        this._validateConfig(config);

        // Set default values
        const normalizedConfig = {
            listeners: config.listeners.map(listener => ({
                name: listener.name,
                address: listener.address || "0.0.0.0",
                port: listener.port,
                protocol: listener.protocol,
                tlsCert: listener.tlsCert,
                tlsKey: listener.tlsKey,
                allowAnonymous: listener.allowAnonymous !== false
            })),
            pluginsConfigDir: config.pluginsConfigDir,
        };

        await mqttServerStart.call(this.server, normalizedConfig);
        this.isRunning = true;
    }

    /**
     * Stop the MQTT server
     * @returns {Promise<void>}
     */
    async stop() {
        if (!this.isRunning) {
            return;
        }

        await mqttServerStop.call(this.server);
        this.isRunning = false;
    }

    /**
     * Close the server and clean up resources
     * Call this to allow the process to exit immediately instead of waiting for GC
     */
    close() {
        if (this.isRunning) {
            // Note: This will force-close without graceful shutdown
            this.isRunning = false;
        }
        mqttServerClose.call(this.server);
    }

    /**
     * Check if the server is currently running
     * @returns {boolean}
     */
    get running() {
        return this.isRunning;
    }

    _validateConfig(config) {
        if (!config || typeof config !== 'object') {
            throw new Error("Configuration must be an object");
        }

        if (!Array.isArray(config.listeners) || config.listeners.length === 0) {
            throw new Error("Configuration must include at least one listener");
        }

        for (const listener of config.listeners) {
            if (!listener.name || typeof listener.name !== 'string') {
                throw new Error("Each listener must have a name");
            }

            if (!listener.port || typeof listener.port !== 'number' || listener.port < 1 || listener.port > 65535) {
                throw new Error("Each listener must have a valid port number");
            }

            if (!listener.protocol || !['tcp', 'tls', 'ws', 'wss'].includes(listener.protocol)) {
                throw new Error("Each listener protocol must be one of: tcp, tls, ws, wss");
            }

            if ((listener.protocol === 'tls' || listener.protocol === 'wss')) {
                if (!listener.tlsCert || !listener.tlsKey) {
                    throw new Error("TLS/WSS listeners require both tlsCert and tlsKey");
                }
            }
        }
    }
}

/**
 * Create a basic MQTT server configuration
 * @param {number} port - Port to listen on (default: 1883)
 * @param {string} address - Address to bind to (default: "0.0.0.0")
 * @returns {Object} Basic configuration object
 */
MqttServer.createBasicConfig = function(port = 1883, address = "0.0.0.0") {
    return {
        listeners: [{
            name: "tcp",
            address,
            port,
            protocol: "tcp",
            allowAnonymous: true
        }]
    };
};

/**
 * Create a multi-protocol MQTT server configuration
 * @param {Object} options - Configuration options
 * @returns {Object} Multi-protocol configuration object
 */
MqttServer.createMultiProtocolConfig = function(options = {}) {
    const {
        tcpPort = 1883,
        tlsPort = 8883,
        wsPort = 8080,
        wssPort = 8443,
        address = "0.0.0.0",
        tlsCert,
        tlsKey,
        allowAnonymous = true
    } = options;

    const listeners = [
        {
            name: "tcp",
            address,
            port: tcpPort,
            protocol: "tcp",
            allowAnonymous
        },
        {
            name: "ws",
            address,
            port: wsPort,
            protocol: "ws",
            allowAnonymous
        }
    ];

    if (tlsCert && tlsKey) {
        listeners.push(
            {
                name: "tls",
                address,
                port: tlsPort,
                protocol: "tls",
                tlsCert,
                tlsKey,
                allowAnonymous
            },
            {
                name: "wss",
                address,
                port: wssPort,
                protocol: "wss",
                tlsCert,
                tlsKey,
                allowAnonymous
            }
        );
    }

    return { listeners };
};

module.exports = MqttServer;
