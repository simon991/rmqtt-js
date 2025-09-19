"use strict";

import { describe, it, beforeEach, afterEach } from 'mocha';
import { expect } from 'chai';
import * as mqtt from 'mqtt';
import { MqttServer, QoS, ServerConfig } from '../index';
import { waitForPort } from './helpers';

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
});
