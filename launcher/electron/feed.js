// The current FestVisuals build, fetched from the public releases repo so players get mod updates
// without reinstalling the launcher. CI keeps a rolling "mod" release there with the jar and a
// mod.json carrying its version and SHA-1.

import fs from 'fs';
import path from 'path';
import { getJson, download } from './net.js';

const FEED = 'https://github.com/Daniiltoptl/FestVisuals-Releases/releases/download/mod/';

/**
 * Path of the newest FestVisuals jar available: freshly downloaded when the feed has a new build,
 * else the cached copy, else a jar bundled next to the launcher if one was shipped.
 * The published exe does not embed the mod, so the first launch needs the feed.
 */
export async function latestFestVisuals(root, bundledJar, progress) {
  const dir = path.join(root, 'cache', 'festvisuals');
  const jar = path.join(dir, 'festvisuals.jar');
  const metaFile = path.join(dir, 'mod.json');

  try {
    const meta = await getJson(FEED + 'mod.json', { timeout: 8000 });
    let local = null;
    try { local = JSON.parse(fs.readFileSync(metaFile, 'utf8')); } catch {}
    if (!fs.existsSync(jar) || !local || local.sha1 !== meta.sha1) {
      progress('festvisuals', 0);
      await download(FEED + meta.file, jar, meta.sha1);
      fs.writeFileSync(metaFile, JSON.stringify(meta));
      progress('festvisuals', 100);
    }
    return jar;
  } catch {
    if (fs.existsSync(jar)) return jar;
    if (bundledJar && fs.existsSync(bundledJar)) return bundledJar;
    throw new Error('Не удалось скачать мод FestVisuals. Для первого запуска нужен интернет.');
  }
}
