/**
 * The autorouter, off the main thread.
 *
 * The worker owns its own `PcbEngine`: it reads the design text it is given,
 * routes it, and sends back the engine's JSON answer together with the routed
 * copper as DSL. Nothing is shared - a `PcbEngine` lives in wasm memory that
 * cannot cross a `postMessage` - so the main thread applies the copper to its
 * own engine through the same merge the save path uses.
 *
 * A fresh engine per request, freed afterwards. The alternative is a worker
 * that keeps one engine and reloads it, which saves a few milliseconds of
 * parsing and buys a class of bug where the second run routes a board that
 * still carries the first run's copper.
 */

import { isWorkerRequest, type WorkerResponse } from './worker-protocol';

/**
 * The worker's own global, described structurally.
 *
 * `DedicatedWorkerGlobalScope` comes from TypeScript's WebWorker library, and
 * this project compiles the viewer and this file in one program with the DOM
 * library, which the two cannot share. What is needed here is two members.
 */
interface WorkerScope {
  postMessage(message: unknown): void;
  onmessage: ((event: MessageEvent<unknown>) => void | Promise<void>) | null;
}

const ctx = self as unknown as WorkerScope;

function post(message: WorkerResponse): void {
  ctx.postMessage(message);
}

interface RoutingEngine {
  load_source(source: string): string;
  auto_route_with_params(params: string): string;
  auto_route_debug(params: string): string;
  export_traces_as_dsl(): string;
  free(): void;
}

/**
 * The wasm module, loaded once per worker.
 *
 * `import(...)` rather than a static import so a failure to load - no wasm
 * built, a browser without it - is an answer to the request rather than an
 * exception while the worker is starting, which the main thread would only see
 * as a silent `error` event.
 */
let modulePromise: Promise<{ PcbEngine: new () => RoutingEngine }> | null = null;

async function wasm(): Promise<{ PcbEngine: new () => RoutingEngine }> {
  if (!modulePromise) {
    modulePromise = (async () => {
      const module = await import('../pkg/cypcb_render.js');
      await module.default();
      return module as unknown as { PcbEngine: new () => RoutingEngine };
    })();
  }
  return modulePromise;
}

ctx.onmessage = async (event: MessageEvent<unknown>): Promise<void> => {
  if (!isWorkerRequest(event.data)) {
    post({ type: 'failed', error: 'the worker was sent a message it does not understand' });
    return;
  }

  const request = event.data;
  let engine: RoutingEngine | null = null;
  try {
    const { PcbEngine } = await wasm();
    engine = new PcbEngine();
    const loaded = engine.load_source(request.source);
    const loadError = readError(loaded);
    if (loadError) {
      post({ type: 'failed', error: loadError });
      return;
    }

    if (request.type === 'route-debug') {
      post({ type: 'debugged', result: engine.auto_route_debug(request.params) });
      return;
    }

    const result = engine.auto_route_with_params(request.params);
    const traces = engine.export_traces_as_dsl();
    post({ type: 'routed', result, traces });
  } catch (error) {
    post({ type: 'failed', error: `${error}` });
  } finally {
    engine?.free();
  }
};

/** `load_source` answers with JSON that may carry an error the load survived. */
function readError(answer: string): string | null {
  try {
    const parsed = JSON.parse(answer) as { ok?: unknown; error?: unknown };
    if (parsed.ok === false && typeof parsed.error === 'string') {
      return parsed.error;
    }
  } catch {
    // Not JSON: older builds answered with a bare string, which is not an error.
  }
  return null;
}

post({ type: 'ready' });
