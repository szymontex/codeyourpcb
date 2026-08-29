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

// The port the server actually bound, read from its own output. Guessing one -
// even a random one - talks to whatever is already listening there: measured
// with 39 leftover servers from earlier runs of this file, one of which
// answered these questions about a directory it had never heard of, so the
// guard looked broken and was not.
let PORT = 0;
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

  // `detached` so the whole group can be killed: `npx` is a wrapper, and
  // signalling it leaves the `tsx server.ts` underneath alive. That is how 39
  // of them accumulated.
  server = spawn('npx', ['tsx', 'server.ts', workspace], {
    cwd: join(__dirname, '..', '..'),
    // No binary, on purpose: the guards above the spawn have to answer before
    // anything about the build does. The order used to be the other way round,
    // and on a machine with nothing built the refusal was `CLI binary not
    // found` - the path was never looked at. A scheduled gate run against a
    // fresh checkout is what found it.
    env: {
      ...process.env,
      CYPCB_NO_VITE: '1',
      CYPCB_WS_PORT: '0',
      CYPCB_CLI_BIN: join(tmpdir(), 'cypcb-no-such-binary'),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: true,
  });

  PORT = await new Promise<number>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('the dev server never said which port')), 25_000);
    let seen = '';
    const read = (chunk: Buffer) => {
      seen += chunk.toString();
      const match = seen.match(/\[WS\] listening on (\d+)/);
      if (match) {
        clearTimeout(timer);
        resolve(Number(match[1]));
      }
    };
    server.stdout?.on('data', read);
    server.stderr?.on('data', read);
    server.once('exit', code => {
      clearTimeout(timer);
      reject(new Error(`the dev server exited with ${code} before listening:\n${seen}`));
    });
  });
}, 40_000);

afterAll(() => {
  // The group, not the wrapper.
  if (server?.pid) {
    try {
      process.kill(-server.pid, 'SIGTERM');
    } catch {
      server.kill('SIGTERM');
    }
  }
  rmSync(workspace, { recursive: true, force: true });
  rmSync(outside, { recursive: true, force: true });
});

describe('the dev server guards the disk', () => {
  it('refuses a browser page it does not serve', async () => {
    // WebSocket is not subject to CORS: any page a developer visits can open
    // this socket. A browser always sends `Origin` and cannot be talked out
    // of it, so a page this project did not serve is refused at the door.
    const socket = new WebSocket(`ws://127.0.0.1:${PORT}`, {
      headers: { Origin: 'https://not-this-project.example' },
    });

    const closed = await new Promise<number>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('the socket stayed open')), 5_000);
      socket.once('close', (code) => {
        clearTimeout(timer);
        resolve(code);
      });
      socket.once('error', () => {});
    });

    expect(closed).toBe(1008);
  });

  it('lets in a page served by the same host', async () => {
    // Not a localhost-only rule: a board is often looked at from another
    // machine on the network, and that browser's origin is the LAN address it
    // loaded the page from - the same host it then opens this socket on.
    const socket = new WebSocket(`ws://127.0.0.1:${PORT}`, {
      headers: { Origin: 'http://127.0.0.1:4321' },
    });
    try {
      await new Promise<void>((resolve, reject) => {
        socket.once('open', () => resolve());
        socket.once('error', reject);
        socket.once('close', () => reject(new Error('refused a page it serves')));
      });
    } finally {
      socket.close();
    }
  });

  it('lets in a page loaded over the network from this machine', async () => {
    // The case a localhost-only rule would have broken: the board is being
    // looked at from another machine, so the page's origin is this machine's
    // LAN address and the socket is opened on the same one.
    const socket = new WebSocket(`ws://127.0.0.1:${PORT}`, {
      headers: { Origin: 'http://192.168.0.10:7001', Host: '192.168.0.10:' + PORT },
    });
    try {
      await new Promise<void>((resolve, reject) => {
        socket.once('open', () => resolve());
        socket.once('error', reject);
        socket.once('close', () => reject(new Error('refused the browser actually in use')));
      });
    } finally {
      socket.close();
    }
  });

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

  it('refuses to route a file outside it', async () => {
    // Routing runs a program with this path as its argument and its directory
    // as the working directory, then reads `<file>.ses` and `<file>.routes`
    // beside it and sends them back - so an unchecked path here is a write and
    // a read of anywhere on the disk.
    const answer = await ask(
      { type: 'route', file: join(outside, 'secrets.txt') },
      (msg) => msg.type === 'route-start' || msg.type === 'route-error',
    );

    expect(answer.type).toBe('route-error');
    expect(String(answer.error)).toContain('outside the watched directory');
  });

  it('saves a file the user emptied', async () => {
    // `!message.content` refused an empty string, so clearing a board and
    // saving it did nothing and said "Missing file path or content".
    const answer = await ask(
      { type: 'save', file: join(workspace, 'emptied.cypcb'), content: '' },
      (msg) => msg.type === 'save-complete' || msg.type === 'save-error',
    );

    expect(answer.type).toBe('save-complete');
    expect(readFileSync(join(workspace, 'emptied.cypcb'), 'utf8')).toBe('');
  });

  it('still saves inside it', async () => {
    const answer = await ask(
      { type: 'save', file: join(workspace, 'board.cypcb'), content: 'version 1\n' },
      (msg) => msg.type === 'save-complete' || msg.type === 'save-error',
    );

    expect(answer.type).toBe('save-complete');
    expect(readFileSync(join(workspace, 'board.cypcb'), 'utf8')).toBe('version 1\n');
  });

  it('really is a server with no binary, which is what makes the case above a test', () => {
    // The guard on the guard: if the override were ignored and the machine's
    // own build were found, the case above would pass whichever order the two
    // checks are in - which is how its first version passed against the very
    // ordering it was written to catch.
    return ask(
      { type: 'route', file: join(workspace, 'board.cypcb') },
      (msg) => msg.type === 'route-start' || msg.type === 'route-error',
    ).then((answer) => {
      expect(answer.type).toBe('route-error');
      expect(String(answer.error)).toContain('CLI binary not found');
    });
  });
});
