/*
 Create a node-gyp-build compatible prebuild from the compiled addon.
 - Reads dist/index.node (produced by cargo build + scripts/copy-native.js)
 - Writes prebuilds/<platform>-<arch>/node.napi.node
 - Platform/arch can be overridden via PREBUILD_PLATFORM and PREBUILD_ARCH env vars
*/
const fs = require('fs');
const path = require('path');

function getPlatform() {
  return process.env.PREBUILD_PLATFORM || process.platform; // 'darwin' | 'linux' | 'win32'
}

function getArch() {
  return process.env.PREBUILD_ARCH || process.arch; // 'x64' | 'arm64' | 'ia32' | ...
}

function main() {
  const platform = getPlatform();
  const arch = getArch();
  const root = path.join(__dirname, '..');
  const from = path.join(root, 'dist', 'index.node');
  if (!fs.existsSync(from)) {
    throw new Error(`dist/index.node not found at ${from}. Did you run 'npm run build:native:release'?`);
  }
  const outDir = path.join(root, 'prebuilds', `${platform}-${arch}`);
  const outFile = path.join(outDir, 'node.napi.node');
  fs.mkdirSync(outDir, { recursive: true });
  fs.copyFileSync(from, outFile);
  const stat = fs.statSync(outFile);
  console.log(`[prebuild] Wrote ${outFile} (${stat.size} bytes)`);
}

main();
