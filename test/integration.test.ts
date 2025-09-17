import { describe, it, beforeEach, afterEach } from 'mocha';
import { expect } from 'chai';
import * as mqtt from 'mqtt';
import { MqttServer, ServerConfig } from '../index';

describe('MQTT Integration Tests', () => {
    let server: MqttServer;
    let client: mqtt.MqttClient;

    beforeEach(async () => {
        server = new MqttServer();
        
        const config: ServerConfig = {
            listeners: [{
                name: "tcp",
                address: "127.0.0.1",
                port: 1883,
                protocol: "tcp",
                allowAnonymous: true
            }]
        };

        await server.start(config);
        
        // Give the server a moment to fully start
        await new Promise(resolve => setTimeout(resolve, 100));
    });

    afterEach(async () => {
        if (client && client.connected) {
            await new Promise<void>((resolve) => {
                client.end(false, {}, () => resolve());
            });
        }
        if (server) {
            await server.close();
        }
    });

    it('should publish messages that can be received by MQTT clients', async () => {
        return new Promise<void>((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Test timeout'));
            }, 5000);

            // Connect an MQTT client to subscribe to a topic
            client = mqtt.connect('mqtt://127.0.0.1:1883');

            client.on('connect', () => {
                console.log('MQTT client connected');
                
                // Subscribe to a test topic
                client.subscribe('test/integration', (err: Error | null) => {
                    if (err) {
                        clearTimeout(timeout);
                        reject(err);
                        return;
                    }
                    
                    console.log('Client subscribed to test/integration');
                    
                    // Publish a message through our server API
                    server.publish('test/integration', 'Hello from API!')
                        .then(() => {
                            console.log('Message published through API');
                        })
                        .catch(reject);
                });
            });

            client.on('message', (topic: string, payload: Buffer) => {
                console.log(`Received message: ${topic} -> ${payload.toString()}`);
                
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
            client = mqtt.connect('mqtt://127.0.0.1:1883');

            client.on('connect', () => {
                console.log('MQTT client connected for multi-message test');
                
                // Subscribe to a test topic
                client.subscribe('test/multi', (err: Error | null) => {
                    if (err) {
                        clearTimeout(timeout);
                        reject(err);
                        return;
                    }
                    
                    console.log('Client subscribed to test/multi');
                    
                    // Publish multiple messages
                    Promise.all([
                        server.publish('test/multi', 'Message 1'),
                        server.publish('test/multi', 'Message 2'),
                        server.publish('test/multi', 'Message 3')
                    ]).catch(reject);
                });
            });

            client.on('message', (topic: string, payload: Buffer) => {
                console.log(`Multi-test received: ${topic} -> ${payload.toString()}`);
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