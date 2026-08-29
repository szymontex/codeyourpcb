/**
 * Development server for CodeYourPCB hot reload.
 *
 * Watches .cypcb files and notifies connected browsers via WebSocket
 * when files change. Spawns Vite dev server as child process.
 *
 * Usage: npx tsx server.ts [watch-dir]
 * Default watch directory: ../examples
 */

import { readFileSync, existsSync, readdirSync, writeFileSync, realpathSync } from 'fs';
import { resolve, join, basename, dirname, sep } from 'path';
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

// The port the browser connects to. Overridable so a test can take a free one:
// with this pinned, a test spawning the server while a developer's own is
// running silently talked to theirs - the new process failed to bind and the
// probe connected to the old one, which is a green test measuring nothing.
const WS_PORT = Number(process.env.CYPCB_WS_PORT ?? 4322);
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

// The port it really got, which is the only way to know when WS_PORT is 0.
// A test that guesses a port talks to whatever is already listening on it -
// measured on 2026-08-10, when 39 leftover servers had accumulated from
// earlier runs and one of them answered a test's questions about a directory
// it had never heard of, so the guard looked broken and was not.
wss.on('listening', () => {
  const address = wss.address();
  const port = typeof address === 'object' && address ? address.port : WS_PORT;
  console.log(`[WS] listening on ${port}`);
});

wss.on('connection', (ws, request) => {
  // A browser sends `Origin` and cannot be talked out of it; WebSocket is not
  // subject to CORS, so any page a developer visits could open this socket and
  // start naming files. Every path is checked against the watched directory,
  // which bounds the damage - but a page could still overwrite the boards in
  // it. Only a page served by the same host as this socket gets in, and a
  // client with no `Origin` at all is not a browser.
  const origin = request.headers.origin;
  if (origin && !allowedOrigin(origin, request.headers.host)) {
    console.warn(`[WS] Refused a connection from ${origin}`);
    ws.close(1008, 'Only a page served locally may talk to this server');
    return;
  }

  console.log('[WS] Client connected');
  clients.add(ws);

  ws.on('close', () => {
    clients.delete(ws);
    console.log('[WS] Client disconnected');
  });

  ws.on('error', (err) => {
    console.error('[WS] Client error:', reason(err));
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
/**
 * Whether a page is allowed to talk to this server.
 *
 * The rule is same-host, not localhost: this server is reached over the LAN as
 * well - a board is often looked at from another machine on `192.168.x.y:7001`
 * - and a localhost-only rule would refuse the browser that is actually being
 * used. A page served by the same host the socket was opened on is the page
 * this project serves; anything else is somebody else's page reaching for a
 * developer's disk.
 */
function allowedOrigin(origin: string, host: string | undefined): boolean {
  let hostname: string;
  try {
    hostname = new URL(origin).hostname;
  } catch {
    return false;
  }

  if (hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '[::1]') return true;
  if (!host) return false;

  // `Host` is `name:port`, and an IPv6 literal keeps its brackets.
  const bare = host.startsWith('[') ? host.slice(0, host.indexOf(']') + 1) : host.split(':')[0];
  return hostname === bare;
}

/**
 * The file a client asked for, if it is really inside the watched directory.
 *
 * This server speaks WebSocket on localhost with no authentication and no
 * origin check, so anything it will do, any page in the browser can ask it to
 * do. It used to read whatever path a message named and write whatever a
 * message named - `readFileSync(message.file)` and
 * `writeFileSync(message.file, message.content)` with nothing in between - so
 * a page could have read a developer's keys or written over their shell
 * profile. Every path a client names goes through here now: relative paths
 * are resolved against the watched directory and anything that climbs out of
 * it with `..` or an absolute path is refused.
 */
/**
 * What went wrong, out of whatever was thrown.
 *
 * `catch (err: any)` reads `err.message` off anything at all; an `Error` is
 * only the usual case, not a guarantee.
 */
function reason(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function insideWatchDir(requested: string): string | null {
  const root = realpathOr(resolve(WATCH_DIR));
  const full = resolve(resolve(WATCH_DIR), requested);
  if (!contains(root, full)) return null;

  // And where the path actually leads, which is not the same question.
  //
  // `resolve` is lexical: it collapses `..` and nothing else, so a symlink
  // sitting in the watched directory and pointing at `/etc` passes it and the
  // handler above reads or writes through the link. The directory belongs to
  // whoever runs the server, but the requests do not - a page in the browser
  // can talk to this socket - so the answer has to be about the file the
  // system would open rather than about the text of the path.
  //
  // A file that is not there yet is judged by its parent, because that is what
  // a save creates it in.
  const led = existsSync(full) ? realpathOr(full) : join(realpathOr(dirname(full)), basename(full));
  return contains(root, led) ? full : null;
}

/** Whether `path` is `root` or sits under it, by directory rather than by prefix. */
function contains(root: string, path: string): boolean {
  return path === root || path.startsWith(root + sep);
}

/** The path a symlink leads to, or the path itself when it leads nowhere new. */
function realpathOr(path: string): string {
  try {
    return realpathSync(path);
  } catch {
    return path;
  }
}

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

// Start Vite dev server as child process.
//
// `CYPCB_NO_VITE=1` runs the WebSocket half on its own, which is what a test
// of this protocol wants: a second Vite on the same port would fight with the
// one Playwright started, and the file requests do not need it.
const vite = process.env.CYPCB_NO_VITE === '1' ? null : startVite();

function startVite() {
  console.log('');
  console.log('[Vite] Starting development server...');
  console.log('-'.repeat(50));

  const child = spawn('npx', ['vite'], {
    stdio: 'inherit',
    shell: true,
    cwd: process.cwd(),
  });

  child.on('error', (err) => {
    console.error('[Vite] Failed to start:', err);
  });

  child.on('exit', (code) => {
    console.log(`[Vite] Exited with code ${code}`);
    process.exit(code ?? 1);
  });

  return child;
}

// Clean shutdown
function shutdown(): void {
  console.log('\n[Server] Shutting down...');

  // Close WebSocket server
  wss.close();

  // Close file watcher
  watcher.close();

  // Kill Vite, when there is one
  vite?.kill();

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
/**
 * What a client can ask for.
 *
 * Every field is optional because it arrives as JSON from a browser: the
 * handlers check what they need, and the ones that name a file put it through
 * `insideWatchDir` before touching the disk.
 */
interface ClientMessage {
  type?: string;
  file?: string;
  path?: string;
  content?: string;
}

function handleClientMessage(ws: WebSocket, message: ClientMessage): void {
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
    case 'open-file':
      handleOpenFileRequest(ws, message);
      break;
    case 'read-file':
      handleReadFileRequest(ws, message);
      break;
    default:
      console.log(`[WS] Unknown message type: ${message.type}`);
  }
}

/**
 * Find CLI binary (release or debug)
 */
function findCliBinary(): string | null {
  // `CYPCB_CLI_BIN` names the binary outright, and a name that is not there is
  // a machine with no binary. It exists so the guards above the spawn can be
  // tested on a machine that has one: deleting the build to check the order of
  // two `if`s is not a test anybody runs twice.
  const named = process.env.CYPCB_CLI_BIN;
  if (named) return existsSync(named) ? named : null;
  if (existsSync(CLI_PATH)) return CLI_PATH;
  if (existsSync(CLI_DEBUG_PATH)) return CLI_DEBUG_PATH;
  return null;
}

/**
 * Handle routing request - runs cypcb route command
 */
function handleRouteRequest(ws: WebSocket, message: { file?: string; content?: string }): void {
  // The path is checked before anything else about this request, including
  // whether there is a binary to run.
  //
  // The check used to sit second, after the missing-binary bail-out, and a
  // scheduled gate run against a fresh checkout found it: with no binary
  // built, a request naming a file outside the watched directory was answered
  // `CLI binary not found` and the guard never ran at all. Nothing escaped -
  // there was nothing to run - but a guard that only runs when an earlier
  // bail-out does not fire is a guard whose reach depends on the order of the
  // lines above it, and the next bail-out added above would move it again.
  //
  // Determine file path. The client names it, so it is checked like every
  // other path a client names: routing runs a program with this as its
  // argument and its directory as the working directory, and afterwards the
  // handler reads `<file>.ses` and `<file>.routes` and sends them back - so an
  // unchecked path here is both a write and a read of anywhere on the disk.
  let filePath: string;
  const named = message.file ? insideWatchDir(message.file) : null;
  if (message.file && !named) {
    ws.send(JSON.stringify({
      type: 'route-error',
      error: `Refused: ${message.file} is outside the watched directory`,
    }));
    return;
  }
  if (named && existsSync(named)) {
    filePath = named;
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

  const cliBinary = findCliBinary();
  if (!cliBinary) {
    ws.send(JSON.stringify({
      type: 'route-error',
      error: 'CLI binary not found. Run: cargo build --release -p cypcb-cli'
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
      error: `Failed to start routing: ${reason(err)}`,
    }));
  });
}

/**
 * Handle save request - saves content to file
 */
/**
 * Hand a file's text back, for a design that imports it.
 *
 * The engine resolves `import "lib/blocks.cypcb"` and cannot read a file: a
 * browser tab has no disk. A design opened from this server's watched
 * directory has its library beside it on the same disk, and this is the one
 * request that lets the page fetch it. The path is relative to the watched
 * directory and is checked, like every other path a client names.
 */
function handleReadFileRequest(ws: WebSocket, message: { path?: string }): void {
  const requested = message.path;
  if (!requested) {
    ws.send(JSON.stringify({ type: 'file-content', path: '', error: 'No path specified' }));
    return;
  }

  const filePath = insideWatchDir(requested);
  if (!filePath || !existsSync(filePath)) {
    ws.send(JSON.stringify({
      type: 'file-content',
      path: requested,
      error: `Cannot read ${requested}`,
    }));
    return;
  }

  try {
    ws.send(JSON.stringify({
      type: 'file-content',
      path: requested,
      content: readFileSync(filePath, 'utf-8'),
    }));
  } catch (err) {
    ws.send(JSON.stringify({ type: 'file-content', path: requested, error: reason(err) }));
  }
}

function handleSaveRequest(ws: WebSocket, message: ClientMessage): void {
  // `content` may be empty: clearing a file is a save like any other, and
  // `!message.content` refused it.
  if (!message.file || message.content === undefined) {
    ws.send(JSON.stringify({
      type: 'save-error',
      error: 'Missing file path or content',
    }));
    return;
  }

  const target = insideWatchDir(message.file);
  if (!target) {
    ws.send(JSON.stringify({
      type: 'save-error',
      error: `Refused: ${message.file} is outside the watched directory`,
    }));
    return;
  }

  try {
    writeFileSync(target, message.content, 'utf-8');
    console.log(`[Save] Saved: ${target}`);
    ws.send(JSON.stringify({
      type: 'save-complete',
      file: target,
    }));
  } catch (err) {
    console.error(`[Save] Error: ${reason(err)}`);
    ws.send(JSON.stringify({
      type: 'save-error',
      error: reason(err),
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

/**
 * Handle open file request — read a specific .cypcb and send as reload
 */
function handleOpenFileRequest(ws: WebSocket, message: { file?: string }): void {
  if (!message.file) {
    ws.send(JSON.stringify({ type: 'route-error', error: 'No file specified' }));
    return;
  }

  const filePath = insideWatchDir(message.file);
  if (!filePath) {
    ws.send(JSON.stringify({
      type: 'route-error',
      error: `Refused: ${message.file} is outside the watched directory`,
    }));
    return;
  }
  if (!existsSync(filePath)) {
    ws.send(JSON.stringify({ type: 'route-error', error: `File not found: ${filePath}` }));
    return;
  }

  try {
    const content = readFileSync(filePath, 'utf-8');
    ws.send(JSON.stringify({
      type: 'reload',
      file: filePath,
      content,
      timestamp: Date.now(),
    }));
    console.log(`[Open] Sent file: ${basename(filePath)}`);
  } catch (err) {
    console.error(`[Open] Error reading file: ${reason(err)}`);
  }
}
