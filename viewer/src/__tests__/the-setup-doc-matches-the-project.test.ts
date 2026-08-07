import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * `DESKTOP-SETUP.md` is the page somebody reads before their first build, and
 * every claim in it had been written rather than run.
 *
 * The installer paths named version `0.1.0` while `tauri.conf.json` says
 * `0.1.0-beta`, so all four were files that would never exist. They are gone -
 * a doc that repeats a version is a doc that goes stale on the next release,
 * and the build scripts already print what they produced. This test keeps any
 * version literal that does appear honest, and checks that the scripts and
 * shortcuts the page tells the reader to use are the ones the project has.
 */

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..', '..');

const doc = readFileSync(join(repo, 'DESKTOP-SETUP.md'), 'utf8');
const tauriConfig = JSON.parse(readFileSync(join(repo, 'src-tauri', 'tauri.conf.json'), 'utf8')) as {
  version: string;
  productName: string;
};
const menu = readFileSync(join(repo, 'src-tauri', 'src', 'menu.rs'), 'utf8');

describe('the setup page matches the project it sets up', () => {
  it('names no version the project does not have', () => {
    // No word boundaries: the version sat inside `CodeYourPCB_0.1.0_x64`,
    // where `_` is a word character, and the first version of this test found
    // nothing and passed against the very doc it was written for.
    const versions = [...doc.matchAll(/\d+\.\d+\.\d+(?:-[a-z]+)?/g)].map((m) => m[0]);
    // Node and Ubuntu versions in the troubleshooting section are not ours.
    const ours = versions.filter((v) => v.startsWith('0.'));
    for (const v of ours) {
      expect(v, `the page names ${v}, tauri.conf.json says ${tauriConfig.version}`).toBe(
        tauriConfig.version
      );
    }
  });

  it.each([
    'setup-windows.bat',
    'setup-macos.sh',
    'setup-linux.sh',
    'dev-windows.bat',
    'dev-macos.sh',
    'dev-linux.sh',
    'build-windows.bat',
    'build-macos.sh',
    'build-linux.sh',
  ])('%s is named by the page and exists', (script) => {
    expect(doc, `${script} should be in the page`).toContain(script);
    expect(existsSync(join(repo, script)), `${script} should exist`).toBe(true);
  });

  it.each([
    ['F11', 'view.fullscreen'],
    ['Ctrl+Shift+T', 'view.theme'],
  ])('the %s shortcut the page promises is the one the menu binds', (shortcut, id) => {
    expect(doc).toContain(shortcut);
    const item = menu.slice(menu.indexOf(`"${id}"`));
    expect(item.slice(0, 200)).toContain(`with_shortcut("${shortcut}")`);
  });

  it('promises only menus the menu bar has', () => {
    // The page tells the reader to expect File/Edit/View/Help.
    for (const name of ['File', 'Edit', 'View', 'Help']) {
      expect(menu, `the menu bar should have a ${name} menu`).toContain(`Menu::new("${name}")`);
    }
  });
});
