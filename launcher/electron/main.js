import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'path';
import { fileURLToPath } from 'url';
import http from 'http';
import updater from 'electron-updater';
import { launchGame } from './game.js';
import { startJarvis, stopJarvis } from './jarvis.js';

const { autoUpdater } = updater;
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

let win = null;

/** One game at a time; the renderer draws its buttons from this. */
let game = { state: 'idle', version: null };

function send(channel, payload) {
  if (win && !win.isDestroyed()) win.webContents.send(channel, payload);
}

function setGame(next) {
  game = { ...game, ...next };
  send('game-state', game);
}

// Loopback-only control endpoint for the mod: it asks for the local Jarvis server here. No CORS
// header, so web pages cannot read the answers.
const server = http.createServer(async (req, res) => {
  if (req.url === '/start-jarvis') {
    const result = await startJarvis(app, __dirname);
    res.writeHead(result.ok ? 200 : 500, { 'Content-Type': 'text/plain; charset=utf-8' });
    res.end(result.message);
  } else if (req.url === '/stop-jarvis') {
    stopJarvis();
    res.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' });
    res.end('stopped');
  } else {
    res.writeHead(404);
    res.end();
  }
});
server.on('error', (e) => console.error('Jarvis control port unavailable:', e.message));
server.listen(4567, '127.0.0.1');

/**
 * Checks the public releases repo on start; a newer launcher is downloaded in the background and
 * installed right away (silent NSIS install, then relaunch) unless a game is running — then it is
 * applied when the launcher quits.
 */
function setupUpdater() {
  // electron-updater can only replace an installed (NSIS) launcher; the portable exe updates by
  // downloading a newer one.
  if (!app.isPackaged || process.env.PORTABLE_EXECUTABLE_DIR) return;
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;

  autoUpdater.on('update-available', (info) => send('updater', { state: 'downloading', version: info.version, percent: 0 }));
  autoUpdater.on('download-progress', (p) => send('updater', { state: 'downloading', percent: Math.round(p.percent) }));
  autoUpdater.on('update-downloaded', (info) => {
    send('updater', { state: 'ready', version: info.version });
    if (game.state === 'idle') setTimeout(() => autoUpdater.quitAndInstall(true, true), 2500);
  });
  autoUpdater.on('error', (e) => {
    console.error('Update check failed:', e.message);
    send('updater', { state: 'idle' });
  });
  autoUpdater.checkForUpdates().catch(() => {});
}

function createWindow() {
  win = new BrowserWindow({
    width: 1280,
    height: 820,
    minWidth: 1040,
    minHeight: 720,
    show: false,
    title: 'FestVisuals Launcher',
    icon: path.join(__dirname, 'icon.png'),
    autoHideMenuBar: true,
    frame: false,
    backgroundColor: '#07060b',
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
      spellcheck: false,
    },
  });

  win.once('ready-to-show', () => win && win.show());

  if (!app.isPackaged) {
    win.loadURL('http://localhost:8080');
  } else {
    win.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  ipcMain.on('minimize-window', () => win && win.minimize());
  ipcMain.on('close-window', () => win && win.close());
  ipcMain.on('get-game-state', (event) => event.sender.send('game-state', game));

  ipcMain.on('launch-game', async (event, config) => {
    if (game.state !== 'idle') {
      send('launch-error', { version: config.version, message: `Уже запущена ${game.version}` });
      return;
    }
    const { version, closeOnLaunch } = config;
    setGame({ state: 'launching', version });

    // The FestVisuals jar shipped with the launcher: the offline fallback for the mod feed.
    const bundledJar = app.isPackaged
      ? path.join(process.resourcesPath, 'mod', 'festvisuals.jar')
      : path.join(__dirname, '..', 'mod', 'festvisuals.jar');

    let hidden = false;
    const emit = (channel, payload) => {
      switch (channel) {
        case 'launch-progress':
          setGame({ state: 'downloading', type: payload.type, percent: payload.percent });
          break;
        case 'launch-success':
          if (game.state !== 'running') setGame({ state: 'starting' });
          break;
        case 'game-ready':
          setGame({ state: 'running' });
          if (closeOnLaunch && win) { win.hide(); hidden = true; }
          break;
        case 'game-closed':
          setGame({ state: 'idle', version: null });
          if (hidden && win) { win.show(); hidden = false; }
          break;
        case 'launch-error':
          send('launch-error', { version, message: payload });
          break;
        default:
          break;
      }
    };

    try {
      await launchGame({ ...config, bundledJar }, emit);
    } catch (err) {
      console.error(err);
      send('launch-error', { version, message: err.message });
      setGame({ state: 'idle', version: null });
    }
  });
}

app.whenReady().then(() => {
  createWindow();
  setupUpdater();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  stopJarvis();
  if (process.platform !== 'darwin') app.quit();
});
