import { strict as assert } from 'assert';
import mqtt from 'mqtt';
import { MqttServer } from '../src';
import { waitForPort } from './helpers';

describe('Session created/terminated hooks', function () {
  this.timeout(15000);

  let server: MqttServer;
  const port = 19994;

  beforeEach(async () => {
    server = new MqttServer();
  });

  afterEach(async () => {
    try { await server.stop(); } catch {}
    try { server.close(); } catch {}
  });

  it('fires onSessionCreated and onSessionTerminated', async () => {
    let created = false;
    let terminated = false;
    server.setHooks({
      onSessionCreated: (session) => { created = true; assert.ok(session.clientId); },
      onSessionTerminated: (session) => { terminated = true; assert.ok(session.clientId); },
    });

    await server.start({ listeners: [{ name: 'tcp', address: '127.0.0.1', port, protocol: 'tcp', allowAnonymous: true }] });
    await waitForPort('127.0.0.1', port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`, { clientId: 'sess-' + Date.now(), protocolVersion: 4, clean: true });
    await new Promise<void>((resolve, reject) => {
      client.on('error', reject);
      client.on('connect', () => resolve());
    });

    await new Promise(r => setTimeout(r, 100));
    client.end(true);
    await new Promise(r => setTimeout(r, 150));

    assert.equal(created, true);
    assert.equal(terminated, true);
  });
});
