import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync, writeFileSync, mkdirSync, existsSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { WebSocket } from 'ws';

/**
 * The dev server hands files to a web page, and used to hand it any file.
 *
 * It speaks WebSocket on localhost with no authentication and no origin check,
 * so anything it will do, any page in the browser can ask it to do - and it
 * read whatever path a message named and wrote whatever a message named:
 * `readFileSync(message.file)` and `writeFileSync(message.file,
 * message.content)` with nothing in between. This drives the real server over
 * the real protocol, because that is the only thing that proves the guard is
 * in the path a message actually takes.
 */

// Not 4322: that is the port a developer's own dev server sits on, and
// spawning a second one there binds nothing and talks to theirs.
const PORT = 4700 + Math.floor(Math.random() * 200);
let server: ChildProcess;
let workspace: string;
let outside: string;

function send(socket: WebSocket, message: object): void {
  socket.send(JSON.stringify(message));
}

/** Open a connection, run one exchange, and wait for the answer that matches. */
async function ask(
  message: object,
  matches: (msg: Record<string, unknown>) => boolean,
): Promise<Record<string, unknown>> {
  const socket = new WebSocket(`ws://127.0.0.1:${PORT}`);
  try {
    await new Promise<void>((resolve, reject) => {
      socket.once('open', () => resolve());
      socket.once('error', reject);
    });

    return await new Promise<Record<string, unknown>>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('the server never answered')), 5_000);
      socket.on('message', (data) => {
        const parsed = JSON.parse(data.toString());
        if (matches(parsed)) {
          clearTimeout(timer);
          resolve(parsed);
        }
      });
      send(socket, message);
    });
  } finally {
    socket.close();
  }
}

beforeAll(async () => {
  workspace = mkdtempSync(join(tmpdir(), 'cypcb-ws-'));
  outside = mkdtempSync(join(tmpdir(), 'cypcb-private-'));
  mkdirSync(join(workspace, 'lib'));
  writeFileSync(join(workspace, 'board.cypcb'), 'version 1\nimport "lib/blocks.cypcb"\n');
  writeFileSync(join(workspace, 'lib', 'blocks.cypcb'), 'version 1\nmodule Divider { pin IN }\n');
  writeFileSync(join(outside, 'secrets.txt'), 'the private key');

  server = spawn('npx', ['tsx', 'server.ts', workspace], {
    cwd: join(__dirname, '..', '..'),
    env: { ...process.env, CYPCB_NO_VITE: '1', CYPCB_WS_PORT: String(PORT) },
    stdio: 'ignore',
  });

  // Wait for the port to answer rather than guessing at a delay.
  const deadline = Date.now() + 20_000;
  for (;;) {
    try {
      const probe = new WebSocket(`ws://127.0.0.1:${PORT}`);
      await new Promise<void>((resolve, reject) => {
        probe.once('open', () => resolve());
        probe.once('error', reject);
      });
      probe.close();
      break;
    } catch {
      if (Date.now() > deadline) throw new Error('the dev server never started');
      await new Promise((r) => setTimeout(r, 250));
    }
  }
}, 30_000);

afterAll(() => {
  server?.kill('SIGTERM');
  rmSync(workspace, { recursive: true, force: true });
  rmSync(outside, { recursive: true, force: true });
});

describe('the dev server guards the disk', () => {
  it('hands over a file inside the watched directory', async () => {
    // The request a design with an `import` makes: the engine cannot read a
    // file, so the page asks the server for the library beside the design.
    const answer = await ask(
      { type: 'read-file', path: join(workspace, 'lib', 'blocks.cypcb') },
      (msg) => msg.type === 'file-content',
    );

    expect(answer.content).toContain('module Divider');
    expect(answer.error).toBeUndefined();
  });

  it('refuses to read a file outside it', async () => {
    const answer = await ask(
      { type: 'read-file', path: join(outside, 'secrets.txt') },
      (msg) => msg.type === 'file-content',
    );

    expect(answer.content).toBeUndefined();
    expect(String(answer.error)).toContain('Cannot read');
  });

  it('refuses to open a file outside it', async () => {
    // `open-file` answers with the file's content as a `reload`, which is how
    // a page would have read anything on the disk.
    const answer = await ask(
      { type: 'open-file', file: join(outside, 'secrets.txt') },
      (msg) => msg.type === 'reload' || msg.type === 'route-error',
    );

    expect(answer.type).toBe('route-error');
    expect(String(answer.error)).toContain('outside the watched directory');
  });

  it('refuses to write outside it', async () => {
    const target = join(outside, 'written-by-the-browser.txt');

    const answer = await ask(
      { type: 'save', file: target, content: 'a page wrote this' },
      (msg) => msg.type === 'save-complete' || msg.type === 'save-error',
    );

    expect(answer.type).toBe('save-error');
    expect(existsSync(target), 'a page must not be able to write here').toBe(false);
  });

  it('still saves inside it', async () => {
    const answer = await ask(
      { type: 'save', file: join(workspace, 'board.cypcb'), content: 'version 1\n' },
      (msg) => msg.type === 'save-complete' || msg.type === 'save-error',
    );

    expect(answer.type).toBe('save-complete');
    expect(readFileSync(join(workspace, 'board.cypcb'), 'utf8')).toBe('version 1\n');
  });
});
