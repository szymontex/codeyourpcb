/**
 * What the main thread and the routing worker say to each other.
 *
 * Routing is the one thing this viewer does that takes seconds rather than
 * milliseconds, and it ran on the main thread until 2026-08-27: the browser
 * froze for the whole run, the overlay that says "routing" could not paint,
 * and the cancel button could not be clicked because nothing could be clicked.
 * R201 to R203 in `.gsd/REQUIREMENTS.md` are that report written down.
 *
 * The protocol is deliberately small. The worker gets the design as text and
 * the tuning parameters as the JSON string the engine already takes, and it
 * answers with what the engine returned plus the routed copper as DSL - the
 * same text `export_traces_as_dsl` produces for a save, so the main thread
 * applies it through the merge path that already exists.
 */

/** Route this design with these parameters. */
export interface RouteRequest {
  type: 'route';
  /** The `.cypcb` source, imports already resolved by the main thread. */
  source: string;
  /** `{"via_cost":N,"layer_preference":N,"roundness":N,"density":N}` */
  params: string;
}

export type WorkerRequest = RouteRequest;

/** The worker has its engine and will answer the next request. */
export interface ReadyResponse {
  type: 'ready';
}

/** The engine ran. `result` is its JSON, `traces` is the routed copper. */
export interface RoutedResponse {
  type: 'routed';
  result: string;
  traces: string;
}

/** The engine threw, or the worker could not start one. */
export interface FailedResponse {
  type: 'failed';
  error: string;
}

export type WorkerResponse = ReadyResponse | RoutedResponse | FailedResponse;

/**
 * Is this a message this protocol describes?
 *
 * A worker can receive anything: an extension, another library's ping, a
 * message from a page that shares the origin. Reading `event.data.type`
 * without asking is how a viewer ends up rendering someone else's object.
 */
export function isWorkerResponse(value: unknown): value is WorkerResponse {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const message = value as { type?: unknown; result?: unknown; traces?: unknown; error?: unknown };
  switch (message.type) {
    case 'ready':
      return true;
    case 'routed':
      return typeof message.result === 'string' && typeof message.traces === 'string';
    case 'failed':
      return typeof message.error === 'string';
    default:
      return false;
  }
}

/** The other direction, for the worker's own reading. */
export function isWorkerRequest(value: unknown): value is WorkerRequest {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const message = value as { type?: unknown; source?: unknown; params?: unknown };
  return (
    message.type === 'route' &&
    typeof message.source === 'string' &&
    typeof message.params === 'string'
  );
}
