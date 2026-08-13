/**
 * Following an `import` in the browser.
 *
 * The engine resolves imports - relative paths, cycles, selective imports, a
 * library built from libraries - but it cannot read a file: a browser tab has
 * no disk. So the host fetches, and hands the engine a map of path to text.
 *
 * The paths here are the ones an `import` writes, resolved relative to the
 * file that wrote them, and normalised the same way the engine normalises so
 * the two agree on what is one file: `lib/../lib/x.cypcb` is `lib/x.cypcb`.
 */

/** Strip comments so an `import` inside one is not fetched. */
function withoutComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, ' ')
    .replace(/\/\/[^\n]*/g, ' ');
}

/**
 * Every path a file imports, in the order it names them, without repeats.
 *
 * Both forms: `import "lib/blocks.cypcb"` and
 * `import Divider, LedDriver from "lib/blocks.cypcb"`.
 */
export function importedPaths(source: string): string[] {
  const paths: string[] = [];
  const pattern = /\bimport\b[^"\n]*"([^"]+)"/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(withoutComments(source))) !== null) {
    if (!paths.includes(match[1])) paths.push(match[1]);
  }
  return paths;
}

/**
 * Flatten `.` and `..` in a path, without touching any filesystem.
 *
 * Mirrors `normalise` in `crates/cypcb-parser/src/imports.rs`: the host and the
 * engine have to agree on which key a file is stored under, or the engine asks
 * for `lib/x.cypcb` while the host supplies `./lib/x.cypcb` and neither says
 * anything useful.
 */
export function normalisePath(path: string): string {
  const out: string[] = [];
  for (const part of path.split('/')) {
    if (part === '' || part === '.') continue;
    if (part === '..') {
      if (out.length > 0 && out[out.length - 1] !== '..') {
        out.pop();
      } else {
        out.push('..');
      }
      continue;
    }
    out.push(part);
  }
  return out.join('/');
}

/** The path `written` names, seen from the file at `from`. */
export function resolveAgainst(from: string, written: string): string {
  const directory = from.includes('/') ? from.slice(0, from.lastIndexOf('/')) : '';
  return normalisePath(directory ? `${directory}/${written}` : written);
}

/**
 * Fetch everything a design imports, and everything those files import.
 *
 * `read` answers with the text at a path, or null when it cannot - a design
 * the user opened through the file picker has no directory to fetch from, and
 * a null there leaves the engine to report the import it could not follow.
 *
 * A file that fails to arrive is left out rather than retried: the engine says
 * which path it wanted and what the host did supply, which is a better message
 * than anything this function could invent.
 */
export async function collectImportedFiles(
  source: string,
  read: (path: string) => Promise<string | null>,
): Promise<Record<string, string>> {
  const files: Record<string, string> = {};
  // Paths still to fetch, each with the file it was written in.
  const queue: Array<{ from: string; written: string }> = importedPaths(source).map(written => ({
    from: '',
    written,
  }));

  while (queue.length > 0) {
    const { from, written } = queue.shift()!;
    const path = resolveAgainst(from, written);
    if (path in files) continue;

    const text = await read(path);
    if (text === null) continue;

    files[path] = text;
    for (const nested of importedPaths(text)) {
      queue.push({ from: path, written: nested });
    }
  }

  return files;
}

/** Reads files served beside the design, over HTTP. */
export function readerForBaseUrl(baseUrl: string): (path: string) => Promise<string | null> {
  return async (path: string) => {
    try {
      const response = await fetch(`${baseUrl}${path}`);
      if (!response.ok) {
        console.warn(`[imports] ${baseUrl}${path} answered ${response.status}`);
        return null;
      }
      return await response.text();
    } catch (error) {
      // Null reaches the caller either way and the design reports the import
      // as missing. What was lost is why - a 404 and a dead network read the
      // same to a user staring at a board with no library.
      console.warn(`[imports] ${baseUrl}${path} could not be fetched:`, error);
      return null;
    }
  };
}
