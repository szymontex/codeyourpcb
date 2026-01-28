/**
 * Mouse interaction handlers for PCB viewer
 * Provides zoom, pan, and selection behaviors
 */

import type { Viewport } from './viewport';
import { zoomAtPoint, pan, screenToWorld } from './viewport';

export interface InteractionState {
  viewport: Viewport;
  isPanning: boolean;
  lastX: number;
  lastY: number;
  dirty: boolean;
  onSelect: (x_nm: number, y_nm: number) => void;
  onViewportChange: (vp: Viewport) => void;
}

/**
 * Set up all interaction handlers for the canvas
 * - Scroll wheel: zoom centered on cursor (also pinch-to-zoom on touchpad)
 * - Two-finger touchpad/touchscreen drag: pan
 * - Middle-click + drag: pan
 * - Ctrl + left-click + drag: pan (alternative for laptops)
 * - Left-click: select component at point
 * - Right-click: reserved (context menu prevented)
 */
export function setupInteraction(
  canvas: HTMLCanvasElement,
  state: InteractionState
): void {
  // Pointer cache for multi-touch pan detection
  const pointerCache: Array<{ pointerId: number; clientX: number; clientY: number }> = [];

  // Wheel zoom (zoom to cursor position)
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    // Zoom in on scroll up, out on scroll down
    const factor = e.deltaY < 0 ? 1.15 : 0.87;
    state.viewport = zoomAtPoint(state.viewport, x, y, factor);
    state.dirty = true;
    state.onViewportChange(state.viewport);
  }, { passive: false });

  // Pointer Events for two-finger touchpad/touchscreen pan
  canvas.addEventListener('pointerdown', (e) => {
    pointerCache.push({
      pointerId: e.pointerId,
      clientX: e.clientX,
      clientY: e.clientY,
    });
  });

  canvas.addEventListener('pointermove', (e) => {
    // Find this pointer in cache
    const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
    if (index === -1) return;

    // If exactly 2 pointers, perform two-finger pan
    if (pointerCache.length === 2) {
      const cached = pointerCache[index];
      const dx = e.clientX - cached.clientX;
      const dy = e.clientY - cached.clientY;

      // Half delta since both fingers contribute to pan
      state.viewport = pan(state.viewport, dx / 2, dy / 2);
      state.dirty = true;
      state.onViewportChange(state.viewport);
    }

    // Update cached position
    pointerCache[index].clientX = e.clientX;
    pointerCache[index].clientY = e.clientY;
  });

  // Shared cleanup function for pointer removal
  const removePointer = (e: PointerEvent) => {
    const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
    if (index !== -1) {
      pointerCache.splice(index, 1);
    }
  };

  canvas.addEventListener('pointerup', removePointer);
  canvas.addEventListener('pointercancel', removePointer);
  canvas.addEventListener('pointerout', removePointer);
  canvas.addEventListener('pointerleave', removePointer);

  // Middle-click pan OR Ctrl+left-click pan (for laptops without middle button)
  canvas.addEventListener('mousedown', (e) => {
    if (e.button === 1 || (e.button === 0 && e.ctrlKey)) {
      state.isPanning = true;
      state.lastX = e.clientX;
      state.lastY = e.clientY;
      e.preventDefault();
      canvas.style.cursor = 'grabbing';
    }
  });

  canvas.addEventListener('mousemove', (e) => {
    if (state.isPanning) {
      const dx = e.clientX - state.lastX;
      const dy = e.clientY - state.lastY;
      state.viewport = pan(state.viewport, dx, dy);
      state.lastX = e.clientX;
      state.lastY = e.clientY;
      state.dirty = true;
      state.onViewportChange(state.viewport);
    }
  });

  canvas.addEventListener('mouseup', () => {
    if (state.isPanning) {
      state.isPanning = false;
      canvas.style.cursor = 'default';
    }
  });

  canvas.addEventListener('mouseleave', () => {
    if (state.isPanning) {
      state.isPanning = false;
      canvas.style.cursor = 'default';
    }
  });

  // Left-click selection (but not if Ctrl held - that's pan)
  canvas.addEventListener('click', (e) => {
    if (e.button !== 0 || e.ctrlKey) return; // Left click only, no Ctrl

    const rect = canvas.getBoundingClientRect();
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;

    const [worldX, worldY] = screenToWorld(state.viewport, screenX, screenY);
    state.onSelect(worldX, worldY);
  });

  // Prevent context menu on right-click (reserve for future)
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
}

/**
 * Create initial interaction state
 */
export function createInteractionState(
  viewport: Viewport,
  onSelect: (x_nm: number, y_nm: number) => void,
  onViewportChange: (vp: Viewport) => void
): InteractionState {
  return {
    viewport,
    isPanning: false,
    lastX: 0,
    lastY: 0,
    dirty: false,
    onSelect,
    onViewportChange,
  };
}
