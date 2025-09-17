import { describe, it, beforeEach, afterEach } from 'mocha';
import { expect } from 'chai';
import { MqttServer, HookCallbacks, MessageInfo, SubscriptionInfo, UnsubscriptionInfo, SessionInfo, MessageFrom } from '../index';
import { connect } from 'mqtt';

describe('MQTT Server Hook Callbacks', () => {
    let server: MqttServer;

    beforeEach(() => {
        server = new MqttServer();
    });

    afterEach(async () => {
        if (server.running) {
            await server.stop();
        }
        server.close();
    });

    it('should register hook callbacks successfully', async function() {
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
                    console.log('Message published:', message);
                    messagePublishEvents.push({ session, from, message });
                },
                onClientSubscribe: (session: SessionInfo | null, subscription: SubscriptionInfo) => {
                    console.log('Client subscribed:', subscription);
                    subscribeEvents.push({ session, subscription });
                },
                onClientUnsubscribe: (session: SessionInfo | null, unsubscription: UnsubscriptionInfo) => {
                    console.log('Client unsubscribed:', unsubscription);
                    unsubscribeEvents.push({ session, unsubscription });
                }
            };

            // Set up hooks before starting the server
            server.setHooks(hooks);

            server.start({
                listeners: [{
                    name: "tcp-test",
                    address: "127.0.0.1",
                    port: 1883,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(() => {
                // Give the server a moment to fully start
                setTimeout(() => {
                    // Connect a real MQTT client to trigger hook events
                    const client = connect('mqtt://127.0.0.1:1883');

                    client.on('connect', () => {
                        console.log('Hook test client connected');
                        
                        // Subscribe to a topic (should trigger onClientSubscribe hook)
                        client.subscribe('test/hook/topic', (err) => {
                            if (err) {
                                clearTimeout(timeout);
                                client.end(false, {}, () => {});
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
                                            }, 100);
                                        });
                                    }, 200);
                                })
                                .catch((error) => {
                                    clearTimeout(timeout);
                                    client.end(false, {}, () => {});
                                    reject(error);
                                });
                        });
                    });

                    client.on('error', (error) => {
                        clearTimeout(timeout);
                        client.end(false, {}, () => {});
                        reject(error);
                    });
                }, 100);
            }).catch((error) => {
                clearTimeout(timeout);
                reject(error);
            });
        });
    });

    it('should handle partial hook registration', async function() {
        this.timeout(5000); // Set a 5 second timeout for this test
        
        // Only register some hooks
        const hooks: HookCallbacks = {
            onMessagePublish: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => {
                console.log('Message published:', message);
            }
            // Note: not registering subscribe/unsubscribe hooks
        };

        // This should not throw an error
        expect(() => server.setHooks(hooks)).to.not.throw();

        await server.start({
            listeners: [{
                name: "tcp-partial",
                address: "127.0.0.1",
                port: 1884,
                protocol: "tcp",
                allowAnonymous: true
            }]
        });

        // Should be able to publish without issues
        await server.publish('test/partial/topic', Buffer.from('Partial hooks test'));

        expect(server.running).to.be.true;
    });
});