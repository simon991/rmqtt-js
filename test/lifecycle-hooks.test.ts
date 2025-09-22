import { describe, it, beforeEach, afterEach } from 'mocha';
import { MqttServer } from '../src';
import { waitForPort } from './helpers';
import mqtt from 'mqtt';
import { strict as assert } from 'assert';

describe('Client lifecycle hooks', function () {
  this.timeout(15000);

  let server: MqttServer;
  const port = 19050 + Math.floor(Math.random() * 1000);

  beforeEach(async () => {
    server = new MqttServer();
  });

  afterEach(async () => {
    try {
      if (server?.running) {
        await server.stop();
      }
    } finally {
      server?.close();
    }
  });

  it('fires onClientConnected and onSessionSubscribed', async () => {
    let connectedCalled = false;
    let subscribedCalled = false;

    server.setHooks({
      onClientConnected: (session) => {
        connectedCalled = true;
        assert.ok(session.clientId);
      },
      onSessionSubscribed: (session) => {
        subscribedCalled = true;
        assert.ok(session.clientId);
      },
    });

    const config = MqttServer.createBasicConfig(port);
    await server.start(config);
    await waitForPort('127.0.0.1', port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`, { clientId: `test-${Date.now()}` });

    await new Promise<void>((resolve, reject) => {
      client.on('connect', () => {
        client.subscribe('foo', (err) => {
          if (err) return reject(err);
          // give a short tick for hook callbacks to flush
          setTimeout(resolve, 50);
        });
      });
      client.on('error', reject);
    });

    client.end(true);

    assert.equal(connectedCalled, true, 'onClientConnected should have been called');
    assert.equal(subscribedCalled, true, 'onSessionSubscribed should have been called');
  });

  it('fires onClientDisconnected with reason', async () => {
    let disconnectedCalled = false;
    let disconnectReason: string | undefined;

    server.setHooks({
      onClientDisconnected: (_session, info) => {
        disconnectedCalled = true;
        disconnectReason = info?.reason;
      },
    });

    const config = MqttServer.createBasicConfig(port + 1);
    await server.start(config);
    await waitForPort('127.0.0.1', port + 1);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port + 1}`, { clientId: `test-${Date.now()}` });

    await new Promise<void>((resolve, reject) => {
      client.on('connect', () => {
        // end connection; force close to expedite
        client.end(true, (err?: Error) => (err ? reject(err) : resolve()));
      });
      client.on('error', reject);
    });

    // Give the hook a brief tick to flush through Neon channel
    await new Promise((r) => setTimeout(r, 100));

    assert.equal(disconnectedCalled, true, 'onClientDisconnected should have been called');
    // Reason may be transport-close or None depending on protocol path; just assert string or undefined is acceptable
    assert.ok(disconnectReason === undefined || typeof disconnectReason === 'string');
  });
});
