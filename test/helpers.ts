import * as net from 'net';

// On Windows, force exit after all tests to prevent hanging
if (process.platform === 'win32') {
  process.on('exit', () => {
    // Give a small window for cleanup, then force exit
    setTimeout(() => {
      process.exit(0);
    }, 1000).unref();
  });
}

export async function waitForPort(host: string, port: number, timeoutMs: number = 8000, intervalMs: number = 50): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const listening = await isListening(host, port);
    if (listening) return;
    await new Promise(r => setTimeout(r, intervalMs));
  }
  throw new Error(`Port ${host}:${port} not ready within ${timeoutMs}ms`);
}

export async function waitForPortClosed(host: string, port: number, timeoutMs = 5000, intervalMs: number = 50): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const listening = await isListening(host, port);
    if (!listening) return;
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  throw new Error(`Port ${port} on ${host} did not close within ${timeoutMs}ms`);
}

function isListening(host: string, port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = new net.Socket();
    let settled = false;

    const cleanup = () => {
      if (!settled) return;
      // no-op
    };

    socket.setTimeout(500, () => {
      settled = true;
      socket.destroy();
      resolve(false);
    });

    socket.once('connect', () => {
      settled = true;
      socket.destroy();
      resolve(true);
    });

    socket.once('error', () => {
      settled = true;
      // don't throw; just report not ready
      resolve(false);
    });

    try {
      socket.connect(port, host);
    } catch {
      settled = true;
      resolve(false);
    }
  });
}
