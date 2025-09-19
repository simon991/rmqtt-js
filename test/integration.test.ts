import { describe, it, beforeEach, afterEach } from 'mocha';
const { expect } = require('chai');
import * as mqtt from 'mqtt';
import { MqttServer, ServerConfig } from '../src/index';
import { waitForPort, waitForPortClosed } from './helpers';

describe('MQTT Integration Tests', () => {
    let server: MqttServer;
    let client: mqtt.MqttClient;
    // Use a unique port per test to avoid interference between cases
    let currentPort: number = 0;
    let portCounter = 0;
    const nextPort = () => 19300 + (portCounter++);

    beforeEach(async () => {
        server = new MqttServer();
        currentPort = nextPort();
        
        const config: ServerConfig = {
            listeners: [{
                name: "tcp",
                address: "127.0.0.1",
                port: currentPort,
                protocol: "tcp",
                allowAnonymous: true
            }]
        };

        await server.start(config);
        
    // Wait for server to be ready
    await waitForPort('127.0.0.1', currentPort);
    });

    afterEach(async () => {
        if (client && client.connected) {
            await new Promise<void>((resolve) => {
                client.end(false, {}, () => resolve());
            });
        }
        if (server) {
            // stop if running; then ensure port closed
            if (server.running) {
                await server.stop();
                if (currentPort) {
                    try { await waitForPortClosed('127.0.0.1', currentPort); } catch {}
                }
            }
            await server.close();
        }
    });

    it('should publish messages that can be received by MQTT clients', async () => {
        return new Promise<void>((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Test timeout'));
            }, 5000);

            // Connect an MQTT client to subscribe to a topic
            client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`);

            client.on('connect', () => {
                
                // Subscribe to a test topic
                client.subscribe('test/integration', (err: Error | null) => {
                    if (err) {
                        clearTimeout(timeout);
                        reject(err);
                        return;
                    }
                    
                    // Publish a message through our server API
                    server.publish('test/integration', 'Hello from API!')
                        .then(() => {})
                        .catch(reject);
                });
            });

            client.on('message', (topic: string, payload: Buffer) => {
                
                try {
                    expect(topic).to.equal('test/integration');
                    expect(payload.toString()).to.equal('Hello from API!');
                    clearTimeout(timeout);
                    resolve();
                } catch (error) {
                    clearTimeout(timeout);
                    reject(error);
                }
            });

            client.on('error', (error: Error) => {
                clearTimeout(timeout);
                reject(error);
            });
        });
    });

    it('should handle multiple clients and messages', async () => {
        return new Promise<void>((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Test timeout'));
            }, 10000);

            let receivedMessages = 0;
            const expectedMessages = 3;

            // Connect an MQTT client
            client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`);

            client.on('connect', () => {
                
                // Subscribe to a test topic
                client.subscribe('test/multi', (err: Error | null) => {
                    if (err) {
                        clearTimeout(timeout);
                        reject(err);
                        return;
                    }
                    
                    // Publish multiple messages
                    Promise.all([
                        server.publish('test/multi', 'Message 1'),
                        server.publish('test/multi', 'Message 2'),
                        server.publish('test/multi', 'Message 3')
                    ]).catch(reject);
                });
            });

            client.on('message', (topic: string, payload: Buffer) => {
                receivedMessages++;
                
                if (receivedMessages === expectedMessages) {
                    clearTimeout(timeout);
                    resolve();
                }
            });

            client.on('error', (error: Error) => {
                clearTimeout(timeout);
                reject(error);
            });
        });
    });
});