import { describe, it, beforeEach, afterEach } from 'mocha';
const { expect } = require('chai');
import { MqttServer } from '../src/index';
import { waitForPort, waitForPortClosed } from './helpers';

describe('Simple Callback Test', () => {
    let server: MqttServer;
    // Use a unique port per test to avoid interference between cases
    let currentPort: number = 0;
    let portCounter = 0;
    const nextPort = () => 19500 + (portCounter++);

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

    it('should call a simple callback function', async function () {
        this.timeout(5000);

        return new Promise<void>((resolve, reject) => {
            let callbackCalled = false;

            // Set up a simple hook that just sets a flag
            const hooks = {
                onMessagePublish: () => {
                    callbackCalled = true;
                    setTimeout(() => {
                        try {
                            expect(callbackCalled).to.be.true;
                            resolve();
                        } catch (error) {
                            reject(error);
                        }
                    }, 40);
                }
            };

            server.setHooks(hooks);

            currentPort = nextPort();

            server.start({
                listeners: [{
                    name: "simple-test",
                    address: "127.0.0.1",
                    port: currentPort,
                    protocol: "tcp",
                    allowAnonymous: true
                }]
            }).then(async () => {
                // Wait for server to be ready then publish
                await waitForPort('127.0.0.1', currentPort);
                server.publish('test/simple', Buffer.from('test'))
                    .catch(reject);
            }).catch(reject);
        });
    });
});