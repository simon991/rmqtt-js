/*
 Cross-platform copy of the cargo cdylib to dist/index.node
 - Crate name inferred from Cargo.toml [package].name (dashes -> underscores for filename)
 - Maps platform extensions: .dylib (darwin), .so (linux), .dll (win32)
*/
const fs = require('fs');
const path = require('path');

function readCrateName() {
    const cargoToml = fs.readFileSync(path.join(__dirname, '..', 'Cargo.toml'), 'utf8');
    const match = cargoToml.match(/name\s*=\s*"([^"]+)"/);
    if (!match) throw new Error('Could not find package.name in Cargo.toml');
    return match[1];
}

function getLibFilename(crateName) {
    const base = crateName.replace(/-/g, '_');
    const platform = process.platform;
    if (platform === 'darwin') return `lib${base}.dylib`;
    if (platform === 'linux') return `lib${base}.so`;
    if (platform === 'win32') return `${base}.dll`;
    throw new Error(`Unsupported platform: ${platform}`);
}

function main() {
    const crateName = readCrateName();
    const libFile = getLibFilename(crateName);
    const targetRoot = path.join(__dirname, '..', 'target');
    const searchOrder = [
        { profile: 'release', file: path.join(targetRoot, 'release', libFile) },
        { profile: 'debug', file: path.join(targetRoot, 'debug', libFile) },
    ];
    const found = searchOrder.find(candidate => fs.existsSync(candidate.file));
    if (!found) {
        const attempted = searchOrder.map(c => c.file).join(', ');
        throw new Error(`Native artifact not found. Looked for: ${attempted}`);
    }
    const toDir = path.join(__dirname, '..', 'dist');
    const to = path.join(toDir, 'index.node');
    fs.mkdirSync(toDir, { recursive: true });
    fs.copyFileSync(found.file, to);
    console.log(`Copied native module (${found.profile}) to ${to}`);
}

main();
