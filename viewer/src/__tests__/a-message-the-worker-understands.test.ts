import { describe, it, expect } from 'vitest';
import { isWorkerRequest, isWorkerResponse } from '../worker-protocol';
import type { Design } from '../design-load';

const DESIGN: Design = { kind: 'cypcb', source: 'board b {}', imports: {}, footprints: [] };

/**
 * The routing worker's protocol, held to what it claims to recognise.
 *
 * A worker receives whatever the page sends it, and a `postMessage` from an
 * extension or another library arrives at the same handler as a routing
 * request. Both guards exist so neither side reads `data.type` from an object
 * that has nothing else the code then uses.
 */
describe('the routing protocol', () => {
  it('accepts the three answers a worker gives', () => {
    expect(isWorkerResponse({ type: 'ready' })).toBe(true);
    expect(isWorkerResponse({ type: 'routed', result: '{"ok":true}', traces: 'trace SIG {}' })).toBe(true);
    expect(isWorkerResponse({ type: 'failed', error: 'the engine threw' })).toBe(true);
  });

  it('refuses an answer that is the right shape and the wrong contents', () => {
    // The type is right and the payload is not, which is the case a bare
    // `data.type` check waves through and the reader then trips over.
    expect(isWorkerResponse({ type: 'routed', result: 12, traces: null })).toBe(false);
    expect(isWorkerResponse({ type: 'failed' })).toBe(false);
    expect(isWorkerResponse({ type: 'something-else' })).toBe(false);
    expect(isWorkerResponse(null)).toBe(false);
    expect(isWorkerResponse('routed')).toBe(false);
  });

  it('accepts a debug report and a debug request', () => {
    // The debug run is the heaviest call the engine has, and it answers with a
    // stage report rather than copper - a shape of its own, so the handler that
    // draws traces cannot be handed one by mistake.
    expect(isWorkerResponse({ type: 'debugged', result: '{"ok":true,"stages":[]}' })).toBe(true);
    expect(isWorkerResponse({ type: 'debugged' })).toBe(false);
    expect(isWorkerRequest({ type: 'route-debug', design: DESIGN, params: '{}' })).toBe(true);
    expect(isWorkerRequest({ type: 'route-debug', design: DESIGN })).toBe(false);
  });

  it('accepts a routing request and refuses a half-written one', () => {
    expect(isWorkerRequest({ type: 'route', design: DESIGN, params: '{}' })).toBe(true);
    expect(isWorkerRequest({ type: 'route', design: DESIGN })).toBe(false);
    expect(isWorkerRequest({ type: 'route', design: { ...DESIGN, source: 42 }, params: '{}' })).toBe(false);
    // The design as bare text, which is what left the worker without the
    // footprints and files the page had.
    expect(isWorkerRequest({ type: 'route', source: 'board b {}', params: '{}' })).toBe(false);
    expect(isWorkerRequest({ type: 'route', design: { ...DESIGN, footprints: undefined }, params: '{}' })).toBe(false);
    expect(isWorkerRequest({ type: 'ready' })).toBe(false);
  });
});
