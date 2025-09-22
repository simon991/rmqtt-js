import { expect } from 'chai';
import mqtt from 'mqtt';
import { MqttServer } from '../src';
import { waitForPort } from './helpers';

describe('Connack emissions (deny) are not duplicated', () => {
  let server: MqttServer;
  const port = 19996;

  beforeEach(async () => {
    server = new MqttServer();
  });

  afterEach(async () => {
    try { await server.stop(); } catch {}
    try { server.close(); } catch {}
  });

  it('emits onClientConnack exactly once on auth deny', async () => {
    let count = 0;
    server.setHooks({
      onClientAuthenticate: () => ({ allow: false }),
      onClientConnack: () => { count++; },
    });

    await server.start({ listeners: [{ name: 'tcp', address: '127.0.0.1', port, protocol: 'tcp', allowAnonymous: true }] });
    await waitForPort('127.0.0.1', port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`, { clientId: 'deny-' + Date.now(), reconnectPeriod: 0 });
    await new Promise<void>((resolve) => {
      client.once('error', () => resolve());
      client.once('close', () => resolve());
      setTimeout(() => resolve(), 200);
    });

    await new Promise((r) => setTimeout(r, 100));
    try { client.end(true); } catch {}

    expect(count).to.equal(1);
  });
});
