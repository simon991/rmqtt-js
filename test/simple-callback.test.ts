import { describe, it, beforeEach, afterEach } from 'mocha';
import { expect } from 'chai';
import { MqttServer } from '../index';

describe('Simple Callback Test', () => {
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

    it('should call a simple callback function', async function() {
        this.timeout(5000);
        
        return new Promise<void>((resolve, reject) => {
            let callbackCalled = false;
            
            // Set up a simple hook that just sets a flag
            const hooks = {
                onMessagePublish: () => {
                    console.log('✅ Simple callback was called!');
                    callbackCalled = true;
                    setTimeout(() => {
                        try {
                            expect(callbackCalled).to.be.true;
                            resolve();
                        } catch (error) {
                            reject(error);
                        }
                    }, 50);
                }
            };

            server.setHooks(hooks);
            
            server.start({
                listeners: [{
                    name: "simple-test",
                    address: "127.0.0.1",
                    port: 1886,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(() => {
                // Just publish a message to trigger the hook
                setTimeout(() => {
                    server.publish('test/simple', Buffer.from('test'))
                        .catch(reject);
                }, 200);
            }).catch(reject);
        });
    });
});