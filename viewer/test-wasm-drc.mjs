/**
 * DRC test - verifies clearance violations are detected in WASM mode.
 *
 * This test creates two 0402 components 0.5mm apart, which should trigger
 * a clearance violation since 0402 courtyards are 1.5mm wide.
 */

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const wasmPath = join(__dirname, 'pkg', 'cypcb_render_bg.wasm');
const wasmBytes = readFileSync(wasmPath);

const { default: init, PcbEngine } = await import('./pkg/cypcb_render.js');

async function main() {
  console.log('=== WASM DRC Test ===\n');

  // Initialize WASM
  await init(wasmBytes);
  const engine = new PcbEngine();

  // Two 0402 resistors placed 0.5mm apart
  // 0402 courtyard is 1.5mm x 1.0mm centered at origin
  // R1 at 10mm, R2 at 10.5mm means they overlap significantly
  const snapshot = {
    board: {
      name: 'drc_test',
      width_nm: 30_000_000,  // 30mm
      height_nm: 30_000_000, // 30mm
      layer_count: 2
    },
    components: [
      {
        refdes: 'R1',
        value: '10k',
        x_nm: 10_000_000,   // 10mm
        y_nm: 15_000_000,   // 15mm
        rotation_mdeg: 0,
        footprint: '0402',
        pads: []
      },
      {
        refdes: 'R2',
        value: '10k',
        x_nm: 10_500_000,   // 10.5mm (0.5mm from R1)
        y_nm: 15_000_000,   // 15mm
        rotation_mdeg: 0,
        footprint: '0402',
        pads: []
      }
    ],
    nets: [],
    violations: []
  };

  console.log('Loading snapshot with two 0402 components 0.5mm apart...');
  const error = engine.load_snapshot(snapshot);
  if (error) {
    console.error('Error:', error);
    process.exit(1);
  }

  const result = engine.get_snapshot();
  console.log('\nDRC Results:');
  console.log('  Violation count:', result.violations.length);

  if (result.violations.length === 0) {
    console.error('\nERROR: Expected clearance violations but found none!');
    process.exit(1);
  }

  for (let i = 0; i < result.violations.length; i++) {
    const v = result.violations[i];
    console.log('  ' + (i + 1) + '. ' + v.kind + ': ' + v.message);
  }

  // Check for clearance violation
  const hasClearance = result.violations.some(v => v.kind === 'clearance');
  if (!hasClearance) {
    console.error('\nERROR: Expected clearance violation but found:', result.violations.map(v => v.kind));
    process.exit(1);
  }

  console.log('\n=== DRC test passed! Clearance violation detected correctly. ===');
  engine.free();
}

main().catch(err => {
  console.error('Test failed:', err);
  process.exit(1);
});
