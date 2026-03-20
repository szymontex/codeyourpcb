/* tslint:disable */
/* eslint-disable */

/**
 * PCB Engine - main interface for JavaScript.
 *
 * Maintains the board state and provides methods for loading source,
 * getting snapshots, and querying the board.
 */
export class PcbEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add a trace to the board.
     *
     * Creates a Manual Trace entity with the given parameters,
     * updates the spatial index, and returns the entity index.
     *
     * `segments_json` is a JSON array of `[x1, y1, x2, y2, ...]` coordinate
     * pairs in nanometers (flat array, 4 values per segment).
     *
     * Returns the entity index (u32) on success, or u32::MAX on error.
     */
    add_trace(net_name: string, layer_str: string, width_nm: bigint, segments_flat: BigInt64Array): number;
    /**
     * Add a trace from a JSON segments string (WASM-friendly).
     *
     * `segments_json` is a JSON array of flat coordinates:
     * `[x1, y1, x2, y2, x3, y3, x4, y4, ...]` (4 values per segment).
     *
     * Returns the entity index (u32) on success, or u32::MAX on error.
     */
    add_trace_json(net_name: string, layer_str: string, width_nm: bigint, segments_json: string): number;
    /**
     * Run the built-in A* autorouter on the current board.
     *
     * Clears existing autorouted traces, routes all unrouted nets,
     * applies the results, and rebuilds the spatial index.
     *
     * Returns a JSON status string: `{"ok":true,"routed":N,"unrouted":N}` on success,
     * or `{"ok":false,"error":"..."}` on failure.
     */
    auto_route(): string;
    /**
     * Generate multiple routing variants with different strategies/configs,
     * rank them by composite score, and auto-apply the best.
     *
     * Returns a JSON array of variant results:
     * `[{ "name": "...", "score": { ... }, "routes": [...], "vias": [...] }]`
     *
     * The best variant (lowest composite score) is auto-applied to the world.
     * On error, returns `{"ok":false,"error":"..."}`.
     */
    auto_route_variants(): string;
    /**
     * Run the autorouter with user-specified tuning parameters.
     *
     * `params_json` is a JSON string with fields:
     * - `via_cost`: f64 (0.1–10.0, default 1.0) — higher = fewer vias
     * - `layer_preference`: f64 (-1.0–1.0, default 0.0) — layer bias
     * - `roundness`: f64 (0.0–1.0, default 0.5) — chamfer aggressiveness
     * - `density`: f64 (0.5–2.0, default 1.0) — grid density multiplier
     *
     * Missing fields use defaults. Values are clamped to valid ranges.
     *
     * Returns a JSON status string: `{"ok":true,"routed":N,"unrouted":N}` on success,
     * or `{"ok":false,"error":"..."}` on failure.
     */
    auto_route_with_params(params_json: string): string;
    /**
     * Get a snapshot of the current board state for rendering (WASM version).
     *
     * Returns a JsValue that can be used directly in JavaScript.
     */
    get_snapshot(): any;
    /**
     * Query for a trace entity at a given point with tolerance.
     *
     * Returns the trace entity index if found, or u32::MAX if not.
     * Tolerance is in nanometers — the point is expanded into a
     * query rectangle of `2*tolerance` side length.
     */
    get_trace_at_point(x_nm: bigint, y_nm: bigint, tolerance_nm: bigint): number;
    /**
     * Get DRC violations as JSON string (WASM-friendly).
     */
    get_violations_json(): string;
    /**
     * Load a pre-parsed board snapshot (WASM mode).
     *
     * This method receives a BoardSnapshot that was parsed in JavaScript
     * and populates the internal world state for queries.
     *
     * Returns an empty string on success, or an error message on failure.
     */
    load_snapshot(snapshot_js: any): string;
    /**
     * Create a new PcbEngine instance.
     */
    constructor();
    /**
     * Query components at a specific point.
     *
     * Returns reference designator strings.
     */
    query_point(x_nm: bigint, y_nm: bigint): string[];
    /**
     * Remove a trace by entity index.
     *
     * Returns `true` if the trace was found and removed, `false` otherwise.
     */
    remove_trace(trace_id: number): boolean;
    /**
     * Rotate a component by delta millidegrees.
     *
     * Finds the component by reference designator and applies the rotation.
     * Returns `true` on success, `false` if the component was not found.
     */
    rotate_component(refdes: string, delta_mdeg: number): boolean;
    /**
     * Run DRC and return violation count.
     *
     * This runs a full DRC check (incremental optimization deferred)
     * and updates the internal violation list.
     */
    run_drc_incremental(): number;
    /**
     * Set the board outline size in nanometers.
     *
     * Returns `true` on success, `false` if no board entity exists.
     */
    set_board_size(width_nm: bigint, height_nm: bigint): boolean;
    /**
     * Get the number of trace entities in the world.
     */
    trace_count(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_pcbengine_free: (a: number, b: number) => void;
    readonly pcbengine_add_trace: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: number, h: number) => number;
    readonly pcbengine_add_trace_json: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: number, h: number) => number;
    readonly pcbengine_auto_route: (a: number, b: number) => void;
    readonly pcbengine_auto_route_variants: (a: number, b: number) => void;
    readonly pcbengine_auto_route_with_params: (a: number, b: number, c: number, d: number) => void;
    readonly pcbengine_get_snapshot: (a: number) => number;
    readonly pcbengine_get_trace_at_point: (a: number, b: bigint, c: bigint, d: bigint) => number;
    readonly pcbengine_get_violations_json: (a: number, b: number) => void;
    readonly pcbengine_load_snapshot: (a: number, b: number, c: number) => void;
    readonly pcbengine_new: () => number;
    readonly pcbengine_query_point: (a: number, b: number, c: bigint, d: bigint) => void;
    readonly pcbengine_remove_trace: (a: number, b: number) => number;
    readonly pcbengine_rotate_component: (a: number, b: number, c: number, d: number) => number;
    readonly pcbengine_run_drc_incremental: (a: number) => number;
    readonly pcbengine_set_board_size: (a: number, b: bigint, c: bigint) => number;
    readonly pcbengine_trace_count: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
