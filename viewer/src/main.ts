/**
 * Main entry point for the CodeYourPCB viewer application
 */

import { loadWasm } from './wasm';

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

  status.textContent = 'Initializing...';

  // Attempt to load WASM module
  const engine = await loadWasm();

  if (!engine) {
    status.textContent = 'WASM not ready - waiting for build';

    // Draw placeholder on canvas
    const ctx = canvas.getContext('2d');
    if (ctx) {
      ctx.fillStyle = '#e0e0e0';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.fillStyle = '#999';
      ctx.font = '16px system-ui';
      ctx.textAlign = 'center';
      ctx.fillText('PCB Canvas - WASM module not loaded', canvas.width / 2, canvas.height / 2);
    }
    return;
  }

  status.textContent = 'Ready';
}

// Start the application
init().catch((error) => {
  console.error('Failed to initialize viewer:', error);
  const status = document.getElementById('status');
  if (status) {
    status.textContent = 'Error: ' + (error instanceof Error ? error.message : String(error));
  }
});
