import { describe, it, expect } from 'vitest';
import { loadWasm, register3DModel } from '../wasm';

/**
 * A 3D model fetched for a part has to reach the engine.
 *
 * `register3DModel` wrote a module-level `Map` and stopped: the only reader of
 * that map had no callers, the engine's own `register_3d_model` was called by
 * nothing, and no component ever came back carrying a `model_3d` - so the 3D
 * view's auto-load pass, the loader it calls and the engine's map were three
 * links of a chain nothing completed.
 *
 * The other half of the fetch, `registerDynamicFootprint`, had exactly this
 * defect and exactly this fix, replay included, in `dynamic-footprint.test.ts`.
 */
describe('3D model registration', () => {
  it('reaches the engine, including models registered before it existed', async () => {
    // Registered first, with no engine to hand it to.
    register3DModel('TEST_MODEL_EARLY', 'uuid-early');

    const engine = await loadWasm();
    const seen = (engine as unknown as { registeredModels?: Map<string, string> })
      .registeredModels;
    expect(seen, 'the engine records the models it was taught').toBeDefined();
    expect(
      seen?.get('TEST_MODEL_EARLY'),
      'a model fetched before load is replayed',
    ).toBe('uuid-early');

    // And one registered afterwards goes straight through.
    register3DModel('TEST_MODEL_LATE', 'uuid-late');
    expect(seen?.get('TEST_MODEL_LATE'), 'a later fetch reaches the engine too').toBe(
      'uuid-late',
    );
  });
});
