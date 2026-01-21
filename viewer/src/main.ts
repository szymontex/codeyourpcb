/**
 * Main entry point for the CodeYourPCB viewer application
 */

import { loadWasm, loadAndSnapshot, isWasmLoaded } from './wasm';
import type { BoardSnapshot } from './types';

// Test source for verification
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
component C1 capacitor "0402" {
  value "100nF"
  at 20mm, 15mm
}
net VCC {
  R1.1
  C1.1
}
`;

/**
 * Initialize the PCB viewer application
 */
async function init(): Promise<void> {
  const status = document.getElementById('status')!;
  const canvas = document.getElementById('pcb-canvas') as HTMLCanvasElement;
  const container = document.getElementById('canvas-container')!;
  const coordsDisplay = document.getElementById('coords')!;

  // Resize canvas to match container
  function resize(): void {
    canvas.width = container.clientWidth;
    canvas.height = container.clientHeight;
    // Re-render on resize if we have a renderer
  }

  resize();
  window.addEventListener('resize', resize);

  // Track mouse position for coordinate display
  canvas.addEventListener('mousemove', (e) => {
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    coordsDisplay.textContent = `X: ${x.toFixed(0)} Y: ${y.toFixed(0)}`;
  });

  canvas.addEventListener('mouseleave', () => {
    coordsDisplay.textContent = '';
  });

  status.textContent = 'Loading WASM...';

  try {
    await loadWasm();

    const usingWasm = isWasmLoaded();
    status.textContent = usingWasm
      ? 'WASM loaded, parsing test source...'
      : 'Mock engine loaded, parsing test source...';

    const result = loadAndSnapshot(TEST_SOURCE);
    if (!result) {
      status.textContent = 'Failed to load source';
      return;
    }

    if (result.errors) {
      console.warn('Parse/sync errors:', result.errors);
      status.textContent = `Errors: ${result.errors.split('\n')[0]}`;
    } else {
      status.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
    }

    // Log snapshot for verification
    console.log('=== BoardSnapshot ===');
    console.log('Board:', result.snapshot.board);
    console.log('Components:', result.snapshot.components.length);
    console.log('Nets:', result.snapshot.nets.length);

    // Verify data structure
    if (result.snapshot.board) {
      console.log(`Board: ${result.snapshot.board.name} ${result.snapshot.board.width_nm}nm x ${result.snapshot.board.height_nm}nm`);
      console.log(`  Expected: test 50000000nm x 30000000nm`);
    }

    for (const comp of result.snapshot.components) {
      console.log(`  ${comp.refdes}: ${comp.value} at (${comp.x_nm}, ${comp.y_nm}), ${comp.pads.length} pads`);
    }

    for (const net of result.snapshot.nets) {
      console.log(`  Net ${net.name}: ${net.connections.map(c => `${c.component}.${c.pin}`).join(', ')}`);
    }

    // Draw basic visualization on canvas
    drawBoard(canvas, result.snapshot);

  } catch (err) {
    console.error('WASM init failed:', err);
    status.textContent = `Error: ${err}`;
  }
}

/**
 * Draw a basic board visualization on the canvas
 */
function drawBoard(canvas: HTMLCanvasElement, snapshot: BoardSnapshot): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  // Clear canvas
  ctx.fillStyle = '#1a1a1a';
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  if (!snapshot.board) {
    ctx.fillStyle = '#666';
    ctx.font = '16px system-ui';
    ctx.textAlign = 'center';
    ctx.fillText('No board defined', canvas.width / 2, canvas.height / 2);
    return;
  }

  // Calculate scale to fit board in canvas
  const boardWidth = snapshot.board.width_nm / 1_000_000; // Convert to mm
  const boardHeight = snapshot.board.height_nm / 1_000_000;

  const margin = 40;
  const availableWidth = canvas.width - 2 * margin;
  const availableHeight = canvas.height - 2 * margin;

  const scale = Math.min(
    availableWidth / boardWidth,
    availableHeight / boardHeight
  );

  const offsetX = (canvas.width - boardWidth * scale) / 2;
  const offsetY = (canvas.height - boardHeight * scale) / 2;

  // Draw board outline
  ctx.strokeStyle = '#444';
  ctx.lineWidth = 2;
  ctx.strokeRect(offsetX, offsetY, boardWidth * scale, boardHeight * scale);

  // Draw board fill
  ctx.fillStyle = '#0a3d0a';
  ctx.fillRect(offsetX, offsetY, boardWidth * scale, boardHeight * scale);

  // Draw components
  for (const comp of snapshot.components) {
    const x = offsetX + (comp.x_nm / 1_000_000) * scale;
    // Y is inverted (screen Y goes down, board Y goes up)
    const y = offsetY + boardHeight * scale - (comp.y_nm / 1_000_000) * scale;

    // Draw component body
    const compWidth = 2 * scale; // 2mm
    const compHeight = 1 * scale; // 1mm

    ctx.fillStyle = '#333';
    ctx.fillRect(x - compWidth / 2, y - compHeight / 2, compWidth, compHeight);

    // Draw pads
    for (const pad of comp.pads) {
      const padX = x + (pad.x_nm / 1_000_000) * scale;
      const padY = y - (pad.y_nm / 1_000_000) * scale;
      const padWidth = (pad.width_nm / 1_000_000) * scale;
      const padHeight = (pad.height_nm / 1_000_000) * scale;

      ctx.fillStyle = '#c4a000';
      ctx.fillRect(padX - padWidth / 2, padY - padHeight / 2, padWidth, padHeight);
    }

    // Draw refdes label
    ctx.fillStyle = '#fff';
    ctx.font = '10px system-ui';
    ctx.textAlign = 'center';
    ctx.fillText(comp.refdes, x, y + compHeight / 2 + 12);
  }

  // Draw board info
  ctx.fillStyle = '#888';
  ctx.font = '12px system-ui';
  ctx.textAlign = 'left';
  ctx.fillText(`${snapshot.board.name} (${boardWidth}mm x ${boardHeight}mm, ${snapshot.board.layer_count} layers)`, 10, 20);
  ctx.fillText(`Components: ${snapshot.components.length}, Nets: ${snapshot.nets.length}`, 10, 36);
}

// Start the application
init().catch((error) => {
  console.error('Failed to initialize viewer:', error);
  const status = document.getElementById('status');
  if (status) {
    status.textContent = 'Error: ' + (error instanceof Error ? error.message : String(error));
  }
});
