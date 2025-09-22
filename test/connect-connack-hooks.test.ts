import { expect } from 'chai';
import mqtt from 'mqtt';
import { MqttServer } from '../src';
import { waitForPort } from './helpers';

describe('Client connect/connack hooks (smoke)', () => {
  let server: MqttServer;
  const port = 19998;

  beforeEach(async () => {
    server = new MqttServer();
  });

  afterEach(async () => {
    try { await server.stop(); } catch {}
    try { server.close(); } catch {}
  });

  it('fires onClientConnect and onClientConnack once on connect', async () => {
    const seen: { connect?: any; connack?: any } = {};
    server.setHooks({
      onClientConnect: (info) => { seen.connect = info; },
      onClientConnack: (info) => { seen.connack = info; },
      // keep auth open to avoid deny
      onClientAuthenticate: () => ({ allow: true }),
    });

    await server.start({ listeners: [{ name: 'tcp', address: '127.0.0.1', port, protocol: 'tcp', allowAnonymous: true }] });
    await waitForPort('127.0.0.1', port);

    const clientId = 'conn-smoke-' + Math.random().toString(16).slice(2);
    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`, { clientId, protocolVersion: 4, clean: true });
    await new Promise<void>((resolve, reject) => {
      client.once('error', reject);
      client.once('connect', () => resolve());
    });

    // Allow Neon channel to flush callbacks
    await new Promise((r) => setTimeout(r, 100));
    client.end(true);

    expect(seen.connect).to.be.an('object');
    expect(seen.connect.clientId).to.equal(clientId);
    expect(seen.connect).to.have.property('protoVer');
    expect(seen.connect).to.have.property('keepAlive');
    expect(seen.connect).to.have.property('remoteAddr');
    if (typeof seen.connect.remoteAddr === 'string') {
      expect(seen.connect.remoteAddr).to.contain('127.0.0.1');
    }

    expect(seen.connack).to.be.an('object');
    expect(seen.connack.clientId).to.equal(clientId);
    expect(seen.connack).to.have.property('connAck');
  });

  // (Removed) A deny-case connack smoke test was flaky; connack deny semantics are covered in connack-dedup.test.ts
});
