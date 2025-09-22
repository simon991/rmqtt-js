const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

function platformDir() {
    const p = process.platform;
    if (p === 'darwin') return 'darwin';
    if (p === 'linux') return 'linux';
    if (p === 'win32') return 'win32';
    return p;
}

function archDir() {
    const a = process.arch;
    // node-gyp-build supports x64, arm64, ia32 etc.
    return a;
}

function prebuildPath() {
    return path.join(__dirname, '..', 'prebuilds', `${platformDir()}-${archDir()}`, 'node.napi.node');
}

function main() {
    const prebuilt = prebuildPath();
    if (fs.existsSync(prebuilt)) {
        console.log(`[install] Found prebuilt binary: ${prebuilt} — skipping source build.`);
        return;
    }

    console.log('[install] No prebuilt binary found. Building from source...');
    const cargo = spawnSync('cargo', ['build', '--release', '--message-format=json-render-diagnostics'], { stdio: 'inherit' });
    if (cargo.status !== 0) {
        process.exit(cargo.status || 1);
    }
    const copy = spawnSync(process.execPath, [path.join(__dirname, 'copy-native.js')], { stdio: 'inherit' });
    if (copy.status !== 0) {
        process.exit(copy.status || 1);
    }
}

main();
