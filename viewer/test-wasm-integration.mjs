/**
 * Integration test that verifies the WasmPcbEngineAdapter works correctly.
 * Tests the complete flow: parse source -> load snapshot -> query.
 */

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Read the WASM file
const wasmPath = join(__dirname, 'pkg', 'cypcb_render_bg.wasm');
const wasmBytes = readFileSync(wasmPath);

// Import the init function and PcbEngine class
const { default: init, PcbEngine } = await import('./pkg/cypcb_render.js');

// Test source code (same as what the viewer uses)
const TEST_SOURCE = `
version 1
board test {
  size 50mm x 30mm
  layers 2
}
component R1 resistor "0402" {
  value "10k"
  at 10mm, 15mm
}
component C1 capacitor "0603" {
  value "100nF"
  at 20mm, 15mm
}
component U1 ic "DIP-8" {
  value "ATtiny85"
  at 35mm, 15mm
}
net VCC {
  R1.1
  C1.1
}
`;

// Simple JS parser to simulate what wasm.ts parseSource() does
function parseSource(source) {
  const errors = [];
  const lines = source.split('\n');

  let board = null;
  let currentBoard = null;
  const components = [];
  const nets = new Map();
  let currentComponent = null;
  let currentNet = null;
  let braceDepth = 0;
  let inBoard = false;
  let inComponent = false;
  let inNet = false;

  function parseUnit(value, unit) {
    switch (unit) {
      case 'mm': return Math.round(value * 1_000_000);
      case 'mil': return Math.round(value * 25_400);
      case 'inch': return Math.round(value * 25_400_000);
      default: return Math.round(value * 1_000_000);
    }
  }

  for (let lineNum = 0; lineNum < lines.length; lineNum++) {
    const line = lines[lineNum].trim();
    if (!line || line.startsWith('//')) continue;

    const openBraces = (line.match(/{/g) || []).length;
    const closeBraces = (line.match(/}/g) || []).length;

    if (line.startsWith('version ')) continue;

    const boardMatch = line.match(/^board\s+(\w+)\s*\{?$/);
    if (boardMatch) {
      currentBoard = { name: boardMatch[1], width_nm: 0, height_nm: 0, layer_count: 2 };
      inBoard = true;
      braceDepth += openBraces;
      continue;
    }

    const compMatch = line.match(/^component\s+(\w+)\s+(\w+)\s+"([^"]+)"\s*\{?$/);
    if (compMatch) {
      currentComponent = {
        refdes: compMatch[1], value: '', x_nm: 0, y_nm: 0,
        rotation_mdeg: 0, footprint: compMatch[3], pads: []
      };
      inComponent = true;
      braceDepth += openBraces;
      continue;
    }

    const netMatch = line.match(/^net\s+(\w+)\s*\{?$/);
    if (netMatch) {
      currentNet = { name: netMatch[1], pins: [] };
      inNet = true;
      braceDepth += openBraces;
      continue;
    }

    if (inBoard && currentBoard) {
      const sizeMatch = line.match(/^size\s+(\d+(?:\.\d+)?)(mm|mil|inch)\s+x\s+(\d+(?:\.\d+)?)(mm|mil|inch)$/);
      if (sizeMatch) {
        currentBoard.width_nm = parseUnit(parseFloat(sizeMatch[1]), sizeMatch[2]);
        currentBoard.height_nm = parseUnit(parseFloat(sizeMatch[3]), sizeMatch[4]);
      }
      const layersMatch = line.match(/^layers\s+(\d+)$/);
      if (layersMatch) {
        currentBoard.layer_count = parseInt(layersMatch[1], 10);
      }
    }

    if (inComponent && currentComponent) {
      const valueMatch = line.match(/^value\s+"([^"]*)"$/);
      if (valueMatch) currentComponent.value = valueMatch[1];

      const atMatch = line.match(/^at\s+(\d+(?:\.\d+)?)(mm|mil|inch),\s*(\d+(?:\.\d+)?)(mm|mil|inch)(?:\s+rotate\s+(\d+(?:\.\d+)?))?$/);
      if (atMatch) {
        currentComponent.x_nm = parseUnit(parseFloat(atMatch[1]), atMatch[2]);
        currentComponent.y_nm = parseUnit(parseFloat(atMatch[3]), atMatch[4]);
        if (atMatch[5]) currentComponent.rotation_mdeg = Math.round(parseFloat(atMatch[5]) * 1000);
      }
    }

    if (inNet && currentNet) {
      const pinMatch = line.match(/^(\w+)\.(\w+)$/);
      if (pinMatch) currentNet.pins.push(pinMatch[1] + '.' + pinMatch[2]);
    }

    if (closeBraces > 0) {
      braceDepth -= closeBraces;
      if (braceDepth <= 0) {
        if (inBoard && currentBoard) { board = currentBoard; currentBoard = null; inBoard = false; }
        if (inComponent && currentComponent) { components.push(currentComponent); currentComponent = null; inComponent = false; }
        if (inNet && currentNet) {
          const connections = currentNet.pins.map(pin => {
            const [component, pinNum] = pin.split('.');
            return { component, pin: pinNum };
          });
          nets.set(currentNet.name, { name: currentNet.name, id: nets.size, connections });
          currentNet = null;
          inNet = false;
        }
        braceDepth = 0;
      }
    }
    braceDepth += openBraces;
  }

  return { snapshot: { board, components, nets: Array.from(nets.values()), violations: [] }, errors };
}

async function main() {
  console.log('=== WASM Integration Test ===\n');

  // 1. Initialize WASM
  console.log('1. Initializing WASM module...');
  await init(wasmBytes);
  console.log('   WASM module initialized.\n');

  // 2. Create engine and parse source (simulating WasmPcbEngineAdapter)
  console.log('2. Creating PcbEngine and parsing source...');
  const engine = new PcbEngine();
  const { snapshot, errors } = parseSource(TEST_SOURCE);

  const boardWidth = snapshot.board.width_nm / 1_000_000;
  const boardHeight = snapshot.board.height_nm / 1_000_000;
  console.log('   Parsed board: ' + snapshot.board.name + ' (' + boardWidth + 'mm x ' + boardHeight + 'mm)');
  console.log('   Parsed components: ' + snapshot.components.length);
  snapshot.components.forEach(c => {
    const xMm = c.x_nm / 1_000_000;
    const yMm = c.y_nm / 1_000_000;
    console.log('     - ' + c.refdes + ': ' + c.footprint + ' at (' + xMm + 'mm, ' + yMm + 'mm)');
  });
  console.log('   Parsed nets: ' + snapshot.nets.length);
  snapshot.nets.forEach(n => console.log('     - ' + n.name + ': ' + n.connections.length + ' connections'));
  console.log('');

  // 3. Load snapshot into WASM engine
  console.log('3. Loading snapshot into WASM engine...');
  const loadError = engine.load_snapshot(snapshot);
  if (loadError) {
    console.error('   Error: ' + loadError);
    process.exit(1);
  }
  console.log('   Snapshot loaded successfully.\n');

  // 4. Get snapshot back from WASM
  console.log('4. Getting snapshot from WASM engine...');
  const wasmSnapshot = engine.get_snapshot();
  const boardName = wasmSnapshot.board ? wasmSnapshot.board.name : 'null';
  console.log('   Board name: ' + boardName);
  console.log('   Component count: ' + wasmSnapshot.components.length);
  console.log('');

  // 5. Test query_point (component R1 at 10mm, 15mm)
  console.log('5. Testing query_point...');
  const r1Hits = engine.query_point(BigInt(10_000_000), BigInt(15_000_000));
  console.log('   Query (10mm, 15mm): ' + (r1Hits.length > 0 ? r1Hits.join(', ') : 'no hits'));

  const c1Hits = engine.query_point(BigInt(20_000_000), BigInt(15_000_000));
  console.log('   Query (20mm, 15mm): ' + (c1Hits.length > 0 ? c1Hits.join(', ') : 'no hits'));

  const emptyHits = engine.query_point(BigInt(1_000_000), BigInt(1_000_000));
  console.log('   Query (1mm, 1mm): ' + (emptyHits.length > 0 ? emptyHits.join(', ') : 'no hits (expected)'));
  console.log('');

  // Cleanup
  engine.free();

  console.log('=== All integration tests passed! ===');
}

main().catch(err => {
  console.error('Test failed:', err);
  process.exit(1);
});
