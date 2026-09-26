/**
 * The autorouter, off the main thread.
 *
 * The worker owns its own `PcbEngine`: it loads the design it is given the way
 * the page loaded it, routes it, and sends back the engine's JSON answer together with the routed
 * copper as DSL. Nothing is shared - a `PcbEngine` lives in wasm memory that
 * cannot cross a `postMessage` - so the main thread applies the copper to its
 * own engine through the same merge the save path uses.
 *
 * A fresh engine per request, freed afterwards. The alternative is a worker
 * that keeps one engine and reloads it, which saves a few milliseconds of
 * parsing and buys a class of bug where the second run routes a board that
 * still carries the first run's copper.
 */

import { loadDesignInto, type DesignLoader } from './design-load';
import type { PadInfo, SilkShape } from './types';
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
  register_footprint(name: string, pads: PadInfo[], silk: SilkShape[]): string;
  load_source_with_imports(source: string, files_json: string): string;
  load_kicad(source: string): string;
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
    const loadError = loadDesignInto(loader(engine), request.design);
    if (loadError) {
      // Routing what did load would answer about a smaller board than the
      // one on screen, and say nothing about the difference.
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

/** The raw engine takes the imported files as JSON text. */
function loader(engine: RoutingEngine): DesignLoader {
  return {
    register_footprint: (name, pads, silk) => engine.register_footprint(name, pads, silk),
    load_source_with_imports: (source, files) =>
      engine.load_source_with_imports(source, JSON.stringify(files)),
    load_kicad: (source) => engine.load_kicad(source),
  };
}

post({ type: 'ready' });
