import { describe, it, beforeEach, afterEach } from 'mocha';
const { expect } = require('chai');
import mqtt, { MqttClient } from 'mqtt';
import {
  MqttServer,
  QoS,
  HookCallbacks,
  AuthenticationRequest,
  AuthenticationResult,
  SessionInfo,
  SubscriptionInfo,
  UnsubscriptionInfo,
  MessageInfo,
  MessageFrom,
  MqttMessage,
  ConnectInfo,
  ConnackInfo,
} from '../src/index';
import { waitForPort, waitForPortClosed } from './helpers';

interface Deferred<T = void> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  readonly settled: boolean;
}

function createDeferred<T = void>(): Deferred<T> {
  let resolved = false;
  let resolveFn!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolveFn = (value: T) => {
      if (!resolved) {
        resolved = true;
        res(value);
      }
    };
  });
  return {
    promise,
    resolve: (value: T) => resolveFn(value),
    get settled() {
      return resolved;
    },
  };
}

async function withTimeout<T>(promise: Promise<T>, label: string, timeoutMs = 10000): Promise<T> {
  let timer: NodeJS.Timeout | null = null;
  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => {
        timer = setTimeout(() => reject(new Error(`Timed out waiting for ${label}`)), timeoutMs);
      }),
    ]);
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
  }
}

describe('Hook parameter coverage', function () {
  this.timeout(25000);

  let server: MqttServer;
  let port: number;
  let client: MqttClient | null = null;

  beforeEach(() => {
    server = new MqttServer();
    port = 20050 + Math.floor(Math.random() * 1000);
  });

  afterEach(async () => {
    if (client) {
      await new Promise<void>((resolve) => {
        try {
          client!.end(true, {}, () => resolve());
        } catch {
          resolve();
        }
      });
      client = null;
    }
    if (server.running) {
      await server.stop();
      try { await waitForPortClosed('127.0.0.1', port); } catch { /* ignore */ }
    }
    server.close();
  });

  it('surfaces full payloads for every hook callback', async () => {
    const clientId = `hook-${Date.now()}`;
    const username = 'alice';
    const password = 'secret';
    const keepAlive = 30;
    const allowedTopic = `${username}/topic`;
    const blockedTopic = `${username}/blocked`;
  const cleanupTopic = `${username}/cleanup`;
    const allowedPayload = 'message-data';

    const captured = {
      authRequest: null as AuthenticationRequest | null,
  subscribeAuthorize: [] as Array<{ session: SessionInfo | null; subscription: SubscriptionInfo }>,
  publishAuthorizeAllow: null as { session: SessionInfo | null; packet: MqttMessage } | null,
  publishAuthorizeDeny: null as { session: SessionInfo | null; packet: MqttMessage } | null,
  messagePublish: [] as Array<{ session: SessionInfo | null; from: MessageFrom; message: MessageInfo }>,
  clientSubscribe: [] as Array<{ session: SessionInfo | null; subscription: SubscriptionInfo }>,
  clientUnsubscribe: [] as Array<{ session: SessionInfo | null; unsubscription: UnsubscriptionInfo }>,
  messageDelivered: [] as Array<{ session: SessionInfo | null; from: MessageFrom; message: MessageInfo }>,
  messageAcked: [] as Array<{ session: SessionInfo | null; from: MessageFrom; message: MessageInfo }>,
  messageDropped: [] as Array<{ session: SessionInfo | null; from: MessageFrom | null; message: MessageInfo; info?: { reason?: string } }>,
      clientConnect: null as ConnectInfo | null,
      clientConnack: null as ConnackInfo | null,
      clientConnected: null as SessionInfo | null,
      clientDisconnected: null as { session: SessionInfo; info?: { reason?: string } } | null,
      sessionCreated: null as SessionInfo | null,
  sessionSubscribed: [] as Array<{ session: SessionInfo; subscription: SubscriptionInfo }>,
  sessionUnsubscribed: [] as Array<{ session: SessionInfo; unsubscription: UnsubscriptionInfo }>,
      sessionTerminated: null as { session: SessionInfo; info?: { reason?: string } } | null,
    };

    const defers = {
      auth: createDeferred<void>(),
      clientConnect: createDeferred<void>(),
      clientConnack: createDeferred<void>(),
      clientConnected: createDeferred<void>(),
      sessionCreated: createDeferred<void>(),
      subscribeAuthorize: createDeferred<void>(),
      clientSubscribe: createDeferred<void>(),
      sessionSubscribed: createDeferred<void>(),
      publishAuthorizeAllow: createDeferred<void>(),
      publishAuthorizeDeny: createDeferred<void>(),
      messagePublish: createDeferred<void>(),
      messageDelivered: createDeferred<void>(),
      messageAcked: createDeferred<void>(),
      messageDropped: createDeferred<void>(),
      clientUnsubscribe: createDeferred<void>(),
      sessionUnsubscribed: createDeferred<void>(),
      clientDisconnected: createDeferred<void>(),
      sessionTerminated: createDeferred<void>(),
    } as const;

    const hooks: HookCallbacks = {
      onClientAuthenticate: (req: AuthenticationRequest): AuthenticationResult => {
        captured.authRequest = { ...req };
        defers.auth.resolve();
        return {
          allow: req.username === username && req.password === password,
          superuser: false,
          reason: 'ok',
        };
      },
      onClientConnect: (info: ConnectInfo) => {
        captured.clientConnect = { ...info };
        defers.clientConnect.resolve();
      },
      onClientConnack: (info: ConnackInfo) => {
        captured.clientConnack = { ...info };
        defers.clientConnack.resolve();
      },
      onClientConnected: (session: SessionInfo) => {
        captured.clientConnected = { ...session };
        defers.clientConnected.resolve();
      },
      onClientDisconnected: (session: SessionInfo, info?: { reason?: string }) => {
        captured.clientDisconnected = info
          ? { session: { ...session }, info: { ...info } }
          : { session: { ...session } };
        defers.clientDisconnected.resolve();
      },
      onSessionCreated: (session: SessionInfo) => {
        captured.sessionCreated = { ...session };
        defers.sessionCreated.resolve();
      },
      onClientSubscribeAuthorize: (session: SessionInfo | null, subscription: SubscriptionInfo) => {
        captured.subscribeAuthorize.push({
          session: session ? { ...session } : null,
          subscription: { ...subscription },
        });
        defers.subscribeAuthorize.resolve();
        const allow = subscription.topicFilter.startsWith(`${username}/`);
        return allow ? { allow: true, qos: QoS.AtLeastOnce } : { allow: false, reason: 'not allowed' };
      },
      onClientSubscribe: (session: SessionInfo | null, subscription: SubscriptionInfo) => {
        captured.clientSubscribe.push({
          session: session ? { ...session } : null,
          subscription: { ...subscription },
        });
        defers.clientSubscribe.resolve();
      },
      onSessionSubscribed: (session: SessionInfo, subscription: SubscriptionInfo) => {
        captured.sessionSubscribed.push({
          session: { ...session },
          subscription: { ...subscription },
        });
        defers.sessionSubscribed.resolve();
      },
      onClientPublishAuthorize: (session: SessionInfo | null, packet: MqttMessage) => {
        const cloned = {
          topic: packet.topic,
          payload: Buffer.from(packet.payload),
          qos: packet.qos,
          retain: packet.retain,
        };
        if (packet.topic === blockedTopic) {
          captured.publishAuthorizeDeny = {
            session: session ? { ...session } : null,
            packet: cloned,
          };
          defers.publishAuthorizeDeny.resolve();
          return { allow: false, reason: 'blocked' };
        }
        captured.publishAuthorizeAllow = {
          session: session ? { ...session } : null,
          packet: cloned,
        };
        defers.publishAuthorizeAllow.resolve();
        return { allow: true };
      },
      onMessagePublish: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => {
        captured.messagePublish.push({
          session: session ? { ...session } : null,
          from: { ...from },
          message: {
            ...message,
            payload: Buffer.from(message.payload),
          },
        });
        defers.messagePublish.resolve();
      },
      onMessageDelivered: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => {
        captured.messageDelivered.push({
          session: session ? { ...session } : null,
          from: { ...from },
          message: {
            ...message,
            payload: Buffer.from(message.payload),
          },
        });
        defers.messageDelivered.resolve();
      },
      onMessageAcked: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => {
        captured.messageAcked.push({
          session: session ? { ...session } : null,
          from: { ...from },
          message: {
            ...message,
            payload: Buffer.from(message.payload),
          },
        });
        defers.messageAcked.resolve();
      },
      onMessageDropped: (session: SessionInfo | null, from: MessageFrom | null, message: MessageInfo, info?: { reason?: string }) => {
        captured.messageDropped.push({
          session: session ? { ...session } : null,
          from: from ? { ...from } : null,
          message: {
            ...message,
            payload: Buffer.from(message.payload),
          },
          ...(info ? { info: { ...info } } : {}),
        });
        defers.messageDropped.resolve();
      },
      onClientUnsubscribe: (session: SessionInfo | null, unsubscription: UnsubscriptionInfo) => {
        captured.clientUnsubscribe.push({
          session: session ? { ...session } : null,
          unsubscription: { ...unsubscription },
        });
        defers.clientUnsubscribe.resolve();
      },
      onSessionUnsubscribed: (session: SessionInfo, unsubscription: UnsubscriptionInfo) => {
        captured.sessionUnsubscribed.push({
          session: { ...session },
          unsubscription: { ...unsubscription },
        });
        defers.sessionUnsubscribed.resolve();
      },
      onSessionTerminated: (session: SessionInfo, info?: { reason?: string }) => {
        captured.sessionTerminated = info
          ? { session: { ...session }, info: { ...info } }
          : { session: { ...session } };
        defers.sessionTerminated.resolve();
      },
    };

    server.setHooks(hooks);

    await server.start({
      listeners: [{
        name: 'hook-coverage',
        address: '127.0.0.1',
        port,
        protocol: 'tcp',
        allowAnonymous: false,
      }],
    });
    await waitForPort('127.0.0.1', port);

    client = mqtt.connect(`mqtt://127.0.0.1:${port}`, {
      clientId,
      username,
      password,
      clean: true,
      keepalive: keepAlive,
      protocolVersion: 4,
      reconnectPeriod: 0,
    });

    await new Promise<void>((resolve, reject) => {
      client!.once('connect', () => resolve());
      client!.once('error', reject);
    });

    await withTimeout(Promise.all([
      defers.auth.promise,
      defers.clientConnect.promise,
      defers.clientConnack.promise,
      defers.clientConnected.promise,
      defers.sessionCreated.promise,
    ]), 'initial client lifecycle');

    await new Promise<void>((resolve, reject) => {
      client!.subscribe(allowedTopic, { qos: 1 }, (err) => (err ? reject(err) : resolve()));
    });

    await withTimeout(Promise.all([
      defers.subscribeAuthorize.promise,
      defers.clientSubscribe.promise,
      defers.sessionSubscribed.promise,
    ]), 'subscription hooks');

    const receivedAllowed = createDeferred<void>();
    client.on('message', (topic, payload) => {
      if (!receivedAllowed.settled && topic === allowedTopic && payload.toString() === allowedPayload) {
        receivedAllowed.resolve();
      }
    });

    await new Promise<void>((resolve, reject) => {
      client!.publish(allowedTopic, allowedPayload, { qos: 1 }, (err) => (err ? reject(err) : resolve()));
    });

    await withTimeout(Promise.all([
      receivedAllowed.promise,
      defers.publishAuthorizeAllow.promise,
      defers.messagePublish.promise,
      defers.messageDelivered.promise,
      defers.messageAcked.promise,
    ]), 'allowed publish pipeline');

    await new Promise<void>((resolve, reject) => {
      client!.publish(blockedTopic, 'nope', { qos: 0 }, (err) => (err ? reject(err) : resolve()));
    });

    await withTimeout(Promise.all([
      defers.publishAuthorizeDeny.promise,
      defers.messageDropped.promise,
    ]), 'denied publish drop');

    await new Promise<void>((resolve, reject) => {
      client!.subscribe(cleanupTopic, { qos: 0 }, (err) => (err ? reject(err) : resolve()));
    });

    await new Promise<void>((resolve, reject) => {
      client!.unsubscribe(cleanupTopic, (err) => (err ? reject(err) : resolve()));
    });

    await withTimeout(defers.clientUnsubscribe.promise, 'client unsubscribe hook');

    await new Promise<void>((resolve) => {
      client!.end(false, {}, () => resolve());
    });
    client = null;

    try {
      await withTimeout(Promise.all([
        defers.clientDisconnected.promise,
        defers.sessionTerminated.promise,
        defers.sessionUnsubscribed.promise,
      ]), 'disconnect hooks');
    } catch (err) {
      throw new Error(
        `disconnect hooks timed out (clientDisconnected=${defers.clientDisconnected.settled}, sessionTerminated=${defers.sessionTerminated.settled}, sessionUnsubscribed=${defers.sessionUnsubscribed.settled}): ${(err as Error).message}`,
      );
    }

    await new Promise((res) => setTimeout(res, 150));

    expect(captured.authRequest).to.not.equal(null);
    expect(captured.authRequest!.clientId).to.equal(clientId);
    expect(captured.authRequest!.username).to.equal(username);
    expect(captured.authRequest!.password).to.equal(password);
    expect(captured.authRequest!.protocolVersion).to.equal(4);
    expect(captured.authRequest!.keepAlive).to.equal(keepAlive);
    expect(captured.authRequest!.cleanSession).to.equal(true);
    expect(captured.authRequest!.remoteAddr).to.be.a('string').and.include('127.0.0.1');

    expect(captured.clientConnect).to.not.equal(null);
    expect(captured.clientConnect!.clientId).to.equal(clientId);
    expect(captured.clientConnect!.username).to.equal(username);
    expect(captured.clientConnect!.keepAlive).to.equal(keepAlive);
    expect(captured.clientConnect!.protoVer).to.equal(4);
    expect(captured.clientConnect!.cleanStart).to.equal(true);

    expect(captured.clientConnack).to.not.equal(null);
    expect(captured.clientConnack!.clientId).to.equal(clientId);
    const connackReason = captured.clientConnack!.connAck;
    expect(connackReason).to.be.a('string');
    expect(
      connackReason === 'Success' || /ConnectionAccepted/i.test(connackReason),
      'connack reason should signal acceptance',
    ).to.equal(true);

    expect(captured.clientConnected).to.not.equal(null);
    expect(captured.clientConnected!.clientId).to.equal(clientId);
    expect(captured.clientConnected!.username).to.equal(username);
    expect(captured.clientConnected!.node).to.equal(1);

    expect(captured.sessionCreated).to.not.equal(null);
    expect(captured.sessionCreated!.clientId).to.equal(clientId);
    expect(captured.sessionCreated!.username).to.equal(username);
    expect(captured.sessionCreated!.node).to.equal(1);

  expect(captured.subscribeAuthorize.length).to.be.greaterThan(0);
  const allowedSubscribeAuth = captured.subscribeAuthorize.find((entry) => entry.subscription.topicFilter === allowedTopic);
  expect(allowedSubscribeAuth, 'allowed subscribe authorize capture').to.not.equal(undefined);
  expect(allowedSubscribeAuth!.session).to.not.equal(null);
  expect(allowedSubscribeAuth!.session!.clientId).to.equal(clientId);
  expect(allowedSubscribeAuth!.session!.username).to.equal(username);
  expect(allowedSubscribeAuth!.session!.remoteAddr).to.be.a('string').and.include('127.0.0.1');
  expect(allowedSubscribeAuth!.subscription.qos).to.equal(QoS.AtLeastOnce);

  expect(captured.clientSubscribe.length).to.be.greaterThan(0);
  const allowedClientSubscribe = captured.clientSubscribe.find((entry) => entry.subscription.topicFilter === allowedTopic);
  expect(allowedClientSubscribe, 'allowed client subscribe capture').to.not.equal(undefined);
  expect(allowedClientSubscribe!.session).to.equal(null);
  expect(allowedClientSubscribe!.subscription.qos).to.equal(1);

  expect(captured.sessionSubscribed.length).to.be.greaterThan(0);
  const allowedSessionSubscribed = captured.sessionSubscribed.find((entry) => entry.subscription.topicFilter === allowedTopic);
  expect(allowedSessionSubscribed, 'allowed session subscribed capture').to.not.equal(undefined);
  expect(allowedSessionSubscribed!.session.clientId).to.equal(clientId);
  expect(allowedSessionSubscribed!.subscription.qos).to.equal(1);

    expect(captured.publishAuthorizeAllow).to.not.equal(null);
    expect(captured.publishAuthorizeAllow!.session).to.not.equal(null);
    expect(captured.publishAuthorizeAllow!.session!.clientId).to.equal(clientId);
    expect(captured.publishAuthorizeAllow!.packet.topic).to.equal(allowedTopic);
    expect(captured.publishAuthorizeAllow!.packet.payload.toString()).to.equal(allowedPayload);
    expect(captured.publishAuthorizeAllow!.packet.qos).to.equal(QoS.AtLeastOnce);
    expect(captured.publishAuthorizeAllow!.packet.retain).to.equal(false);

    expect(captured.messagePublish.length).to.be.greaterThan(0);
    const allowedMessagePublish = captured.messagePublish.find((entry) => entry.message.topic === allowedTopic);
    expect(allowedMessagePublish, 'allowed message publish capture').to.not.equal(undefined);
    expect(allowedMessagePublish!.session).to.not.equal(null);
    expect(allowedMessagePublish!.session!.clientId).to.equal(clientId);
    expect(allowedMessagePublish!.from.type).to.equal('client');
    expect(allowedMessagePublish!.from.clientId).to.equal(clientId);
    expect(allowedMessagePublish!.from.username).to.equal(username);
    expect(allowedMessagePublish!.message.payload.toString()).to.equal(allowedPayload);
    expect(allowedMessagePublish!.message.qos).to.equal(QoS.AtLeastOnce);
    expect(allowedMessagePublish!.message.retain).to.equal(false);
    expect(allowedMessagePublish!.message.dup).to.equal(false);
    expect(allowedMessagePublish!.message.createTime).to.be.a('number');

    const blockedMessagePublish = captured.messagePublish.find((entry) => entry.message.topic === blockedTopic);
    expect(blockedMessagePublish, 'blocked message publish capture').to.not.equal(undefined);
    expect(blockedMessagePublish!.session?.clientId).to.equal(clientId);

    expect(captured.messageDelivered.length).to.be.greaterThan(0);
    const allowedMessageDelivered = captured.messageDelivered.find((entry) => entry.message.topic === allowedTopic);
    expect(allowedMessageDelivered, 'allowed message delivered capture').to.not.equal(undefined);
    expect(allowedMessageDelivered!.message.payload.toString()).to.equal(allowedPayload);

    expect(captured.messageAcked.length).to.be.greaterThan(0);
    const allowedMessageAcked = captured.messageAcked.find((entry) => entry.message.topic === allowedTopic);
    expect(allowedMessageAcked, 'allowed message acked capture').to.not.equal(undefined);
    expect(allowedMessageAcked!.message.payload.toString()).to.equal(allowedPayload);

    expect(captured.publishAuthorizeDeny).to.not.equal(null);
    expect(captured.publishAuthorizeDeny!.packet.topic).to.equal(blockedTopic);
    expect(captured.publishAuthorizeDeny!.packet.payload.toString()).to.equal('nope');

    expect(captured.messageDropped.length).to.be.greaterThan(0);
    const blockedMessageDropped = captured.messageDropped.find((entry) => entry.message.topic === blockedTopic);
    expect(blockedMessageDropped, 'blocked message dropped capture').to.not.equal(undefined);
    expect(blockedMessageDropped!.info?.reason).to.be.a('string');

  expect(captured.clientUnsubscribe.length).to.be.greaterThan(0);
  const cleanupClientUnsubscribe = captured.clientUnsubscribe.find((entry) => entry.unsubscription.topicFilter === cleanupTopic);
  expect(cleanupClientUnsubscribe, 'cleanup client unsubscribe capture').to.not.equal(undefined);
  expect(cleanupClientUnsubscribe!.session).to.equal(null);

  expect(captured.sessionUnsubscribed.length).to.be.greaterThan(0);
  const cleanupSessionUnsubscribed = captured.sessionUnsubscribed.find((entry) => entry.unsubscription.topicFilter === cleanupTopic);
  expect(cleanupSessionUnsubscribed, 'cleanup session unsubscribed capture').to.not.equal(undefined);
  expect(cleanupSessionUnsubscribed!.session.clientId).to.equal(clientId);

    expect(captured.clientDisconnected).to.not.equal(null);
    expect(captured.clientDisconnected!.session.clientId).to.equal(clientId);
    if (captured.clientDisconnected!.info?.reason) {
      expect(captured.clientDisconnected!.info!.reason).to.be.a('string');
    }

    expect(captured.sessionTerminated).to.not.equal(null);
    expect(captured.sessionTerminated!.session.clientId).to.equal(clientId);
    if (captured.sessionTerminated!.info?.reason) {
      expect(captured.sessionTerminated!.info!.reason).to.be.a('string');
    }
  });
});
