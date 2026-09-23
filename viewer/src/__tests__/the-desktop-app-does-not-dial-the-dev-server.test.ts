import { describe, it, expect, vi, afterEach } from 'vitest';
import { dialDevServer } from '../dev-socket';

/**
 * The desktop app does not open the dev server's socket.
 *
 * Until 2026-09-23 a build with the bundle embedded dialled `localhost:4322`
 * on the user's machine, and a `reload` from whatever answered replaced the
 * editor's text that File > Save writes to disk. `dev-socket.ts` has the why.
 * This holds the decision at the place the socket is constructed.
 */

const URL = 'ws://localhost:4322';

function spyOnTheConstructor() {
  const dialled: string[] = [];
  class Recording {
    constructor(url: string) {
      dialled.push(url);
    }
  }
  vi.stubGlobal('WebSocket', Recording);
  return dialled;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('dialDevServer', () => {
  it('dials from a browser page', () => {
    const dialled = spyOnTheConstructor();
    expect(dialDevServer(URL, false, false)).not.toBeNull();
    expect(dialled).toEqual([URL]);
  });

  it('does not dial from the desktop app', () => {
    const dialled = spyOnTheConstructor();
    expect(dialDevServer(URL, true, false)).toBeNull();
    expect(dialled).toEqual([]);
  });

  it('dials from the desktop app when a developer built it to', () => {
    const dialled = spyOnTheConstructor();
    expect(dialDevServer(URL, true, true)).not.toBeNull();
    expect(dialled).toEqual([URL]);
  });
});
