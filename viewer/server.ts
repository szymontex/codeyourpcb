/**
 * Development server for CodeYourPCB hot reload.
 *
 * Watches .cypcb files and notifies connected browsers via WebSocket
 * when files change. Spawns Vite dev server as child process.
 *
 * Usage: npx tsx server.ts [watch-dir]
 * Default watch directory: ../examples
 */

import { readFileSync, existsSync, readdirSync, writeFileSync } from 'fs';
import { resolve, join, basename, dirname } from 'path';
import { fileURLToPath } from 'url';
import { WebSocketServer, WebSocket } from 'ws';
import * as chokidar from 'chokidar';
import { spawn } from 'child_process';

// ES module equivalent of __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// CLI binary path (from cargo build)
const CLI_PATH = resolve(__dirname, '../target/release/cypcb');
const CLI_DEBUG_PATH = resolve(__dirname, '../target/debug/cypcb');

// FreeRouting JAR path
const FREEROUTING_JAR = resolve(__dirname, '../freerouting.jar');

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

  // Handle incoming messages from clients
  ws.on('message', (data) => {
    try {
      const message = JSON.parse(data.toString());
      handleClientMessage(ws, message);
    } catch (err) {
      console.error('[WS] Invalid message:', err);
    }
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

/**
 * Handle messages from WebSocket clients
 */
function handleClientMessage(ws: WebSocket, message: any): void {
  console.log(`[WS] Received message: ${message.type}`);

  switch (message.type) {
    case 'route':
      handleRouteRequest(ws, message);
      break;
    case 'save':
      handleSaveRequest(ws, message);
      break;
    case 'list-files':
      handleListFilesRequest(ws);
      break;
    default:
      console.log(`[WS] Unknown message type: ${message.type}`);
  }
}

/**
 * Find CLI binary (release or debug)
 */
function findCliBinary(): string | null {
  if (existsSync(CLI_PATH)) return CLI_PATH;
  if (existsSync(CLI_DEBUG_PATH)) return CLI_DEBUG_PATH;
  return null;
}

/**
 * Handle routing request - runs cypcb route command
 */
function handleRouteRequest(ws: WebSocket, message: { file?: string; content?: string }): void {
  const cliBinary = findCliBinary();
  if (!cliBinary) {
    ws.send(JSON.stringify({
      type: 'route-error',
      error: 'CLI binary not found. Run: cargo build --release -p cypcb-cli'
    }));
    return;
  }

  // Determine file path
  let filePath: string;
  if (message.file && existsSync(message.file)) {
    filePath = message.file;
  } else if (message.content) {
    // Save content to temp file
    filePath = join(WATCH_DIR, '_temp_route.cypcb');
    writeFileSync(filePath, message.content, 'utf-8');
  } else {
    ws.send(JSON.stringify({
      type: 'route-error',
      error: 'No file path or content provided'
    }));
    return;
  }

  console.log(`[Route] Starting route for: ${filePath}`);
  ws.send(JSON.stringify({ type: 'route-start', file: filePath }));

  // Run routing command
  const routeProcess = spawn(cliBinary, ['route', filePath], {
    cwd: dirname(filePath),
    env: { ...process.env, FREEROUTING_JAR },
  });

  let stdout = '';
  let stderr = '';

  routeProcess.stdout.on('data', (data) => {
    const text = data.toString();
    stdout += text;
    // Forward progress updates to client
    ws.send(JSON.stringify({ type: 'route-progress', output: text }));
    console.log(`[Route] ${text.trim()}`);
  });

  routeProcess.stderr.on('data', (data) => {
    stderr += data.toString();
    console.error(`[Route] Error: ${data.toString().trim()}`);
  });

  routeProcess.on('close', (code) => {
    console.log(`[Route] Completed with code ${code}`);

    if (code === 0) {
      // Read .ses file if it was created
      const sesPath = filePath.replace('.cypcb', '.ses');
      let sesContent: string | null = null;
      if (existsSync(sesPath)) {
        sesContent = readFileSync(sesPath, 'utf-8');
      }

      // Read .routes file if created
      const routesPath = filePath.replace('.cypcb', '.routes');
      let routesContent: string | null = null;
      if (existsSync(routesPath)) {
        routesContent = readFileSync(routesPath, 'utf-8');
      }

      ws.send(JSON.stringify({
        type: 'route-complete',
        file: filePath,
        sesContent,
        routesContent,
        output: stdout,
      }));
    } else {
      ws.send(JSON.stringify({
        type: 'route-error',
        error: stderr || `Routing failed with code ${code}`,
        output: stdout,
      }));
    }
  });

  routeProcess.on('error', (err) => {
    console.error(`[Route] Process error: ${err}`);
    ws.send(JSON.stringify({
      type: 'route-error',
      error: `Failed to start routing: ${err.message}`,
    }));
  });
}

/**
 * Handle save request - saves content to file
 */
function handleSaveRequest(ws: WebSocket, message: { file: string; content: string }): void {
  if (!message.file || !message.content) {
    ws.send(JSON.stringify({
      type: 'save-error',
      error: 'Missing file path or content',
    }));
    return;
  }

  try {
    writeFileSync(message.file, message.content, 'utf-8');
    console.log(`[Save] Saved: ${message.file}`);
    ws.send(JSON.stringify({
      type: 'save-complete',
      file: message.file,
    }));
  } catch (err: any) {
    console.error(`[Save] Error: ${err.message}`);
    ws.send(JSON.stringify({
      type: 'save-error',
      error: err.message,
    }));
  }
}

/**
 * Handle list files request
 */
function handleListFilesRequest(ws: WebSocket): void {
  const files = getCypcbFiles();
  ws.send(JSON.stringify({
    type: 'file-list',
    files: files.map(f => ({
      path: f,
      name: basename(f),
    })),
  }));
}
