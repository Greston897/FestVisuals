// Everything needed to get from "Play" to a running game, with no Electron dependency so it can be
// run and tested from plain Node.
//
// For each version this installs the Java runtime Mojang specifies for it, the Fabric profile and
// the version's mods into its own instance folder, then hands over to minecraft-launcher-core for
// the vanilla files. Later launches only check that files exist, so the wait is the game's own
// start-up. Libraries, assets and runtimes are shared by all versions.

import fs from 'fs';
import os from 'os';
import path from 'path';
import { createRequire } from 'module';
import { extractZip } from './unzip.js';
import { PARALLEL, get, getJson, download, inParallel } from './net.js';
import { VERSIONS, profileId } from './versions.js';
import { ensureMods } from './mods.js';
import { latestFestVisuals } from './feed.js';

const require = createRequire(import.meta.url);
const { Client } = require('minecraft-launcher-core');
const Handler = require('minecraft-launcher-core/components/handler.js');

const VERSION_MANIFEST = 'https://piston-meta.mojang.com/mc/game/version_manifest_v2.json';
const JAVA_RUNTIMES = 'https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json';
const FULL_CHECK_EVERY_MS = 7 * 24 * 60 * 60 * 1000;

// minecraft-launcher-core SHA-1 hashes every asset and library (thousands of files) on every
// launch. After one full verification the fast path only checks that each file exists — anything
// missing is still downloaded — and a full check runs again weekly or after a launch that did not
// reach the game window.
let fastVerify = false;
const originalCheckSum = Handler.prototype.checkSum;
Handler.prototype.checkSum = function (hash, file) {
  // Fast path still rejects empty files, so a zero-byte download is re-fetched rather than kept.
  if (fastVerify) {
    try { return Promise.resolve(fs.statSync(file).size > 0); } catch { return Promise.resolve(false); }
  }
  return originalCheckSum.call(this, hash, file);
};

// The stock check parses `java -version` output and throws inside the callback when it doesn't
// look as expected (javaw prints nothing), which leaves the launch hanging forever.
Handler.prototype.checkJava = function (java) {
  if (path.isAbsolute(java)) {
    return Promise.resolve(fs.existsSync(java) ? { run: true } : { run: false, message: `${java} not found` });
  }
  return new Promise((resolve) => {
    require('child_process').execFile(java, ['-version'], (error) => {
      resolve(error ? { run: false, message: error.message } : { run: true });
    });
  });
};


const originalGetNatives = Handler.prototype.getNatives;
Handler.prototype.getNatives = async function () {
  const nativeDirectory = path.resolve(this.options.overrides.natives || path.join(this.options.root, 'natives', this.version.id));
  if (parseInt(this.version.id.split('.')[1]) >= 19) return this.options.overrides.cwd || this.options.root;
  if (!fs.existsSync(nativeDirectory) || !fs.readdirSync(nativeDirectory).length) {
    fs.mkdirSync(nativeDirectory, { recursive: true });
    const stat = [];
    for (const lib of this.version.libraries) {
      if (!lib.downloads || !lib.downloads.classifiers || this.parseRule(lib)) continue;
      const native = this.getOS() === 'osx' ? lib.downloads.classifiers['natives-osx'] || lib.downloads.classifiers['natives-macos'] : lib.downloads.classifiers[`natives-${this.getOS()}`];
      if (native) stat.push(native);
    }
    this.client.emit('progress', { type: 'natives', task: 0, total: stat.length });
    let counter = 0;
    await inParallel(stat, PARALLEL, async (native) => {
      const name = native.path.split('/').pop();
      const target = path.join(nativeDirectory, name);
      await this.downloadAsync(native.url, nativeDirectory, name, true, 'natives');
      if (!await this.checkSum(native.sha1, target)) {
        await this.downloadAsync(native.url, nativeDirectory, name, true, 'natives');
      }
      try {
        extractZip(target, nativeDirectory);
      } catch (e) {
        console.warn('Zip extract error:', e);
      }
      fs.unlinkSync(target);
      this.client.emit('progress', { type: 'natives', task: ++counter, total: stat.length });
    });
    this.client.emit('debug', '[MCLC]: Downloaded and extracted natives using Bare-Metal ZIP and Worker Pool');
  }
  return nativeDirectory;
};

const originalGetAssets = Handler.prototype.getAssets;
Handler.prototype.getAssets = async function () {
  const assetDirectory = path.resolve(this.options.overrides.assetRoot || path.join(this.options.root, 'assets'));
  const assetId = this.options.version.custom || this.options.version.number;
  const indexFile = path.join(assetDirectory, 'indexes', `${assetId}.json`);
  if (!fs.existsSync(indexFile)) {
    await this.downloadAsync(this.version.assetIndex.url, path.join(assetDirectory, 'indexes'), `${assetId}.json`, true, 'asset-json');
  }
  const index = JSON.parse(fs.readFileSync(indexFile, { encoding: 'utf8' }));
  const objects = Object.keys(index.objects);
  this.client.emit('progress', { type: 'assets', task: 0, total: objects.length });
  let counter = 0;
  await inParallel(objects, PARALLEL, async (asset) => {
    const hash = index.objects[asset].hash;
    const subhash = hash.substring(0, 2);
    const subAsset = path.join(assetDirectory, 'objects', subhash);
    const target = path.join(subAsset, hash);
    if (!fs.existsSync(target) || !await this.checkSum(hash, target)) {
      await this.downloadAsync(`${this.options.overrides.url.resource}/${subhash}/${hash}`, subAsset, hash, true, 'assets');
    }
    this.client.emit('progress', { type: 'assets', task: ++counter, total: objects.length });
  });
  
  if (this.isLegacy()) {
    const legacyDirectory = path.join(this.options.root, 'resources');
    this.client.emit('debug', `[MCLC]: Copying assets over to ${legacyDirectory}`);
    this.client.emit('progress', { type: 'assets-copy', task: 0, total: objects.length });
    let copyCounter = 0;
    await inParallel(objects, PARALLEL, async (asset) => {
      const hash = index.objects[asset].hash;
      const subhash = hash.substring(0, 2);
      const subAsset = path.join(assetDirectory, 'objects', subhash);
      const legacyAsset = asset.split('/');
      legacyAsset.pop();
      if (!fs.existsSync(path.join(legacyDirectory, legacyAsset.join('/')))) {
        fs.mkdirSync(path.join(legacyDirectory, legacyAsset.join('/')), { recursive: true });
      }
      fs.copyFileSync(path.join(subAsset, hash), path.join(legacyDirectory, asset));
      this.client.emit('progress', { type: 'assets-copy', task: ++copyCounter, total: objects.length });
    });
  }
  return assetDirectory;
};

const originalGetClasses = Handler.prototype.getClasses;
Handler.prototype.getClasses = async function (classJson) {
  let libs = [];
  const libraryDirectory = path.resolve(this.options.overrides.libraryRoot || path.join(this.options.root, 'libraries'));
  if (classJson) {
    if (classJson.mavenFiles) await this.downloadToDirectory(libraryDirectory, classJson.mavenFiles, 'classes-maven-custom');
    libs = await this.downloadToDirectory(libraryDirectory, classJson.libraries, 'classes-custom');
  }
  const parsed = this.version.libraries.filter(lib => {
    if (lib.downloads && lib.downloads.artifact && !this.parseRule(lib)) return true;
    return false;
  });
  this.client.emit('progress', { type: 'classes', task: 0, total: parsed.length });
  let counter = 0;
  await inParallel(parsed, PARALLEL, async (library) => {
    const lib = library.name.split(':');
    const jarPath = path.join(libraryDirectory, `${lib[0].replace(/\./g, '/')}/${lib[1]}/${lib[2]}`);
    const name = `${lib[1]}-${lib[2]}${lib[3] ? '-' + lib[3] : ''}.jar`;
    const target = path.join(jarPath, name);
    const downloadLibrary = async (libObj) => {
      if (libObj.url) {
        const url = `${libObj.url}${lib[0].replace(/\./g, '/')}/${lib[1]}/${lib[2]}/${name}`;
        await this.downloadAsync(url, jarPath, name, true, 'classes');
      } else if (libObj.downloads && libObj.downloads.artifact && libObj.downloads.artifact.url) {
        await this.downloadAsync(libObj.downloads.artifact.url, jarPath, name, true, 'classes');
      }
    };
    if (!fs.existsSync(target)) await downloadLibrary(library);
    if (library.downloads && library.downloads.artifact) {
      if (!await this.checkSum(library.downloads.artifact.sha1, target)) await downloadLibrary(library);
    }
    this.client.emit('progress', { type: 'classes', task: ++counter, total: parsed.length });
    libs.push(`${jarPath}${path.sep}${name}`);
  });
  return libs;
};


/**
 * A jar is only sound if it ends with the ZIP end-of-central-directory record. An interrupted or
 * empty download leaves a truncated or zero-byte file that minecraft-launcher-core happily keeps
 * (its re-download-on-bad-checksum path is broken), and the game then dies with
 * "zip file is empty" / "error reading ...jar". We delete such files so the next launch refetches.
 */
function isWholeZip(file) {
  let fd;
  try {
    const size = fs.statSync(file).size;
    if (size < 22) return false; // smaller than an empty zip's EOCD record
    fd = fs.openSync(file, 'r');
    const span = Math.min(size, 65557); // max comment length + EOCD size
    const buf = Buffer.alloc(span);
    fs.readSync(fd, buf, 0, span, size - span);
    return buf.indexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06])) !== -1; // "PK\x05\x06"
  } catch {
    return false;
  } finally {
    if (fd !== undefined) try { fs.closeSync(fd); } catch {}
  }
}

/** Removes every corrupt jar under the game folder so the launcher re-downloads clean copies. */
function removeCorruptJars(root) {
  const roots = [path.join(root, 'libraries'), path.join(root, 'versions')];
  let removed = 0;
  const walk = (dir) => {
    let entries;
    try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith('.jar') && !isWholeZip(full)) {
        try { fs.rmSync(full, { force: true }); removed++; } catch {}
      }
    }
  };
  roots.forEach(walk);
  return removed;
}

function markerPath(root) { return path.join(root, '.festvisuals-verified'); }
function verifiedRecently(root) {
  try {
    return Date.now() - fs.statSync(markerPath(root)).mtimeMs < FULL_CHECK_EVERY_MS;
  } catch {
    return false;
  }
}

function defaultRoot() {
  const base = process.env.APPDATA || path.join(os.homedir(), '.local', 'share');
  return path.join(base, '.festvisuals');
}

/** The requested folder if it can be created (a missing D: drive can't), otherwise the default. */
function usableRoot(requested) {
  for (const candidate of [requested, defaultRoot()]) {
    if (!candidate) continue;
    try {
      fs.mkdirSync(candidate, { recursive: true });
      return path.resolve(candidate);
    } catch {
      // try the next one
    }
  }
  throw new Error('Не удалось создать папку игры');
}


/** Mojang's own Java build for the version (8, 21 or 25), unpacked once into <root>/runtime. */
async function ensureJava(root, component, progress) {
  if (process.platform !== 'win32') return 'java';

  const home = path.join(root, 'runtime', component);
  const javaw = path.join(home, 'bin', 'javaw.exe');
  const done = path.join(home, '.complete');
  if (fs.existsSync(done) && fs.existsSync(javaw)) return javaw;

  const platform = os.arch() === 'arm64' ? 'windows-arm64' : 'windows-x64';
  const all = await getJson(JAVA_RUNTIMES);
  // Old runtimes (Java 8) have no arm64 build; Windows on ARM runs the x64 one.
  const entry = (all[platform] && all[platform][component] && all[platform][component][0])
    || (all['windows-x64'][component] && all['windows-x64'][component][0]);
  if (!entry) throw new Error(`Нет Java (${component}) для ${platform}`);
  const manifest = await getJson(entry.manifest.url);

  const files = Object.entries(manifest.files).filter(([, f]) => f.type === 'file');
  let finished = 0;
  progress('java', 0);
  await inParallel(files, PARALLEL, async ([relative, file]) => {
    const dest = path.join(home, relative);
    const raw = file.downloads.raw;
    if (!fs.existsSync(dest) || fs.statSync(dest).size !== raw.size) {
      await download(raw.url, dest, raw.sha1);
    }
    progress('java', Math.round((++finished / files.length) * 100));
  });
  fs.writeFileSync(done, entry.version.name);
  return javaw;
}

/**
 * The vanilla version JSON, saved where minecraft-launcher-core looks for it
 * (versions/<profile>/<version>.json) so it is read from disk instead of the network.
 */
async function ensureVersionJson(root, version) {
  const file = path.join(root, 'versions', profileId(version), `${version}.json`);
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    // not there yet, or damaged
  }
  const manifest = await getJson(VERSION_MANIFEST);
  const entry = manifest.versions.find((v) => v.id === version);
  if (!entry) throw new Error(`Mojang не знает версию ${version}`);
  await download(entry.url, file, entry.sha1);
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

/** The Fabric version profile minecraft-launcher-core reads as a "custom" version. */
async function ensureFabricProfile(root, version) {
  const id = profileId(version);
  const file = path.join(root, 'versions', id, `${id}.json`);
  if (fs.existsSync(file)) return;
  const profile = await get(`https://meta.fabricmc.net/v2/versions/loader/${version}/${VERSIONS[version].loader}/profile/json`);
  JSON.parse(profile.toString('utf8')); // refuse to save an error page
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, profile);
}

// What stays in the root after the switch to per-version folders: files shared by all versions.
const SHARED = new Set(['assets', 'libraries', 'versions', 'runtime', 'natives', 'cache', 'instances',
  '.festvisuals-verified', 'launcher_profiles.json']);

/**
 * Before multiple versions the game folder was the root itself. Move that 26.2 install (mods,
 * FestVisuals configs, worlds, options, ...) into instances/26.2 once, so nothing is lost.
 */
function migrateLegacyLayout(root) {
  const target = path.join(root, 'instances', '26.2');
  if (fs.existsSync(target)) return;
  const mods = path.join(root, 'mods');
  const looksLikeOurs = fs.existsSync(path.join(root, 'FestVisuals'))
    || (fs.existsSync(mods) && fs.readdirSync(mods).some((name) => /^festvisuals.*\.jar$/i.test(name)));
  if (!looksLikeOurs) return;

  fs.mkdirSync(target, { recursive: true });
  for (const name of fs.readdirSync(root)) {
    if (SHARED.has(name)) continue;
    try {
      fs.renameSync(path.join(root, name), path.join(target, name));
    } catch {
      // in use or not movable; it simply stays where it was
    }
  }
}

/**
 * Directory for the JDK's loopback sockets. The default is %TEMP%, which breaks when that path is
 * long or not ASCII (e.g. a Cyrillic user name) — and with it the game's networking.
 */
function socketTmpDir() {
  const dir = path.join(process.env.ProgramData || 'C:\\ProgramData', 'FestVisuals', 'tmp');
  if (!/^[\x20-\x7e]+$/.test(dir)) return null;
  try {
    fs.mkdirSync(dir, { recursive: true });
    return dir;
  } catch {
    return null;
  }
}

/** Assets are content-addressed, so an existing vanilla install can lend its copies. */
function sharedAssets() {
  const vanilla = path.join(process.env.APPDATA || os.homedir(), '.minecraft', 'assets');
  return fs.existsSync(path.join(vanilla, 'objects')) ? vanilla : undefined;
}

/** Folder of one version's game data (mods, configs, worlds) under the launcher root. */
export function instanceDir(root, version) {
  return path.join(root, 'instances', version);
}

/**
 * Prepares everything for one version and starts it.
 * emit(channel, payload) reports: launch-progress {type, percent}, launch-success, game-ready,
 * game-closed (code), launch-error (message).
 */
export async function launchGame({ version, root: requestedRoot, ram, username, bundledJar, window, vulkan }, emit) {
  const cfg = VERSIONS[version];
  if (!cfg) throw new Error(`Неизвестная версия ${version}`);

  // One message per visible change: thousands of per-file updates only clog the IPC channel.
  const shown = {};
  const progress = (type, percent) => {
    if (shown[type] === percent) return;
    shown[type] = percent;
    emit('launch-progress', { type, percent });
  };

  const root = usableRoot(requestedRoot);
  migrateLegacyLayout(root);
  const gameDir = instanceDir(root, version);
  fs.mkdirSync(gameDir, { recursive: true });

  const versionJson = await ensureVersionJson(root, version);
  const javaComponent = (versionJson.javaVersion && versionJson.javaVersion.component) || 'jre-legacy';
  const warm = verifiedRecently(root);
  const cachedFest = path.join(root, 'cache', 'festvisuals', 'festvisuals.jar');
  // A warm install skips the feed and the jar walk. The first launch still checks everything.
  const festvisualsJar = !cfg.festvisuals ? null
    : (warm && fs.existsSync(cachedFest) ? cachedFest : await latestFestVisuals(root, bundledJar, progress));

  const [javaPath] = await Promise.all([
    ensureJava(root, javaComponent, progress),
    ensureFabricProfile(root, version),
    ensureMods(gameDir, version, { festvisualsJar, progress, vulkan }),
  ]);

  // A warm install already proved the jars. Walking them again is the slow part of a repeat launch.
  if (!warm) removeCorruptJars(root);
  else progress('instant', 100);

  const launcher = new Client();
  let windowOpen = false;
  let closed = false;
  let lastLines = [];
  launcher.on('progress', (e) => {
    progress(e.type, e.total ? Math.round((e.task / e.total) * 100) : 0);
  });
  launcher.on('data', (line) => {
    const text = String(line);
    lastLines = lastLines.concat(text.split(/\r?\n/).filter(Boolean)).slice(-20);
    // LWJGL reports its backend as the game window is created: from here the player sees the game.
    if (!windowOpen && (text.includes('Backend library:') || text.includes('Sound engine started'))) {
      windowOpen = true;
      try { fs.writeFileSync(markerPath(root), String(Date.now())); } catch {}
      emit('game-ready');
    }
  });
  launcher.on('debug', (line) => {
    lastLines = lastLines.concat(String(line)).slice(-20);
  });
  launcher.on('close', (code) => {
    closed = true;
    if (!windowOpen) {
      // A launch that never opened the window may have hit a broken file: verify fully next time.
      try { fs.rmSync(markerPath(root), { force: true }); } catch {}
      if (code !== 0) emit('launch-error', `Игра закрылась (код ${code}).\n${lastLines.slice(-6).join('\n')}`);
    }
    emit('game-closed', code);
  });

  // Minecraft's start-up crash-report preview asks OSHI for process counters; where Windows'
  // counter registry is damaged that falls back to WMI and stalls the window for up to minutes.
  const customArgs = ['-Doshi.os.windows.perfproc.disabled=true', '-Doshi.util.wmi.timeout=2000'];
  const sockets = socketTmpDir();
  if (sockets) customArgs.push(`-Djdk.net.unixdomain.tmpdir=${sockets}`);

  fastVerify = verifiedRecently(root);
  const process_ = await launcher.launch({
    authorization: {
      access_token: '0',
      client_token: '0',
      uuid: '00000000-0000-0000-0000-000000000000',
      name: username || 'Player',
      user_properties: '{}',
      meta: { type: 'offline', demo: false },
    },
    root,
    javaPath,
    version: { number: version, type: 'release', custom: profileId(version) },
    memory: { max: `${ram}M`, min: `${Math.max(512, Math.floor(ram / 2))}M` },
    customArgs,
    window,
    overrides: {
      maxSockets: PARALLEL,
      assetRoot: sharedAssets(),
      gameDirectory: gameDir,
      cwd: gameDir, // FestVisuals keeps its configs relative to the working directory
    },
  });
  if (!process_) {
    // A failed Java check is reported through 'close'; any other failure only leaves a debug line.
    if (!closed) throw new Error(lastLines.slice(-3).join('\n') || 'Не удалось запустить игру');
    return root;
  }
  // The process is up, but the window takes a while: the UI says "starting" until game-ready.
  emit('launch-success');
  return root;
}
