"use strict";

import { strict as assert } from "assert";
import mqtt from "mqtt";
import { MqttServer, QoS } from "../src";
import { waitForPort, waitForPortClosed } from "./helpers";

function nextPort(base = 19000) {
  return base + Math.floor(Math.random() * 1000);
}

describe("publish authorization hook", function () {
  this.timeout(15000);

  let server: MqttServer;
  let port: number;

  beforeEach(async () => {
    server = new MqttServer();
    port = nextPort();
  });

  afterEach(async () => {
    try {
      if (server?.running) {
        await server.stop();
        if (port) {
          try { await waitForPortClosed("127.0.0.1", port); } catch {}
        }
      }
    } finally {
      server?.close();
    }
  });

  it("allows publish when hook permits", async () => {
    const topic = "user1/data";
    const payload = Buffer.from("hello");
    let received: Buffer | null = null;

    server.setHooks({
      onClientPublishAuthorize: () => ({ allow: true })
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.subscribe(topic, { qos: 0 }, (err) => {
            if (err) return reject(err);
            client.publish(topic, payload, { qos: 0 }, (err2) => {
              if (err2) return reject(err2);
            });
          });
        });
        client.on("message", (t, p) => {
          if (t === topic) {
            received = p;
            resolve();
          }
        });
      });
    } finally {
      client.end(true);
    }

  assert.ok(received, "Expected to receive a message");
  assert.equal((received as Buffer).toString(), payload.toString());
  });

  it("denies publish when hook denies", async () => {
    const topic = "user1/deny";
    const payload = Buffer.from("nope");
    let received = false;

    server.setHooks({
      onClientPublishAuthorize: () => ({ allow: false, reason: "policy" })
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.subscribe(topic, { qos: 0 }, (err) => {
            if (err) return reject(err);
            client.publish(topic, payload, { qos: 0 }, (err2) => {
              if (err2) return reject(err2);
              // wait briefly for any unexpected delivery (generous to avoid flakes)
              setTimeout(resolve, 1200);
            });
          });
        });
        client.on("message", () => { received = true; });
      });
    } finally {
      client.end(true);
    }

    assert.equal(received, false, "Should not receive denied message");
  });

  it("supports Promise-based hook that mutates", async () => {
    const inTopic = "user1/in2";
    const outTopic = "user1/out2";
    let seenTopic: string | null = null;
    let seenPayload: string | null = null;

    server.setHooks({
      onClientPublishAuthorize: async (_session, packet) => {
        await new Promise((r) => setTimeout(r, 50));
        return {
          allow: true,
          topic: outTopic,
          payload: Buffer.from(packet.payload.toString("utf8").repeat(2)),
          qos: QoS.AtLeastOnce,
        };
      },
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.subscribe(outTopic, { qos: 1 }, (err) => {
            if (err) return reject(err);
            client.publish(inTopic, "xy", { qos: 1 });
          });
        });
        client.on("message", (t, p) => { seenTopic = t; seenPayload = p.toString(); resolve(); });
      });
    } finally {
      client.end(true);
    }

    assert.equal(seenTopic, outTopic);
    assert.equal(seenPayload, "xyxy");
  });

  it("denies when Promise rejects", async () => {
    const topic = "reject/test";
    let received = false;

    server.setHooks({
      onClientPublishAuthorize: async () => {
        return Promise.reject(new Error("nope"));
      }
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.subscribe(topic, { qos: 0 }, (err) => {
            if (err) return reject(err);
            client.publish(topic, "z", { qos: 0 }, (err2) => {
              if (err2) return reject(err2);
              setTimeout(resolve, 800);
            });
          });
        });
        client.on("message", () => { received = true; });
      });
    } finally {
      client.end(true);
    }

    assert.equal(received, false, "Should not receive when hook rejects");
  });

  it("denies when hook times out (~5s)", async function () {
    this.timeout(20000);
    const topic = "timeout/test";
    let received = false;

    server.setHooks({
      onClientPublishAuthorize: async () => {
        // Never resolve within timeout window
        await new Promise((r) => setTimeout(r, 6000));
        return { allow: true };
      }
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      const start = Date.now();
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.subscribe(topic, { qos: 0 }, (err) => {
            if (err) return reject(err);
            client.publish(topic, "t", { qos: 0 }, (err2) => {
              if (err2) return reject(err2);
              // wait slightly beyond timeout to be sure
              setTimeout(resolve, 6200);
            });
          });
        });
        client.on("message", () => { received = true; });
      });
      const elapsed = Date.now() - start;
      // Ensure we actually waited close to the timeout period to validate path
      if (elapsed < 5000) {
        throw new Error(`Test did not wait long enough: ${elapsed}ms`);
      }
    } finally {
      client.end(true);
    }

    assert.equal(received, false, "Should not receive when hook times out");
  });

  it("mutates topic and payload", async () => {
    const inTopic = "user1/in";
    const outTopic = "user1/out";
    let seenTopic: string | null = null;
    let seenPayload: string | null = null;

    server.setHooks({
      onClientPublishAuthorize: (_session, packet) => ({
        allow: true,
        topic: outTopic,
        payload: Buffer.from(packet.payload.toString("utf8").toUpperCase()),
        qos: QoS.AtMostOnce
      })
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.subscribe(outTopic, { qos: 0 }, (err) => {
            if (err) return reject(err);
            client.publish(inTopic, "abc", { qos: 1 });
          });
        });
        client.on("message", (t, p) => {
          seenTopic = t; seenPayload = p.toString(); resolve();
        });
      });
    } finally {
      client.end(true);
    }

    assert.equal(seenTopic, outTopic);
    assert.equal(seenPayload, "ABC");
  });

  it("default denies $SYS publishes when no hook is registered", async () => {
    const topic = "$SYS/test";
    let received = false;

    // No hook registered
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.subscribe("$SYS/#", { qos: 0 }, (err) => {
            if (err) return reject(err);
            client.publish(topic, "x", { qos: 0 }, (err2) => {
              if (err2) return reject(err2);
              setTimeout(resolve, 300);
            });
          });
        });
        client.on("message", () => { received = true; });
      });
    } finally {
      client.end(true);
    }

    assert.equal(received, false, "Should not receive on $SYS without publish hook");
  });

  it("ignores invalid QoS override from publish hook and uses original publish QoS", async function () {
    this.timeout(12000);

    const topic = "user1/qos-invalid";

    server.setHooks({
      onClientPublishAuthorize: () => ({ allow: true, qos: 7 as any })
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          // Subscribe with QoS 2 so delivery QoS is governed by publish QoS
          client.subscribe(topic, { qos: 2 }, (err) => {
            if (err) return reject(err);
            // Publish with QoS 2; hook tries to override to invalid qos=7
            client.publish(topic, "ping", { qos: 2 }, (err2) => {
              if (err2) return reject(err2);
            });
          });
        });
        client.on("message", (t, _p, packet) => {
          if (t !== topic) return;
          try {
            // Expect delivered QoS to equal original publish QoS (2), since invalid override is ignored
            if (packet && typeof (packet as any).qos === "number") {
              // @ts-ignore - mqtt.js packet typing at runtime
              const q = (packet as any).qos;
              assert.equal(q, 2);
            }
            resolve();
          } catch (e) { reject(e as any); }
        });
      });
    } finally {
      client.end(true);
    }
  });

  it("ignores invalid topic mutation (+/#) from publish hook and uses original topic", async () => {
    const inTopic = "user1/input";
    const invalidTopic = "user1/+/#"; // invalid for publish; should be ignored
    let observedTopic: string | null = null;

    server.setHooks({
      onClientPublishAuthorize: () => ({ allow: true, topic: invalidTopic }),
      onMessagePublish: (_session, _from, msg) => {
        if (msg.topic.startsWith("user1/")) {
          observedTopic = msg.topic;
        }
      }
    });
    await server.start(MqttServer.createBasicConfig(port));
    await waitForPort("127.0.0.1", port);

    const client = mqtt.connect(`mqtt://127.0.0.1:${port}`);
    try {
      await new Promise<void>((resolve, reject) => {
        client.on("error", reject);
        client.on("connect", () => {
          client.publish(inTopic, "data", { qos: 1 }, (err) => {
            if (err) return reject(err);
            // Allow a short delay for onMessagePublish to fire
            setTimeout(resolve, 150);
          });
        });
      });
    } finally {
      client.end(true);
    }

    assert.equal(observedTopic, inTopic, "Hook should not change topic when mutation invalid");
  });
});
