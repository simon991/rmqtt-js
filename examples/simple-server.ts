#!/usr/bin/env node

/**
 * Simple MQTT Server Example
 * 
 * Demonstrates the full pub/sub API including:
 * - Real-time MQTT event hooks
 * - Server-side message publishing  
 * - QoS levels and message retention
 * - Message processing and forwarding
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
import type { ServerConfig } from '../dist/index.js';

const { MqttServer, QoS } = MqttModule;

/**
 * Simple MQTT Server Example
 * 
 * This example demonstrates how to create and manage an MQTT server
 * using the mqtt-pubsub-adapter library with full pub/sub functionality.
 * 
 * Features demonstrated:
 * - Basic server setup with real-time event hooks
 * - Publishing messages from the server
 * - Monitoring client subscriptions and message publishes
 * - Periodic heartbeat publishing
 * - Graceful shutdown handling
 * 
 * Also available: pubSubExample() for advanced message processing patterns
 */

async function main() {
    console.log('🚀 Starting MQTT Server Example');

    // Create a new MQTT server instance
    const server = new MqttServer();
    let heartbeatInterval: NodeJS.Timeout | null = null;


    // Set up graceful shutdown
    const shutdown = async () => {
        console.log('\n⏹️  Shutting down server...');
        try {
            // Clean up interval
            if (heartbeatInterval) {
                clearInterval(heartbeatInterval);
            }

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
        // Example 1: Basic TCP server with pub/sub functionality
        console.log('\n📡 Starting basic TCP MQTT server on port 1883...');

        const basicConfig: ServerConfig = {
            listeners: [{
                name: "tcp",
                address: "0.0.0.0",
                port: 1883,
                protocol: 'tcp',
                allowAnonymous: false  // Require authentication
            }],
            pluginsConfigDir: './examples/plugins',
            pluginsDefaultStartups: []
        };

        console.log('\n🔐 Authentication:');
        console.log('   - Authentication is implemented via JavaScript onClientAuthenticate hook');
        console.log('   - Anonymous connections are disabled to enforce authentication');
        console.log('\n📝 Authentication implemented via custom auth hook (see below)');
        console.log('\n📡 Client Connection Examples (password must be "demo"):');
        console.log('   mosquitto_pub -h localhost -p 1883 -u alice -P "demo" -t "alice/test" -m "Hello!"');
        console.log('   mosquitto_sub -h localhost -p 1883 -u alice -P "demo" -t "alice/+"');

        // Set up real-time event hooks to monitor MQTT activity and control ACLs
        server.setHooks({
            // Demo authentication hook: allow users when password === "demo".
            // In production, replace with real credential checks (e.g., DB, OAuth, JWT validation, etc.).
            onClientAuthenticate: (auth) => {
                const { username, password, clientId, remoteAddr } = auth;
                const allowed = Boolean(username) && password === 'demo';
                if (!allowed) {
                    console.warn(`Auth DENY: clientId=${clientId}, user=${username}, from=${remoteAddr}`);
                    return { allow: false, superuser: false, reason: 'Invalid credentials' };
                }
                console.log(`Auth ALLOW: clientId=${clientId}, user=${username}, from=${remoteAddr}`);
                return { allow: true, superuser: false, reason: 'Demo credentials accepted' };
            },
            // Subscribe authorization: only allow topics starting with "<username>/" and "server/public/#".
            // Also cap granted QoS to 1.
            onClientSubscribeAuthorize: (session, subscription) => {
                const user = session?.username ?? '';
                const tf = subscription.topicFilter;
                console.log(`SUB REQUEST: user=${user} filter=${tf} (requested QoS ${subscription.qos})`);
                const allow = tf.startsWith(`${user}/`) || tf.startsWith('server/public/');
                if (!allow) {
                    console.warn(`SUB DENY: user=${user} filter=${tf}`);
                    return { allow: false, reason: 'Not authorized for topic filter' };
                }
                console.log(`SUB ALLOW: user=${user} filter=${tf} (QoS<=1)`);
                return { allow: true, qos: QoS.AtLeastOnce };
            },
            onMessagePublish: (session, from, message) => {
                console.log(`📨 Message published to "${message.topic}": ${message.payload.toString()}`);
                console.log(`   - From: ${from.type} (${from.clientId})`);
                console.log(`   - QoS: ${message.qos}, Retain: ${message.retain}`);

                // Log potential ACL enforcement
                if (from.type === 'client' && from.username) {
                    const topicPrefix = message.topic.split('/')[0];
                    if (topicPrefix === from.username) {
                        console.log(`   ✅ ACL: User "${from.username}" correctly publishing to their topic space`);
                    } else {
                        console.log(`   ⚠️  ACL: User "${from.username}" publishing to "${message.topic}" - check ACL rules`);
                    }
                }
            },
            onMessageDelivered: (_session, from, message) => {
                console.log(`✅ Delivered: ${message.topic} (from ${from.type})`);
            },
            onMessageAcked: (_session, from, message) => {
                console.log(`📝 Acked: ${message.topic} (from ${from.type})`);
            },
            onMessageDropped: (_session, from, message, info) => {
                console.warn(`🗑️  Dropped: ${message.topic} (from ${from?.type ?? 'unknown'}) reason=${info?.reason ?? 'n/a'}`);
            },
            onClientSubscribe: (session, subscription) => {
                console.log(`📝 Client subscribed to: "${subscription.topicFilter}" (QoS ${subscription.qos})`);

                // Note: The actual ACL enforcement happens at the RMQTT level before this hook is called
                // If we see this message, it means the subscription was allowed by ACL
                if (session && session.username) {
                    const topicPrefix = subscription.topicFilter.split('/')[0];
                    if (topicPrefix === session.username) {
                        console.log(`   ✅ ACL: User "${session.username}" subscribed to their topic space`);
                    } else if (subscription.topicFilter.startsWith('server/')) {
                        console.log(`   ✅ ACL: User "${session.username}" subscribed to allowed server topic`);
                    }
                }
            },
            onClientUnsubscribe: (session, unsubscription) => {
                console.log(`📤 Client unsubscribed from: "${unsubscription.topicFilter}"`);
            }
        });

        await server.start(basicConfig);
        console.log('✅ Server started successfully!');
        console.log(`   - Protocol: TCP`);
        console.log(`   - Address: 0.0.0.0:1883`);
        console.log(`   - Anonymous connections: disabled`);
        console.log(`   - Real-time event hooks: active`);

        console.log('\n🔒 Subscribe ACL demo:');
        console.log('   - Allowed: username "alice" -> topics "alice/#" and "server/public/#"');
        console.log('   - Denied: any other topic filters');
        console.log('   - Granted QoS is capped to 1 even if client requests 2');
        console.log('\nTry:');
        console.log('   mosquitto_sub -h localhost -p 1883 -u alice -P "demo" -t "alice/#"');
        console.log('   mosquitto_sub -h localhost -p 1883 -u alice -P "demo" -t "server/public/#"');
        console.log('   mosquitto_sub -h localhost -p 1883 -u alice -P "demo" -t "server/secret/#"   # should be denied');

        // Log server status
        console.log(`\n📊 Server status: ${server.running ? 'Running' : 'Stopped'}`);

        // Demonstrate server-side publishing
        console.log('\n🔄 Publishing example messages from server...');

        // Publish a welcome message
        await server.publish('server/status', 'Server started successfully', {
            qos: QoS.AtLeastOnce,
            retain: true
        });

        // Publish some example sensor data
        setTimeout(async () => {
            const sensorData = {
                temperature: 23.5,
                humidity: 65.2,
                timestamp: Date.now()
            };
            await server.publish('sensors/environment', JSON.stringify(sensorData));
        }, 200);

        // Publish a periodic heartbeat
        heartbeatInterval = setInterval(async () => {
            if (server.running) {
                await server.publish('server/heartbeat', new Date().toISOString(), {
                    qos: QoS.AtMostOnce
                });
            } else {
                if (heartbeatInterval) clearInterval(heartbeatInterval);
            }
        }, 30000); // Every 30 seconds

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

// Example 2: Advanced pub/sub with message processing
async function pubSubExample() {
    console.log('\n🔄 Advanced Pub/Sub Example');

    const server = new MqttServer();

    // Set up intelligent message processing hooks
    server.setHooks({
        onMessagePublish: (session, from, message) => {
            console.log(`📨 Processing message: ${message.topic}`);

            // Automatically process sensor data
            if (message.topic.startsWith('sensors/')) {
                try {
                    const data = JSON.parse(message.payload.toString());

                    // Add processing timestamp
                    const processedData = {
                        ...data,
                        processed_at: new Date().toISOString(),
                        processed_by: 'rmqtt-server'
                    };

                    // Republish to processed topic
                    const processedTopic = message.topic.replace('sensors/', 'processed/');
                    server.publish(processedTopic, JSON.stringify(processedData), {
                        qos: QoS.AtLeastOnce,
                        retain: true
                    }).catch(err => console.error('Error republishing:', err));

                    console.log(`✅ Processed and republished to: ${processedTopic}`);
                } catch (error) {
                    console.error(`❌ Error processing sensor data from ${message.topic}:`, error);
                }
            }

            // Alert on critical messages
            if (message.topic.includes('alert') || message.topic.includes('critical')) {
                server.publish('alerts/all', `ALERT: ${message.topic} - ${message.payload.toString()}`, {
                    qos: QoS.ExactlyOnce,
                    retain: false
                }).catch(err => console.error('Error sending alert:', err));

                console.log('🚨 Critical alert forwarded to alerts/all');
            }
        },

        onClientSubscribe: (session, subscription) => {
            console.log(`👤 New subscription: ${subscription.topicFilter}`);

            // Send welcome message to new subscribers
            if (subscription.topicFilter.startsWith('welcome/')) {
                setTimeout(() => {
                    server.publish(subscription.topicFilter, JSON.stringify({
                        type: 'welcome',
                        message: 'Welcome to the MQTT server!',
                        server_time: new Date().toISOString(),
                        features: ['real-time hooks', 'message processing', 'QoS support']
                    }), {
                        qos: subscription.qos
                    }).catch(err => console.error('Error sending welcome:', err));
                }, 100);
            }
        },

        onClientUnsubscribe: (session, unsubscription) => {
            console.log(`👋 Client unsubscribed: ${unsubscription.topicFilter}`);
        }
    });

    const config = MqttServer.createBasicConfig(1884, '0.0.0.0'); // Different port
    await server.start(config);

    console.log('✅ Advanced pub/sub server started on port 1884');
    console.log('📝 Try these commands to see message processing:');
    console.log('   mosquitto_pub -h localhost -p 1884 -t "sensors/temp" -m \'{"value": 25.5, "unit": "C"}\'');
    console.log('   mosquitto_pub -h localhost -p 1884 -t "alerts/critical" -m "System overheating"');
    console.log('   mosquitto_sub -h localhost -p 1884 -t "welcome/test"  # Then subscribe to see welcome message');
    console.log('   mosquitto_sub -h localhost -p 1884 -t "processed/+" -v  # See processed messages');

    return server;
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

export { main, pubSubExample, multiProtocolExample };
