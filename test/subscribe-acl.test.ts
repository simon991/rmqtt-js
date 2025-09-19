"use strict";

import { describe, it, beforeEach, afterEach } from 'mocha';
const { expect } = require('chai');
import * as mqtt from 'mqtt';
import { MqttServer, QoS, ServerConfig } from '../src/index';
import { waitForPort, waitForPortClosed } from './helpers';

describe('Subscribe Authorization Hook', () => {
  let server: MqttServer;
  let client: mqtt.MqttClient | null = null;
  let currentPort = 0;
  let portCounter = 0;
  const nextPort = () => 19600 + (portCounter++);

  beforeEach(() => {
    server = new MqttServer();
    currentPort = nextPort();
    // Register hooks:
    // - Auth: require password 'demo'
    // - Subscribe ACL: allow only `${username}/#` and cap QoS to 1
    server.setHooks({
      onClientAuthenticate: (auth) => ({ allow: Boolean(auth.username) && auth.password === 'demo' }),
      onClientSubscribeAuthorize: (session, subscription) => {
        const user = session?.username ?? '';
        const allow = subscription.topicFilter.startsWith(`${user}/`);
        return allow ? { allow: true, qos: QoS.AtLeastOnce } : { allow: false, reason: 'Not allowed' };
      }
    });
  });

  afterEach(async () => {
    if (client) {
      await new Promise<void>(resolve => client!.end(false, {}, () => resolve()));
      client = null;
    }
    if (server && server.running) {
      await server.stop();
      if (currentPort) {
        try { await waitForPortClosed('127.0.0.1', currentPort); } catch {}
      }
    }
    server.close();
  });

  it('allows subscription with QoS override when hook approves', async function () {
    this.timeout(10000);

    const config: ServerConfig = {
      listeners: [{ name: 'tcp', address: '127.0.0.1', port: currentPort, protocol: 'tcp', allowAnonymous: true }]
    };
    await server.start(config);
    await waitForPort('127.0.0.1', currentPort);

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Test timeout')), 7000);
      client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`, { username: 'alice', password: 'demo' });

      client.once('connect', () => {
        // Request QoS 2, expect override to QoS 1
  client!.subscribe('alice/topic', { qos: 2 }, (err, granted) => {
          if (err) { clearTimeout(timeout); return reject(err); }
          try {
            expect(granted, 'granted results').to.exist;
            expect(granted!).to.have.length(1);
            // mqtt.js sets qos=128 for failure, otherwise granted qos value
            expect(granted![0].qos).to.equal(1);
          } catch (e) { clearTimeout(timeout); return reject(e); }

          // Now publish to verify we actually receive messages
          client!.once('message', (topic, payload) => {
            try {
              expect(topic).to.equal('alice/topic');
              expect(payload.toString()).to.equal('hello');
              clearTimeout(timeout);
              resolve();
            } catch (e) {
              clearTimeout(timeout);
              reject(e);
            }
          });
          client!.on('error', (e) => { /* swallow client errors in this test */ });
          server.publish('alice/topic', 'hello').catch(reject);
        });
      });

      client.on('error', (e) => { clearTimeout(timeout); reject(e); });
    });
  });

  it('allows subscription with Promise-based hook and QoS override', async function () {
    this.timeout(12000);

    const config: ServerConfig = {
      listeners: [{ name: 'tcp', address: '127.0.0.1', port: currentPort, protocol: 'tcp', allowAnonymous: true }]
    };
    // Override hook to be Promise-based for this test
    server.setHooks({
      onClientAuthenticate: (auth) => ({ allow: Boolean(auth.username) && auth.password === 'demo' }),
      onClientSubscribeAuthorize: async (session, subscription) => {
        await new Promise(r => setTimeout(r, 30));
        const user = session?.username ?? '';
        const allow = subscription.topicFilter.startsWith(`${user}/`);
        return allow ? { allow: true, qos: QoS.AtLeastOnce } : { allow: false, reason: 'Not allowed' };
      }
    });

    await server.start(config);
    await waitForPort('127.0.0.1', currentPort);

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Test timeout')), 9000);
      client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`, { username: 'alice', password: 'demo' });

      client.once('connect', () => {
        client!.subscribe('alice/promise', { qos: 2 }, (err, granted) => {
          if (err) { clearTimeout(timeout); return reject(err); }
          try {
            expect(granted, 'granted results').to.exist;
            expect(granted!).to.have.length(1);
            expect(granted![0].qos).to.equal(1);
          } catch (e) { clearTimeout(timeout); return reject(e as any); }

          client!.once('message', (topic, payload) => {
            try {
              expect(topic).to.equal('alice/promise');
              expect(payload.toString()).to.equal('hello2');
              clearTimeout(timeout);
              resolve();
            } catch (e) { clearTimeout(timeout); reject(e as any); }
          });
          client!.on('error', () => { /* ignore client errors for this test */ });
          server.publish('alice/promise', 'hello2').catch(e => { clearTimeout(timeout); reject(e); });
        });
      });

      client.on('error', (e) => { clearTimeout(timeout); reject(e); });
    });
  });

  it('denies subscription when Promise rejects', async function () {
    this.timeout(12000);

    // Promise-rejecting hook
    server.setHooks({
      onClientAuthenticate: (auth) => ({ allow: Boolean(auth.username) && auth.password === 'demo' }),
      onClientSubscribeAuthorize: async () => {
        return Promise.reject(new Error('nope'));
      }
    });

    const config: ServerConfig = {
      listeners: [{ name: 'tcp', address: '127.0.0.1', port: currentPort, protocol: 'tcp', allowAnonymous: true }]
    };
    await server.start(config);
    await waitForPort('127.0.0.1', currentPort);

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Test timeout')), 9000);
      client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`, { username: 'alice', password: 'demo' });

      client.once('connect', () => {
        client!.subscribe('alice/reject', { qos: 0 }, (err, granted) => {
          try {
            if (err) {
              expect(err).to.be.instanceOf(Error);
            } else {
              expect(granted).to.exist;
              expect(granted!).to.have.length(1);
              expect(granted![0].qos).to.equal(128);
            }
          } catch (e) { clearTimeout(timeout); return reject(e as any); }

          let received = false;
          client!.on('message', () => { received = true; });
          server.publish('alice/reject', 'hidden')
            .then(() => setTimeout(() => {
              try {
                expect(received).to.equal(false);
                clearTimeout(timeout);
                resolve();
              } catch (e) { clearTimeout(timeout); reject(e as any); }
            }, 200))
            .catch(e => { clearTimeout(timeout); reject(e); });
        });
      });

      client.on('error', (e) => { clearTimeout(timeout); reject(e); });
    });
  });

  it('denies subscription when hook times out (~5s)', async function () {
    this.timeout(20000);

    // Slow hook that resolves after 6s to trigger timeout path (deny)
    server.setHooks({
      onClientAuthenticate: (auth) => ({ allow: Boolean(auth.username) && auth.password === 'demo' }),
      onClientSubscribeAuthorize: async () => {
        await new Promise(r => setTimeout(r, 6000));
        return { allow: true, qos: QoS.AtLeastOnce };
      }
    });

    const config: ServerConfig = {
      listeners: [{ name: 'tcp', address: '127.0.0.1', port: currentPort, protocol: 'tcp', allowAnonymous: true }]
    };
    await server.start(config);
    await waitForPort('127.0.0.1', currentPort);

    await new Promise<void>((resolve, reject) => {
      const start = Date.now();
      const timeout = setTimeout(() => reject(new Error('Test timeout')), 16000);
      client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`, { username: 'alice', password: 'demo' });

      client.once('connect', () => {
        client!.subscribe('alice/slow', { qos: 1 }, (err, granted) => {
          try {
            const elapsed = Date.now() - start;
            if (elapsed < 5000) {
              throw new Error(`Subscribe returned too quickly: ${elapsed}ms`);
            }
            if (err) {
              expect(err).to.be.instanceOf(Error);
            } else {
              expect(granted).to.exist;
              expect(granted![0].qos).to.equal(128);
            }
          } catch (e) { clearTimeout(timeout); return reject(e as any); }
          clearTimeout(timeout);
          resolve();
        });
      });

      client.on('error', (e) => { clearTimeout(timeout); reject(e); });
    });
  });
  it('denies subscription when hook rejects', async function () {
    this.timeout(10000);

    const config: ServerConfig = {
      listeners: [{ name: 'tcp', address: '127.0.0.1', port: currentPort, protocol: 'tcp', allowAnonymous: true }]
    };
    await server.start(config);
    await waitForPort('127.0.0.1', currentPort);

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Test timeout')), 7000);
      client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`, { username: 'alice', password: 'demo' });

      client.once('connect', () => {
  client!.subscribe('bob/topic', { qos: 0 }, (err, granted) => {
          try {
            if (err) {
              // mqtt.js with MQTT v5 maps NotAuthorized to an Error (often 'Unspecified error')
              expect(err).to.be.instanceOf(Error);
            } else {
              expect(granted, 'granted results').to.exist;
              expect(granted!).to.have.length(1);
              // 128 indicates failure to subscribe per MQTT v3 suback
              expect(granted![0].qos).to.equal(128);
            }
          } catch (e) { clearTimeout(timeout); return reject(e as any); }

          // Publish should not be delivered to this client as subscription failed
          let received = false;
          client!.on('message', () => { received = true; });
          server.publish('bob/topic', 'hidden')
            .then(() => setTimeout(() => {
              try {
                expect(received).to.equal(false);
                clearTimeout(timeout);
                resolve();
              } catch (e) { clearTimeout(timeout); reject(e as any); }
            }, 200))
            .catch(e => { clearTimeout(timeout); reject(e); });
        });
      });

      client.on('error', (e) => { clearTimeout(timeout); reject(e); });
    });
  });

  it('ignores invalid QoS override from hook and uses requested QoS', async function () {
    this.timeout(12000);

    // Hook returns invalid qos (e.g., 7); should be ignored and grant requested qos
    server.setHooks({
      onClientAuthenticate: (auth) => ({ allow: Boolean(auth.username) && auth.password === 'demo' }),
      onClientSubscribeAuthorize: (_session, _sub) => ({ allow: true, qos: 7 as any })
    });

    const config: ServerConfig = {
      listeners: [{ name: 'tcp', address: '127.0.0.1', port: currentPort, protocol: 'tcp', allowAnonymous: true }]
    };
    await server.start(config);
    await waitForPort('127.0.0.1', currentPort);

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Test timeout')), 9000);
      const topic = 'alice/qos-check';
      client = mqtt.connect(`mqtt://127.0.0.1:${currentPort}`, { username: 'alice', password: 'demo' });

      client.once('connect', () => {
        // Request QoS 2; since hook's qos=7 is invalid, granted should equal requested (2)
        client!.subscribe(topic, { qos: 2 }, (err, granted) => {
          if (err) { clearTimeout(timeout); return reject(err); }
          try {
            expect(granted).to.exist;
            expect(granted!).to.have.length(1);
            expect(granted![0].qos).to.equal(2);
          } catch (e) { clearTimeout(timeout); return reject(e as any); }

          client!.once('message', (t, p) => {
            try {
              expect(t).to.equal(topic);
              expect(p.toString()).to.equal('ok');
              clearTimeout(timeout);
              resolve();
            } catch (e) { clearTimeout(timeout); reject(e as any); }
          });
          server.publish(topic, 'ok').catch(e => { clearTimeout(timeout); reject(e); });
        });
      });

      client.on('error', (e) => { clearTimeout(timeout); reject(e); });
    });
  });
});
