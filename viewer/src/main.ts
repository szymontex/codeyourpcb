/**
 * Main entry point for the CodeYourPCB viewer application
 * Integrates WASM engine, rendering, and user interaction
 */

import { loadWasm, isWasmLoaded } from './wasm';
import type { BoardSnapshot } from './types';
import { createViewport, fitBoard, screenToWorld } from './viewport';
import { render, type RenderState } from './renderer';
import { setupInteraction, type InteractionState } from './interaction';
import { createLayerVisibility } from './layers';

// Test source for initial verification
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
component U1 ic "DIP-8" {
  value "ATtiny85"
  at 35mm, 15mm
}
`;

/**
 * Initialize the PCB viewer application
 */
async function init(): Promise<void> {
  const status = document.getElementById('status')!;
  const canvas = document.getElementById('pcb-canvas') as HTMLCanvasElement;
  const container = document.getElementById('canvas-container')!;
  const coordsEl = document.getElementById('coords')!;
  const topLayerCb = document.getElementById('layer-top') as HTMLInputElement;
  const bottomLayerCb = document.getElementById('layer-bottom') as HTMLInputElement;

  const ctx = canvas.getContext('2d')!;

  // State
  let snapshot: BoardSnapshot | null = null;
  let viewport = createViewport(canvas.width, canvas.height);
  let layers = createLayerVisibility();
  let selectedRefdes: string | null = null;
  let dirty = true;

  // Resize handler
  function resize(): void {
    canvas.width = container.clientWidth;
    canvas.height = container.clientHeight;
    viewport = {
      ...viewport,
      width: canvas.width,
      height: canvas.height,
    };
    dirty = true;
  }
  resize();
  window.addEventListener('resize', resize);

  // Layer checkbox handlers
  topLayerCb.addEventListener('change', () => {
    layers = {
      ...layers,
      topCopper: topLayerCb.checked,
    };
    dirty = true;
  });
  bottomLayerCb.addEventListener('change', () => {
    layers = {
      ...layers,
      bottomCopper: bottomLayerCb.checked,
    };
    dirty = true;
  });

  // Load WASM
  status.textContent = 'Loading WASM...';
  let engine;
  try {
    engine = await loadWasm();
  } catch (err) {
    console.error('WASM load failed:', err);
    status.textContent = `WASM Error: ${err}`;
    return;
  }

  const usingWasm = isWasmLoaded();
  status.textContent = usingWasm
    ? 'WASM loaded, parsing...'
    : 'Mock engine loaded, parsing...';

  // Load test source
  const errors = engine.load_source(TEST_SOURCE);
  if (errors) {
    console.warn('Parse errors:', errors);
  }

  snapshot = engine.get_snapshot();
  console.log('Loaded snapshot:', snapshot);

  // Fit board in view
  if (snapshot.board) {
    viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
  }

  status.textContent = errors
    ? `Warnings: ${errors.split('\n').filter(Boolean).length}`
    : usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';

  // Interaction setup
  const interactionState: InteractionState = {
    viewport,
    isPanning: false,
    lastX: 0,
    lastY: 0,
    dirty: false,
    onSelect: (x_nm, y_nm) => {
      // Query engine for component at point
      const hits = engine.query_point(Math.round(x_nm), Math.round(y_nm));
      if (hits && hits.length > 0) {
        selectedRefdes = hits[0];
        console.log('Selected:', selectedRefdes);
        // Show selected in status
        const comp = snapshot?.components.find(c => c.refdes === selectedRefdes);
        if (comp) {
          status.textContent = `Selected: ${comp.refdes} (${comp.value})`;
        }
      } else {
        selectedRefdes = null;
        status.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }
      dirty = true;
    },
    onViewportChange: (vp) => {
      viewport = vp;
      interactionState.viewport = vp;
    },
  };

  setupInteraction(canvas, interactionState);

  // Coordinate display on mouse move
  canvas.addEventListener('mousemove', (e) => {
    const rect = canvas.getBoundingClientRect();
    const [worldX, worldY] = screenToWorld(
      viewport,
      e.clientX - rect.left,
      e.clientY - rect.top
    );
    // Convert to mm for display
    const xMm = (worldX / 1_000_000).toFixed(2);
    const yMm = (worldY / 1_000_000).toFixed(2);
    coordsEl.textContent = `(${xMm}, ${yMm}) mm`;
  });

  canvas.addEventListener('mouseleave', () => {
    coordsEl.textContent = '';
  });

  // Render loop
  function frame(): void {
    if (dirty || interactionState.dirty) {
      const renderState: RenderState = {
        snapshot,
        viewport,
        layers,
        selectedRefdes,
      };
      render(ctx, renderState);
      dirty = false;
      interactionState.dirty = false;
    }
    requestAnimationFrame(frame);
  }
  frame();
}

// Start the application
init().catch((error) => {
  console.error('Failed to initialize viewer:', error);
  const status = document.getElementById('status');
  if (status) {
    status.textContent = 'Error: ' + (error instanceof Error ? error.message : String(error));
  }
});
