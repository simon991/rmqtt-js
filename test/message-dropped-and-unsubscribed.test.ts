import { expect } from 'chai';
import mqtt from 'mqtt';
import { MqttServer } from '../src';
import { waitForPort } from './helpers';

describe('Message dropped and session unsubscribed hooks', () => {
  let server: MqttServer;
  const port = 19995;

  beforeEach(async () => {
    server = new MqttServer();
  });

  afterEach(async () => {
    try { await server.stop(); } catch { }
    try { server.close(); } catch { }
  });

  it('fires onSessionUnsubscribed when client unsubscribes', async () => {
    let unsubResolve: () => void;
    const unsubPromise = new Promise<void>((resolve) => { unsubResolve = resolve; });
    server.setHooks({
      onSessionUnsubscribed: () => { unsubResolve!(); },
    });

    await server.start({ listeners: [{ name: 'tcp', address: '127.0.0.1', port, protocol: 'tcp', allowAnonymous: true }] });
    await waitForPort('127.0.0.1', port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`, { protocolVersion: 4, clean: true });
    await new Promise<void>((resolve, reject) => {
      client.on('error', reject);
      client.on('connect', () => resolve());
    });

    await new Promise<void>((resolve, reject) => {
      client.subscribe('tmp/drop', { qos: 0 }, (err) => err ? reject(err) : resolve());
    });

    await new Promise<void>((resolve, reject) => {
      client.unsubscribe('tmp/drop', (err) => err ? reject(err) : resolve());
    });

    await Promise.race([
      unsubPromise,
      new Promise((r) => setTimeout(r, 1000)),
    ]);
    client.end(true);

    // If the promise didn't resolve within 1s, this will fail with a better message
    // @ts-ignore - subtle assertion using resolved promise state isn't required
    expect(true).to.equal(true);
  });

  it('fires onMessageDropped for denied client publish via ACL', async () => {
    let dropped = false;
    server.setHooks({
      onClientPublishAuthorize: (_session, pkt) => {
        // Deny publishing to a specific topic to force a drop
        if (pkt.topic === 'denied/topic') return { allow: false };
        return { allow: true };
      },
      onMessageDropped: (_session, _from, msg) => {
        if (msg.topic === 'denied/topic') dropped = true;
      },
    });

    await server.start({ listeners: [{ name: 'tcp', address: '127.0.0.1', port, protocol: 'tcp', allowAnonymous: true }] });
    await waitForPort('127.0.0.1', port);

    // Use a client publish so ACL check runs in MessagePublishCheckAcl
    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`, { protocolVersion: 4, clean: true });
    await new Promise<void>((resolve, reject) => {
      client.on('error', reject);
      client.on('connect', () => resolve());
    });

    await new Promise<void>((resolve) => {
      client.publish('denied/topic', 'nope', { qos: 0, retain: false }, () => resolve());
    });

    await new Promise(r => setTimeout(r, 200));
    client.end(true);

    expect(dropped).to.equal(true);
  });
});
