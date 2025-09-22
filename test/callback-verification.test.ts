import { describe, it, beforeEach, afterEach } from 'mocha';
const { expect } = require('chai');
import { MqttServer, HookCallbacks } from '../src/index';
import { waitForPort } from './helpers';
import { connect } from 'mqtt';

describe('MQTT JavaScript Hook Callbacks', () => {
    let server: MqttServer;
    // Use a unique port per test to avoid interference between cases
    let currentPort: number = 0;
    let portCounter = 0;
    const nextPort = () => 19100 + (portCounter++);

    beforeEach(() => {
        server = new MqttServer();
    });

    afterEach(async () => {
        if (server.running) {
            await server.stop();
        }
        server.close();
    });

    it('should actually call JavaScript hook functions when MQTT events occur', async function () {
        this.timeout(15000);

        return new Promise<void>((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Hook callback test timeout'));
            }, 12000);

            // Track whether callbacks were actually called
            let messagePublishCalled = false;
            let clientSubscribeCalled = false;
            let clientUnsubscribeCalled = false;

            const hooks: HookCallbacks = {
                onMessagePublish: (session, from, message) => {
                    messagePublishCalled = true;
                },
                onClientSubscribe: (session, subscription) => {
                    clientSubscribeCalled = true;
                },
                onClientUnsubscribe: (session, unsubscription) => {
                    clientUnsubscribeCalled = true;

                    // Test completion condition: all three callback types have been called
                    setTimeout(() => {
                        clearTimeout(timeout);
                        try {
                            expect(messagePublishCalled).to.be.true;
                            expect(clientSubscribeCalled).to.be.true;
                            expect(clientUnsubscribeCalled).to.be.true;
                            resolve();
                        } catch (error) {
                            reject(error);
                        }
                    }, 40);
                }
            };

            // Set up hooks
            server.setHooks(hooks);

            // Pick a unique port for this test
            currentPort = nextPort();

            server.start({
                listeners: [{
                    name: "tcp-callback-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(async () => {
                await waitForPort('127.0.0.1', currentPort);
                const client = connect(`mqtt://127.0.0.1:${currentPort}`);

                client.on('connect', () => {

                    // Subscribe to trigger onClientSubscribe
                    client.subscribe('test/callback/topic', (err) => {
                        if (err) {
                            clearTimeout(timeout);
                            client.end(false, {}, () => { });
                            reject(err);
                            return;
                        }

                        // Publish to trigger onMessagePublish
                        setTimeout(() => {
                            server.publish('test/callback/topic', Buffer.from('Callback test message'))
                                .then(() => {
                                    // Unsubscribe to trigger onClientUnsubscribe
                                    setTimeout(() => {
                                        client.unsubscribe('test/callback/topic', () => {
                                            client.end(false, {}, () => { });
                                        });
                                    }, 200);
                                })
                                .catch((error: any) => {
                                    clearTimeout(timeout);
                                    client.end(false, {}, () => { });
                                    reject(error);
                                });
                        }, 150);
                    });
                });

                client.on('error', (error) => {
                    clearTimeout(timeout);
                    client.end(false, {}, () => { });
                    reject(error);
                });

            }).catch((error) => {
                clearTimeout(timeout);
                reject(error);
            });
        });
    });
});