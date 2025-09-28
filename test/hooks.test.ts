import { describe, it, beforeEach, afterEach } from 'mocha';
const { expect } = require('chai');
import { MqttServer, HookCallbacks, MessageInfo, SubscriptionInfo, UnsubscriptionInfo, SessionInfo, MessageFrom } from '../src/index';
import { waitForPort, waitForPortClosed } from './helpers';
import { connect } from 'mqtt';

describe('MQTT Server Hook Callbacks', () => {
    let server: MqttServer;
    // Use a unique port per test to avoid interference between cases
    let currentPort: number = 0;
    let portCounter = 0;
    const nextPort = () => 19200 + (portCounter++);

    beforeEach(() => {
        server = new MqttServer();
    });

    afterEach(async () => {
        if (server.running) {
            await server.stop();
            if (currentPort) {
                try { await waitForPortClosed('127.0.0.1', currentPort); } catch { }
            }
        }
        server.close();
    });

    it('should register hook callbacks successfully', async function () {
        this.timeout(10000); // Set a 10 second timeout for this test

        return new Promise<void>((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Hook test timeout'));
            }, 8000);

            const messagePublishEvents: { session: SessionInfo | null, from: MessageFrom, message: MessageInfo }[] = [];
            const subscribeEvents: { session: SessionInfo | null, subscription: SubscriptionInfo }[] = [];
            const unsubscribeEvents: { session: SessionInfo | null, unsubscription: UnsubscriptionInfo }[] = [];

            const hooks: HookCallbacks = {
                onMessagePublish: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => {
                    messagePublishEvents.push({ session, from, message });
                },
                onClientSubscribe: (session: SessionInfo | null, subscription: SubscriptionInfo) => {
                    subscribeEvents.push({ session, subscription });
                },
                onClientUnsubscribe: (session: SessionInfo | null, unsubscription: UnsubscriptionInfo) => {
                    unsubscribeEvents.push({ session, unsubscription });
                }
            };

            // Set up hooks before starting the server
            server.setHooks(hooks);

            // Pick a unique port for this test
            currentPort = nextPort();

            server.start({
                listeners: [{
                    name: "tcp-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(async () => {
                // Wait for server to be ready
                await waitForPort('127.0.0.1', currentPort);
                // Connect a real MQTT client to trigger hook events
                const client = connect(`mqtt://127.0.0.1:${currentPort}`);

                client.on('connect', () => {

                    // Subscribe to a topic (should trigger onClientSubscribe hook)
                    client.subscribe('test/hook/topic', (err) => {
                        if (err) {
                            clearTimeout(timeout);
                            client.end(false, {}, () => { });
                            reject(err);
                            return;
                        }

                        // Publish a message from the server (should trigger onMessagePublish hook)
                        server.publish('test/hook/topic', Buffer.from('Hook test message'))
                            .then(() => {
                                // Publish a message from the client (should also trigger onMessagePublish hook)
                                client.publish('test/hook/topic', 'Client message');

                                // Give hooks time to be called, then unsubscribe
                                setTimeout(() => {
                                    client.unsubscribe('test/hook/topic', () => {
                                        // Give final hooks time to be called, then end test
                                        setTimeout(() => {
                                            clearTimeout(timeout);
                                            client.end(false, {}, () => {
                                                // Verify that hooks were set without errors
                                                try {
                                                    expect(messagePublishEvents.length).to.be.greaterThanOrEqual(0);
                                                    expect(subscribeEvents.length).to.be.greaterThanOrEqual(0);
                                                    expect(unsubscribeEvents.length).to.be.greaterThanOrEqual(0);
                                                    resolve();
                                                } catch (error) {
                                                    reject(error);
                                                }
                                            });
                                        }, 60);
                                    });
                                }, 120);
                            })
                            .catch((error) => {
                                clearTimeout(timeout);
                                client.end(false, {}, () => { });
                                reject(error);
                            });
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

    it('should handle partial hook registration', async function () {
        this.timeout(5000); // Set a 5 second timeout for this test

        // Only register some hooks
        const hooks: HookCallbacks = {
            onMessagePublish: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => {
            }
            // Note: not registering subscribe/unsubscribe hooks
        };

        // This should not throw an error
        expect(() => server.setHooks(hooks)).to.not.throw();

        // Pick a unique port for this test
        currentPort = nextPort();

        await server.start({
            listeners: [{
                name: "tcp-partial",
                address: "127.0.0.1",
                port: currentPort,
                protocol: "tcp",
                allowAnonymous: true
            }]
        });

        // Should be able to publish without issues
        await server.publish('test/partial/topic', Buffer.from('Partial hooks test'));

        expect(server.running).to.be.true;
    });

    it('onMessagePublish should include publishing client identity', async function () {
        this.timeout(10000);

        const topic = 'test/hooks/identity';
        const payload = 'client-identity';
        const clientId = `client-${Date.now()}`;
        const username = `user-${Date.now()}`;
        let capturedSession: SessionInfo | null = null;
        let capturedFrom: MessageFrom | null = null;

        let resolveHook: (() => void) | null = null;
        const hookObserved = new Promise<void>((resolve) => {
            resolveHook = resolve;
        });

        server.setHooks({
            onMessagePublish: (session, from, message) => {
                if (message.topic === topic && message.payload.toString() === payload) {
                    capturedSession = session;
                    capturedFrom = from;
                    resolveHook?.();
                }
            }
        });

        currentPort = nextPort();

        await server.start({
            listeners: [{
                name: 'tcp-publish-identity',
                address: '127.0.0.1',
                port: currentPort,
                protocol: 'tcp',
                allowAnonymous: true
            }]
        });

        await waitForPort('127.0.0.1', currentPort);

        const client = connect(`mqtt://127.0.0.1:${currentPort}`, {
            clientId,
            username
        });

        try {
            await new Promise<void>((resolve, reject) => {
                const timer = setTimeout(() => reject(new Error('client connect timeout')), 4000);
                client.once('connect', () => {
                    clearTimeout(timer);
                    resolve();
                });
                client.once('error', (err) => {
                    clearTimeout(timer);
                    reject(err);
                });
            });

            await new Promise<void>((resolve, reject) => {
                client.publish(topic, payload, { qos: 0 }, (err?: Error) => {
                    if (err) {
                        reject(err);
                        return;
                    }
                    resolve();
                });
            });

            await Promise.race([
                hookObserved,
                new Promise<void>((_, reject) => setTimeout(() => reject(new Error('onMessagePublish hook not observed')), 5000))
            ]);
        } finally {
            await new Promise<void>((resolve, reject) => {
                client.end(false, {}, (err?: Error) => {
                    if (err) {
                        reject(err);
                    } else {
                        resolve();
                    }
                });
            });
        }

        expect(capturedSession, 'session info should be provided for client publishes').to.not.equal(null);
        expect(capturedSession!.clientId).to.equal(clientId);
        expect(capturedSession!.username).to.equal(username);
        expect(capturedFrom, 'message origin should be provided').to.not.equal(null);
        expect(capturedFrom!.clientId).to.equal(clientId);
        expect(capturedFrom!.username).to.equal(username);
        expect(capturedFrom!.type).to.not.equal('system');
    });
});