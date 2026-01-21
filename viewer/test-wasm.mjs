/**
 * WASM smoke test - verifies the WASM module can be loaded and executed.
 *
 * Run with: node test-wasm.mjs
 */

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

// Get the directory of the current module
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Read the WASM file directly
const wasmPath = join(__dirname, 'pkg', 'cypcb_render_bg.wasm');
const wasmBytes = readFileSync(wasmPath);

// Import the init function and PcbEngine class
const { default: init, PcbEngine } = await import('./pkg/cypcb_render.js');

async function main() {
    console.log('Initializing WASM module...');
    // Pass the WASM bytes directly to init
    await init(wasmBytes);
    console.log('WASM module initialized.');

    console.log('Creating PcbEngine instance...');
    const engine = new PcbEngine();
    console.log('PcbEngine instance created.');

    // Test with a pre-parsed board snapshot
    const snapshot = {
        board: {
            name: 'test',
            width_nm: 100000000,  // 100mm
            height_nm: 100000000, // 100mm
            layer_count: 2
        },
        components: [
            {
                refdes: 'R1',
                value: '10k',
                x_nm: 10000000,  // 10mm
                y_nm: 10000000,  // 10mm
                rotation_mdeg: 0,
                footprint: '0402',
                pads: []
            }
        ],
        nets: []
    };

    console.log('Loading snapshot...');
    const error = engine.load_snapshot(snapshot);
    if (error) {
        console.error('Error loading snapshot:', error);
        process.exit(1);
    }
    console.log('Snapshot loaded successfully.');

    console.log('Getting snapshot back...');
    const result = engine.get_snapshot();
    console.log('Snapshot result:', JSON.stringify(result, null, 2));

    // Verify the snapshot has expected structure
    if (!result.board) {
        console.error('Error: No board in snapshot');
        process.exit(1);
    }
    if (result.board.name !== 'test') {
        console.error('Error: Board name mismatch');
        process.exit(1);
    }
    if (result.components.length !== 1) {
        console.error('Error: Expected 1 component, got', result.components.length);
        process.exit(1);
    }
    if (result.components[0].refdes !== 'R1') {
        console.error('Error: Component refdes mismatch');
        process.exit(1);
    }

    console.log('Querying point (10mm, 10mm)...');
    const components = engine.query_point(BigInt(10000000), BigInt(10000000));
    console.log('Components at point:', components);

    // Clean up
    engine.free();
    console.log('Engine freed.');

    console.log('');
    console.log('=== WASM test passed! ===');
}

main().catch(err => {
    console.error('Test failed:', err);
    process.exit(1);
});
