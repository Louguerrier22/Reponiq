import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
const config = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const cargo = fs.readFileSync('src-tauri/Cargo.toml', 'utf8');
const version = pkg.version;
if (!/^\d+\.\d+\.\d+$/.test(version) || config.version !== version || !cargo.includes(`version = "${version}"`)) throw new Error('Package, native configuration, and Cargo versions must agree.');
if (process.env.GITHUB_REF_TYPE === 'tag') {
  if (process.env.GITHUB_REF_NAME !== `v${version}`) throw new Error('Release tag must equal the exact application version.');
  execFileSync('git', ['merge-base', '--is-ancestor', 'HEAD', 'origin/main']);
}
if (process.argv[2] === 'verify') {
  console.log(`Reponiq ${version}: version and tag checks passed.`);
} else if (process.argv[2] === 'collect') {
  const platform = process.argv[3];
  const formats = { 'windows-x64': [['nsis', '.exe']], 'linux-x64': [['deb', '.deb'], ['rpm', '.rpm'], ['appimage', '.AppImage']] }[platform];
  if (!formats) throw new Error('Choose windows-x64 or linux-x64.');
  const output = path.resolve('.work/release', platform);
  if (fs.existsSync(output) && fs.readdirSync(output).length) throw new Error('Installer output directory must be empty; refusing to include stale files.');
  fs.mkdirSync(output, { recursive: true });
  const sums = [];
  for (const [folder, extension] of formats) {
    const base = path.join('src-tauri/target/release/bundle', folder);
    const candidates = fs.readdirSync(base).filter(file => file.endsWith(extension));
    if (candidates.length !== 1 || !candidates[0].includes(version)) throw new Error(`Expected exactly one ${version} ${extension} installer.`);
    const source = path.join(base, candidates[0]);
    const stat = fs.lstatSync(source);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error('Installer must be a regular file.');
    const name = `Reponiq-${version}-${platform}${extension}`;
    if (stat.size < 1 || stat.size > 512 * 1024 * 1024) throw new Error('Unexpected installer size.');
    fs.copyFileSync(source, path.join(output, name));
    const hash = crypto.createHash('sha256');
    for await (const chunk of fs.createReadStream(path.join(output, name))) hash.update(chunk);
    sums.push(`${hash.digest('hex')}  ${name}`);
  }
  fs.writeFileSync(path.join(output, 'SHA256SUMS.txt'), `${sums.join('\n')}\n`);
  console.log(`Collected ${formats.length} installer(s) with SHA-256 checksums in ${output}`);
} else if (process.argv[2] === 'update-manifest') {
  const outputs = [
    ['windows-x86_64', 'windows-x64', 'exe'],
    ['linux-x86_64', 'linux-x64', 'AppImage'],
  ];
  const platforms = {};
  for (const [target, directory, extension] of outputs) {
    const name = `Reponiq-${version}-${directory}.${extension}`;
    const asset = path.join('.work', 'release', directory, name);
    const info = fs.lstatSync(asset);
    if (!info.isFile() || info.isSymbolicLink()) throw new Error(`Invalid update asset ${name}.`);
    const signatureFile = asset + '.sig';
    const signatureInfo = fs.lstatSync(signatureFile);
    if (!signatureInfo.isFile() || signatureInfo.isSymbolicLink() || signatureInfo.size > 4096) throw new Error(`Invalid signature file for ${name}.`);
    const signature = fs.readFileSync(signatureFile, 'utf8').trim();
    if (!/^[A-Za-z0-9+/]+={0,2}$/.test(signature)) throw new Error(`Malformed signature for ${name}.`);
    const signedComment = Buffer.from(signature, 'base64').toString('utf8').split('\n')
      .find(line => line.startsWith('trusted comment: '));
    if (!signedComment?.split('\t').includes(`version:${version}`)) {
      throw new Error(`Sign ${name} for version ${version} before generating the manifest.`);
    }
    platforms[target] = {
      signature,
      url: `https://github.com/Louguerrier22/Reponiq/releases/download/v${version}/${name}`,
    };
  }
  fs.writeFileSync('.work/release/latest.json', JSON.stringify({ version, platforms }, null, 2) + '\n', { flag: 'wx' });
  console.log(`Prepared the Reponiq ${version} update manifest for signed Windows and AppImage assets.`);
} else throw new Error('Use verify, collect <windows-x64|linux-x64>, or update-manifest.');
