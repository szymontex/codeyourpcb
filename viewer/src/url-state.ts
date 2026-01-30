/**
 * URL state encoding and decoding for sharing design views
 * Enables collaboration by encoding viewport and layer visibility in shareable URLs
 */

export interface ViewState {
  layers: string[];      // e.g. ['top', 'bottom', 'ratsnest']
  zoom: number;
  panX: number;
  panY: number;
}

/**
 * Encode view state to URL query parameters
 * Uses short parameter names to keep URLs compact
 */
export function encodeViewState(state: ViewState): string {
  const params = new URLSearchParams();
  params.set('l', state.layers.join(','));
  params.set('z', state.zoom.toFixed(2));
  params.set('x', Math.round(state.panX).toString());
  params.set('y', Math.round(state.panY).toString());
  return '?' + params.toString();
}

/**
 * Decode view state from URL query parameters
 * Returns null if no view state is present in URL
 */
export function decodeViewState(): ViewState | null {
  const params = new URLSearchParams(window.location.search);
  if (!params.has('l')) return null;
  return {
    layers: (params.get('l') || '').split(',').filter(Boolean),
    zoom: parseFloat(params.get('z') || '1'),
    panX: parseFloat(params.get('x') || '0'),
    panY: parseFloat(params.get('y') || '0'),
  };
}
