import { describe, it, beforeEach, afterEach } from 'mocha';
import { expect } from 'chai';
import { MqttServer, HookCallbacks } from '../index';
import { connect } from 'mqtt';

describe('MQTT JavaScript Hook Callbacks', () => {
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

    it('should actually call JavaScript hook functions when MQTT events occur', async function() {
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
                    console.log('✅ JavaScript onMessagePublish callback called!', {
                        topic: message.topic,
                        payload: message.payload.toString(),
                        from: from.type
                    });
                    messagePublishCalled = true;
                },
                onClientSubscribe: (session, subscription) => {
                    console.log('✅ JavaScript onClientSubscribe callback called!', {
                        topicFilter: subscription.topicFilter,
                        qos: subscription.qos
                    });
                    clientSubscribeCalled = true;
                },
                onClientUnsubscribe: (session, unsubscription) => {
                    console.log('✅ JavaScript onClientUnsubscribe callback called!', {
                        topicFilter: unsubscription.topicFilter
                    });
                    clientUnsubscribeCalled = true;
                    
                    // Test completion condition: all three callback types have been called
                    setTimeout(() => {
                        clearTimeout(timeout);
                        try {
                            expect(messagePublishCalled).to.be.true;
                            expect(clientSubscribeCalled).to.be.true;
                            expect(clientUnsubscribeCalled).to.be.true;
                            console.log('🎉 All JavaScript hook callbacks were successfully invoked!');
                            resolve();
                        } catch (error) {
                            reject(error);
                        }
                    }, 50);
                }
            };

            // Set up hooks
            server.setHooks(hooks);

            server.start({
                listeners: [{
                    name: "tcp-callback-test",
                    address: "127.0.0.1",
                    port: 1885,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(() => {
                setTimeout(() => {
                    const client = connect('mqtt://127.0.0.1:1885');

                    client.on('connect', () => {
                        console.log('Test client connected for callback verification');
                        
                        // Subscribe to trigger onClientSubscribe
                        client.subscribe('test/callback/topic', (err) => {
                            if (err) {
                                clearTimeout(timeout);
                                client.end(false, {}, () => {});
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
                                                client.end(false, {}, () => {});
                                            });
                                        }, 500);
                                    })
                                    .catch((error) => {
                                        clearTimeout(timeout);
                                        client.end(false, {}, () => {});
                                        reject(error);
                                    });
                            }, 300);
                        });
                    });

                    client.on('error', (error) => {
                        clearTimeout(timeout);
                        client.end(false, {}, () => {});
                        reject(error);
                    });
                }, 200);
            }).catch((error) => {
                clearTimeout(timeout);
                reject(error);
            });
        });
    });
});