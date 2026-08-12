import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * A menu entry that emits an event nobody listens for is a control that is not
 * a control.
 *
 * The desktop menu declares an id per item. `src-tauri/src/menu.rs` handles two
 * of them natively - quit and fullscreen - and emits every other one as
 * `menu-action`, which `viewer/src/desktop.ts` switches on. Anything the switch
 * misses reaches `default:` and writes a console line, which is what Edit >
 * Undo, Redo, Cut, Copy and Paste did: five entries that looked like the app's
 * own features and did nothing when clicked.
 *
 * This reads `menu.rs` as text, because the wiring is a question about two
 * lists and does not need a compiler. It used to say here that nothing else in
 * the project could read that file, since the desktop crate needed system GTK
 * and webkit that nothing installed and `cargo test --workspace` excluded it.
 * That stopped being true on 2026-08-12: the dependencies are in
 * `scripts/setup-dev.sh`, the gate covers the crate, and
 * `src-tauri/tests/the_menu_handles_what_it_declares.rs` asks the same
 * question of the data model `create_app_menu()` actually returns.
 *
 * Both are worth keeping. This one is the only half that can see
 * `desktop.ts`'s switch; the Rust one is the only half that can see an id
 * built rather than written as a literal.
 */

const here = dirname(fileURLToPath(import.meta.url));
const viewer = join(here, '..', '..');
const repo = join(viewer, '..');

const menuSource = readFileSync(join(repo, 'src-tauri', 'src', 'menu.rs'), 'utf8');
const desktopSource = readFileSync(join(viewer, 'src', 'desktop.ts'), 'utf8');

/** Every id the native menu declares, e.g. `file.open`. */
function declaredIds(): string[] {
  const found = menuSource.matchAll(/action\("([a-z]+\.[a-z_]+)"/g);
  return [...new Set([...found].map((m) => m[1]))];
}

/** The ids `menu.rs` acts on itself instead of emitting. */
function handledNatively(): string[] {
  // The match arms above the catch-all, which is the one that emits.
  const body = menuSource.slice(menuSource.indexOf('pub fn handle_menu_event'));
  const found = body.matchAll(/"([a-z]+\.[a-z_]+)"\s*=>/g);
  return [...new Set([...found].map((m) => m[1]))];
}

/** The ids the frontend switches on. */
function handledInTheFrontend(): string[] {
  const found = desktopSource.matchAll(/case '([a-z]+\.[a-z_]+)':/g);
  return [...new Set([...found].map((m) => m[1]))];
}

describe('every native menu item does something', () => {
  it('declares menu items at all, or this test is vacuous', () => {
    expect(declaredIds().length).toBeGreaterThan(5);
  });

  it('leaves no declared id without a handler on either side', () => {
    const handled = new Set([...handledNatively(), ...handledInTheFrontend()]);
    const dead = declaredIds().filter((id) => !handled.has(id));

    expect(dead, `menu entries that emit an event nobody handles: ${dead.join(', ')}`).toEqual([]);
  });

  it('handles nothing the menu does not offer', () => {
    // The other direction: a case left behind after an item is removed is dead
    // code that reads like a feature.
    const declared = new Set(declaredIds());
    const orphans = handledInTheFrontend().filter((id) => !declared.has(id));

    expect(orphans, `handlers for menu entries that no longer exist: ${orphans.join(', ')}`).toEqual(
      []
    );
  });

  it('undo and redo are wired, because the app has them', () => {
    // The two that matter most: they are real features with a keyboard
    // shortcut that worked while the menu item did not.
    expect(handledInTheFrontend()).toContain('edit.undo');
    expect(handledInTheFrontend()).toContain('edit.redo');
  });
});
