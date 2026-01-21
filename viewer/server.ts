/**
 * Development server for CodeYourPCB hot reload.
 *
 * Watches .cypcb files and notifies connected browsers via WebSocket
 * when files change. Spawns Vite dev server as child process.
 *
 * Usage: npx tsx server.ts [watch-dir]
 * Default watch directory: ../examples
 */

import { readFileSync, existsSync, readdirSync } from 'fs';
import { resolve, join } from 'path';
import { WebSocketServer, WebSocket } from 'ws';
import * as chokidar from 'chokidar';
import { spawn } from 'child_process';

const WS_PORT = 4322;
const WATCH_DIR = resolve(process.argv[2] || '../examples');

// Track connected WebSocket clients
const clients = new Set<WebSocket>();

console.log('='.repeat(50));
console.log('CodeYourPCB Development Server');
console.log('='.repeat(50));
console.log(`Watch directory: ${WATCH_DIR}`);
console.log(`WebSocket port: ${WS_PORT}`);
console.log('');

// Create WebSocket server (bind to 0.0.0.0 for external access)
const wss = new WebSocketServer({ port: WS_PORT, host: '0.0.0.0' });

wss.on('connection', (ws) => {
  console.log('[WS] Client connected');
  clients.add(ws);

  ws.on('close', () => {
    clients.delete(ws);
    console.log('[WS] Client disconnected');
  });

  ws.on('error', (err) => {
    console.error('[WS] Client error:', err.message);
    clients.delete(ws);
  });

  // Send current file content on connection
  const files = getCypcbFiles();
  if (files.length > 0) {
    try {
      const content = readFileSync(files[0], 'utf-8');
      ws.send(JSON.stringify({
        type: 'init',
        file: files[0],
        content
      }));
      console.log(`[WS] Sent init with ${files[0]}`);
    } catch (err) {
      console.error('[WS] Failed to send init:', err);
    }
  }
});

wss.on('listening', () => {
  console.log(`[WS] Server listening on ws://localhost:${WS_PORT}`);
});

/**
 * Get list of .cypcb files in watch directory
 */
function getCypcbFiles(): string[] {
  if (!existsSync(WATCH_DIR)) {
    console.warn(`[Watch] Directory does not exist: ${WATCH_DIR}`);
    return [];
  }

  try {
    return readdirSync(WATCH_DIR)
      .filter(f => f.endsWith('.cypcb'))
      .map(f => join(WATCH_DIR, f));
  } catch (err) {
    console.error('[Watch] Failed to read directory:', err);
    return [];
  }
}

/**
 * Broadcast message to all connected clients
 */
function broadcast(message: object): void {
  const data = JSON.stringify(message);
  let sent = 0;

  clients.forEach(client => {
    if (client.readyState === WebSocket.OPEN) {
      client.send(data);
      sent++;
    }
  });

  if (sent > 0) {
    console.log(`[WS] Broadcast to ${sent} client(s)`);
  }
}

// Watch for file changes using chokidar
const watchPattern = join(WATCH_DIR, '**/*.cypcb');
console.log(`[Watch] Pattern: ${watchPattern}`);

const watcher = chokidar.watch(watchPattern, {
  ignoreInitial: true,
  // Wait for file to be fully written (handles editor save patterns)
  awaitWriteFinish: {
    stabilityThreshold: 200,
    pollInterval: 50,
  },
});

watcher.on('ready', () => {
  const files = getCypcbFiles();
  console.log(`[Watch] Ready, found ${files.length} .cypcb file(s)`);
  files.forEach(f => console.log(`  - ${f}`));
});

watcher.on('change', (path) => {
  const timestamp = new Date().toISOString().split('T')[1].slice(0, 8);
  console.log(`[${timestamp}] File changed: ${path}`);

  try {
    const content = readFileSync(path, 'utf-8');
    broadcast({
      type: 'reload',
      file: path,
      content,
      timestamp: Date.now(),
    });
  } catch (err) {
    console.error('[Watch] Error reading file:', err);
  }
});

watcher.on('add', (path) => {
  console.log(`[Watch] File added: ${path}`);
});

watcher.on('unlink', (path) => {
  console.log(`[Watch] File removed: ${path}`);
});

watcher.on('error', (err) => {
  console.error('[Watch] Error:', err);
});

// Start Vite dev server as child process
console.log('');
console.log('[Vite] Starting development server...');
console.log('-'.repeat(50));

const vite = spawn('npx', ['vite'], {
  stdio: 'inherit',
  shell: true,
  cwd: process.cwd(),
});

vite.on('error', (err) => {
  console.error('[Vite] Failed to start:', err);
});

vite.on('exit', (code) => {
  console.log(`[Vite] Exited with code ${code}`);
  process.exit(code ?? 1);
});

// Clean shutdown
function shutdown(): void {
  console.log('\n[Server] Shutting down...');

  // Close WebSocket server
  wss.close();

  // Close file watcher
  watcher.close();

  // Kill Vite
  vite.kill();

  process.exit(0);
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

console.log('');
console.log('Press Ctrl+C to stop');
console.log('');
