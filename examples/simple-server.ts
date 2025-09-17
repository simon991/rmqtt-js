#!/usr/bin/env node

/**
 * Simple MQTT Server Example
 * 
 * Run this example:
 *   npm run example
 *   
 * Or directly (Node.js 20.6.0+):
 *   node examples/simple-server.ts --experimental-strip-types
 *   
 * Or compiled version:
 *   npm run build && node dist/examples/simple-server.js
 */

import * as MqttModule from '../dist/index.js';

const { MqttServer } = MqttModule;

/**
 * Simple MQTT Server Example
 * 
 * This example demonstrates how to create and manage an MQTT server
 * using the mqtt-pubsub-adapter library.
 */

async function main() {
    console.log('🚀 Starting MQTT Server Example');

    // Create a new MQTT server instance
    const server = new MqttServer();

    // Set up graceful shutdown
    const shutdown = async () => {
        console.log('\n⏹️  Shutting down server...');
        try {
            await server.stop();
            server.close();
            console.log('✅ Server stopped gracefully');
            process.exit(0);
        } catch (error) {
            console.error('❌ Error during shutdown:', error);
            process.exit(1);
        }
    };

    // Handle shutdown signals
    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);

    try {
        // Example 1: Basic TCP server
        console.log('\n📡 Starting basic TCP MQTT server on port 1883...');
        const basicConfig = MqttServer.createBasicConfig(1883, '0.0.0.0');

        await server.start(basicConfig);
        console.log('✅ Server started successfully!');
        console.log(`   - Protocol: TCP`);
        console.log(`   - Address: 0.0.0.0:1883`);
        console.log(`   - Anonymous connections: enabled`);

        // Log server status
        console.log(`\n📊 Server status: ${server.running ? 'Running' : 'Stopped'}`);

        // Example client connection instructions
        console.log('\n🔗 Connect to your MQTT server:');
        console.log('   Using mosquitto_pub/sub:');
        console.log('     mosquitto_pub -h localhost -p 1883 -t "test/topic" -m "Hello MQTT!"');
        console.log('     mosquitto_sub -h localhost -p 1883 -t "test/topic"');
        console.log('');
        console.log('   Using MQTT.js:');
        console.log('     const mqtt = require("mqtt");');
        console.log('     const client = mqtt.connect("mqtt://localhost:1883");');
        console.log('');
        console.log('   Using Python paho-mqtt:');
        console.log('     import paho.mqtt.client as mqtt');
        console.log('     client = mqtt.Client()');
        console.log('     client.connect("localhost", 1883, 60)');

        // Keep the server running
        console.log('\n⏳ Server is running... Press Ctrl+C to stop');

        // Simple keep-alive loop
        while (server.running) {
            await new Promise(resolve => setTimeout(resolve, 1000));
        }

    } catch (error) {
        console.error('❌ Error starting server:', error);

        // Try to clean up
        try {
            if (server.running) {
                await server.stop();
            }
            server.close();
        } catch (cleanupError) {
            console.error('❌ Error during cleanup:', cleanupError);
        }

        process.exit(1);
    }
}

// Example 3: Multi-protocol with TLS (requires certificates)
async function multiProtocolExample() {
    const server = new MqttServer();

    // Note: This requires actual certificate files
    // Generate self-signed certs for testing:
    // openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
    const multiConfig = MqttServer.createMultiProtocolConfig({
        tcpPort: 1883,
        wsPort: 8080,
        tlsPort: 8883,
        wssPort: 8443,
        address: "0.0.0.0",
        // tlsCert: "/path/to/cert.pem",    // Uncomment when you have certificates
        // tlsKey: "/path/to/key.pem",      // Uncomment when you have certificates
        allowAnonymous: true
    });

    await server.start(multiConfig);
    console.log('Multi-protocol server started (TCP + WebSocket only, no TLS without certs)');

    return server;
}

// Run the example (ES module main check)
if (process.argv[1] && process.argv[1].endsWith('simple-server.ts')) {
    main().catch(error => {
        console.error('❌ Unhandled error:', error);
        process.exit(1);
    });
}

export { main, multiProtocolExample };
