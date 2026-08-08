import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

/**
 * The template list is written twice, and both copies have to agree.
 *
 * `index.html` holds the cards a user clicks; `project-manager.ts` holds the
 * file each id loads. A card whose id is in neither list does nothing when
 * clicked - the click handler looks the id up and returns - and an entry with
 * no card is a template nobody can reach.
 */

const VIEWER = join(__dirname, '..', '..');

function cardIds(): string[] {
  const html = readFileSync(join(VIEWER, 'index.html'), 'utf8');
  return [...html.matchAll(/data-template="([\w-]+)"/g)].map(m => m[1]);
}

function registeredIds(): string[] {
  const source = readFileSync(join(VIEWER, 'src', 'project-manager.ts'), 'utf8');
  const registry = source.slice(source.indexOf('const TEMPLATES'), source.indexOf('\n];'));
  return [...registry.matchAll(/id: '([\w-]+)'/g)].map(m => m[1]);
}

describe('every template card is a template', () => {
  it('the cards and the registry name the same templates', () => {
    expect(cardIds().sort()).toEqual(registeredIds().sort());
  });

  it('every registered template has a file to load', () => {
    const source = readFileSync(join(VIEWER, 'src', 'project-manager.ts'), 'utf8');
    const registry = source.slice(source.indexOf('const TEMPLATES'), source.indexOf('\n];'));
    const files = [...registry.matchAll(/file: '([\w.-]+)'/g)].map(m => m[1]);

    expect(files.length).toBe(registeredIds().length);
    const missing = files.filter(f => !existsSync(join(VIEWER, 'public', 'templates', f)));
    expect(missing, 'a template card that loads nothing').toEqual([]);
  });
});
