# Phase 7: Navigation Controls - Research

**Researched:** 2026-01-28
**Domain:** Browser input events, touchpad gesture detection
**Confidence:** HIGH

## Summary

Phase 7 requires implementing alternative navigation controls for laptop users without middle-click capability. The research focused on modern browser APIs for detecting touchpad gestures (two-finger pan, pinch-to-zoom) and keyboard-modified mouse events.

**Current state analysis:** The viewer already implements Ctrl+LMB pan (NAV-01 complete). The wheel event with `{ passive: false }` already handles pinch-to-zoom via `ctrlKey` detection (NAV-03 works). The primary gap is NAV-02: detecting true multi-touch touchpad panning via Pointer Events API.

The standard approach uses **Pointer Events API** (Baseline: Widely available since July 2020) for multi-touch detection, combined with the existing wheel event handler. Browser APIs intentionally obscure differences between mouse and touchpad input, so the same code handles both naturally.

**Primary recommendation:** Use Pointer Events API to detect two simultaneous pointers for pan gestures, maintain existing wheel event handler for pinch-zoom (already works via ctrlKey), and set `touch-action: none` on canvas to prevent browser hijacking.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Pointer Events API | Level 2 (Web Standard) | Unified mouse/touch/pen input | Baseline: widely available since July 2020, supersedes touch events |
| WheelEvent API | Web Standard | Scroll and pinch-zoom detection | Already in use, ctrlKey indicates zoom actions |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| N/A | - | Pure vanilla JS | This phase doesn't require libraries |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Pointer Events | Touch Events | Touch Events are legacy, pointer events are the modern standard |
| Pointer Events | Gesture libraries (ZingTouch, @use-gesture/vanilla, The Finger) | Adds dependencies for minimal functionality; overkill for simple pan/zoom |
| Native implementation | Three.js OrbitControls pattern | Three.js OrbitControls has known touchpad issues (Issue #22525), we have simpler needs |

**Installation:**
```bash
# No installation needed - native browser APIs
```

## Architecture Patterns

### Recommended Project Structure
```
viewer/src/
├── interaction.ts    # Already exists - extend with pointer events
└── viewport.ts       # Already exists - no changes needed
```

### Pattern 1: Pointer Events for Multi-Touch Pan

**What:** Track multiple simultaneous pointers to detect two-finger pan gestures.

**When to use:** For touchpad two-finger panning that should pan the viewport (not scroll the page).

**Example:**
```typescript
// Source: https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events/Multi-touch_interaction
// Adapted for pan detection instead of pinch

interface PointerCache {
  pointerId: number;
  clientX: number;
  clientY: number;
}

const pointerCache: PointerCache[] = [];

canvas.addEventListener('pointerdown', (e) => {
  pointerCache.push({ pointerId: e.pointerId, clientX: e.clientX, clientY: e.clientY });
});

canvas.addEventListener('pointermove', (e) => {
  const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
  if (index === -1) return;

  if (pointerCache.length === 2) {
    // Two-finger pan: calculate average movement
    const cached = pointerCache[index];
    const dx = e.clientX - cached.clientX;
    const dy = e.clientY - cached.clientY;

    // Apply pan (half delta since both fingers contribute)
    state.viewport = pan(state.viewport, dx / 2, dy / 2);
    state.dirty = true;
    state.onViewportChange(state.viewport);
  }

  // Update cache
  pointerCache[index] = { pointerId: e.pointerId, clientX: e.clientX, clientY: e.clientY };
});

canvas.addEventListener('pointerup', (e) => {
  const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
  if (index !== -1) {
    pointerCache.splice(index, 1);
  }
});

// Also handle pointercancel, pointerout, pointerleave
['pointercancel', 'pointerout', 'pointerleave'].forEach(eventType => {
  canvas.addEventListener(eventType, (e) => {
    const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
    if (index !== -1) {
      pointerCache.splice(index, 1);
    }
  });
});
```

### Pattern 2: Touch-Action CSS Property

**What:** Disable browser gesture handling so JavaScript can handle gestures.

**When to use:** On canvas element to prevent browser from intercepting touch/pointer gestures.

**Example:**
```css
/* Source: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/touch-action */
canvas {
  touch-action: none;
}
```

**Critical:** Set `touch-action: none` in CSS, not JavaScript. CSS setting allows browser's compositor thread to handle it declaratively.

### Pattern 3: Wheel Event Already Handles Pinch-Zoom

**What:** Current wheel event listener already detects pinch-zoom via `ctrlKey`.

**When to use:** Already implemented correctly in interaction.ts line 32-43.

**Current implementation is correct:**
```typescript
// Pinch-to-zoom on touchpad fires wheel event with ctrlKey: true
canvas.addEventListener('wheel', (e) => {
  e.preventDefault();
  const rect = canvas.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;

  // Browser sends ctrlKey: true for pinch gestures
  const factor = e.deltaY < 0 ? 1.15 : 0.87;
  state.viewport = zoomAtPoint(state.viewport, x, y, factor);
  state.dirty = true;
  state.onViewportChange(state.viewport);
}, { passive: false });
```

### Anti-Patterns to Avoid

- **Using Touch Events instead of Pointer Events:** Touch Events are legacy (2011), Pointer Events are the modern standard (2020+)
- **Trying to distinguish mouse from touchpad:** Browser APIs intentionally obscure this difference. Design gestures that work for both.
- **Using `{ passive: true }` on wheel listener:** Prevents `preventDefault()`, which is needed to stop browser zoom
- **Setting touch-action in JavaScript:** Use CSS instead for better compositor thread performance
- **Tracking pointers globally instead of in cache:** Pointer IDs can be reused; always remove on pointerup/cancel
- **Forgetting pointercancel/pointerout/pointerleave:** Gestures can be interrupted; clean up cache on all exit events

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Gesture library | Custom multi-touch library with swipe/rotate/pinch | Native Pointer Events API | Already in all browsers since 2020; gesture libs are overkill for pan/zoom |
| Momentum scrolling | Custom inertia physics | Browser native (don't implement) | Touchpads already provide momentum in wheel events |
| Device detection | Parse userAgent to detect touchpad | Accept all input equally | Browser APIs are input-agnostic by design; don't fight it |

**Key insight:** Modern browser APIs are designed for input-agnostic code. The best implementation handles mouse, touchpad, and touchscreen identically by using Pointer Events.

## Common Pitfalls

### Pitfall 1: PointerCancel Breaking Gestures

**What goes wrong:** User starts two-finger pan, browser decides to handle scrolling instead, sends `pointercancel`, gesture breaks, fingers remain "stuck" in cache.

**Why it happens:** Browser inspects `touch-action` CSS when gesture *starts*. If not set to `none`, browser can hijack gesture mid-interaction.

**How to avoid:**
1. Set `touch-action: none` in CSS on canvas element
2. Always clean up pointer cache on `pointercancel`, `pointerout`, `pointerleave` events

**Warning signs:**
- Panning stops working mid-gesture
- Subsequent gestures don't work until page reload
- Console shows "pointer not found" errors

### Pitfall 2: Passive Event Listener Prevents preventDefault

**What goes wrong:** Wheel event zoom doesn't prevent browser zoom, so both custom zoom AND browser zoom happen simultaneously.

**Why it happens:** Chrome warns about non-passive wheel listeners for performance. Developer adds `{ passive: true }` to silence warning, which disables `preventDefault()`.

**How to avoid:**
```typescript
// WRONG - passive prevents preventDefault
canvas.addEventListener('wheel', zoom, { passive: true });

// CORRECT - must be non-passive to prevent default
canvas.addEventListener('wheel', zoom, { passive: false });
```

**Warning signs:**
- Browser zooms entire page while custom zoom also happens
- Users report "double zoom" behavior
- `preventDefault()` has no effect

### Pitfall 3: Touch-Action Inheritance Confusion

**What goes wrong:** Set `touch-action: none` on canvas, but browser still hijacks gestures.

**Why it happens:** When a gesture starts, browser intersects `touch-action` values from touched element up to first scrolling ancestor. Parent elements with `touch-action: auto` can override.

**How to avoid:**
```css
/* Set touch-action on canvas, not parent container */
canvas {
  touch-action: none;  /* Correct */
}

/* If parent is scrollable, this won't help: */
#viewer-container {
  touch-action: none;  /* Wrong - apply to canvas */
}
```

**Warning signs:**
- touch-action in DevTools shows "none" but gestures still intercepted
- Works in CodePen but not in production (different DOM structure)

### Pitfall 4: Forgetting Pointer Cleanup Events

**What goes wrong:** Pointer cache grows unbounded, old pointers never removed, memory leak, gesture detection wrong.

**Why it happens:** Only handling `pointerup`, forgetting `pointercancel`, `pointerout`, `pointerleave`.

**How to avoid:**
```typescript
// Handle ALL pointer exit events
const cleanupPointer = (e: PointerEvent) => {
  const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
  if (index !== -1) pointerCache.splice(index, 1);
};

canvas.addEventListener('pointerup', cleanupPointer);
canvas.addEventListener('pointercancel', cleanupPointer);
canvas.addEventListener('pointerout', cleanupPointer);
canvas.addEventListener('pointerleave', cleanupPointer);
```

**Warning signs:**
- Pointer cache length grows over time
- First gesture works, subsequent gestures behave incorrectly
- Pan works with different number of fingers than expected

### Pitfall 5: Accessibility - Breaking Browser Zoom

**What goes wrong:** Users with low vision can't zoom the page because `touch-action: none` prevents pinch-zoom.

**Why it happens:** Canvas is full-viewport and disables all touch actions, including accessibility zoom.

**How to avoid:**
- For this phase, this is acceptable because the canvas implements its own zoom
- Custom zoom must be keyboard accessible (already is via Ctrl+wheel)
- Document the tradeoff: canvas zoom replaces browser zoom

**Warning signs:**
- Screen reader users report zoom not working
- Fails WCAG 1.4.4 (Resize text) testing

## Code Examples

Verified patterns from official sources:

### Multi-Touch Pointer Event Handling

```typescript
// Source: https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events/Multi-touch_interaction
// Full example for two-finger pan detection

interface PointerCache {
  pointerId: number;
  clientX: number;
  clientY: number;
}

export function setupTwoFingerPan(
  canvas: HTMLCanvasElement,
  state: InteractionState
): void {
  const pointerCache: PointerCache[] = [];

  canvas.addEventListener('pointerdown', (e) => {
    pointerCache.push({
      pointerId: e.pointerId,
      clientX: e.clientX,
      clientY: e.clientY,
    });
  });

  canvas.addEventListener('pointermove', (e) => {
    const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
    if (index === -1) return;

    if (pointerCache.length === 2) {
      // Two-finger pan
      const cached = pointerCache[index];
      const dx = e.clientX - cached.clientX;
      const dy = e.clientY - cached.clientY;

      // Pan viewport (half delta because both fingers contribute)
      state.viewport = pan(state.viewport, dx / 2, dy / 2);
      state.dirty = true;
      state.onViewportChange(state.viewport);
    }

    // Update cache
    pointerCache[index] = {
      pointerId: e.pointerId,
      clientX: e.clientX,
      clientY: e.clientY,
    };
  });

  // Cleanup on all pointer exit events
  const cleanupPointer = (e: PointerEvent) => {
    const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
    if (index !== -1) {
      pointerCache.splice(index, 1);
    }
  };

  canvas.addEventListener('pointerup', cleanupPointer);
  canvas.addEventListener('pointercancel', cleanupPointer);
  canvas.addEventListener('pointerout', cleanupPointer);
  canvas.addEventListener('pointerleave', cleanupPointer);
}
```

### CSS Touch-Action Configuration

```css
/* Source: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/touch-action */

/* Disable all browser touch handling on canvas */
canvas {
  touch-action: none;
}

/* Alternative: Allow only pinch-zoom (if we wanted to disable pan) */
/* canvas {
  touch-action: pinch-zoom;
} */

/* Alternative: Allow only vertical pan (if horizontal was for our app) */
/* canvas {
  touch-action: pan-y;
} */
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Touch Events (touchstart, touchmove, touchend) | Pointer Events (pointerdown, pointermove, pointerup) | July 2020 (Baseline) | Unified API for mouse, touch, and pen input |
| Mousewheel event | Wheel event | 2014 (Chrome M35) | Deprecated non-standard event replaced with standard |
| Custom gesture libraries (Hammer.js, etc.) | Native Pointer Events | 2020+ | Libraries no longer needed for basic gestures |
| Safari GestureEvent (gesturestart, gesturechange) | Wheel event with ctrlKey | Safari 9.1 (2016) | Safari proprietary event; cross-browser uses wheel+ctrlKey |

**Deprecated/outdated:**
- **Touch Events:** Still supported but superseded by Pointer Events for new development
- **mousewheel event:** Non-standard, use `wheel` event instead
- **Gesture libraries for basic pan/zoom:** Overkill when Pointer Events API is sufficient
- **Safari GestureEvent:** Safari-only API, use wheel event with ctrlKey for cross-browser

## Open Questions

Things that couldn't be fully resolved:

1. **Two-finger vs three-finger distinction**
   - What we know: NAV-02 mentions "two-finger/three-finger touchpad panning" but doesn't specify different behaviors
   - What's unclear: Should two-finger and three-finger gestures do different things?
   - Recommendation: Implement two-finger pan only. Browsers/OS typically reserve three-finger for system gestures (Exposé, workspace switching). Detecting 3+ fingers works the same way (check `pointerCache.length === 3`) but conflicts with OS.

2. **Distinguishing touchpad from mouse**
   - What we know: Browser APIs intentionally obscure this difference (confirmed by multiple sources)
   - What's unclear: Why requirements mention "touchpad panning" specifically when it works for touch too
   - Recommendation: Implement input-agnostic code. Two-finger pan works on touchpads AND touchscreens identically.

3. **Pinch-to-zoom already works**
   - What we know: Current wheel event handler with `{ passive: false }` and ctrlKey detection already handles pinch-zoom
   - What's unclear: NAV-03 is marked as a requirement but appears already complete
   - Recommendation: Verify in testing. Chrome/Firefox encode pinch as wheel+ctrlKey since 2014, current code should work.

## Sources

### Primary (HIGH confidence)
- [MDN: Pointer Events](https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events) - Web standard API documentation
- [MDN: Pinch Zoom Gestures](https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events/Pinch_zoom_gestures) - Official implementation guide
- [MDN: touch-action](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/touch-action) - CSS property documentation
- [MDN: Wheel Event](https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event) - WheelEvent API reference

### Secondary (MEDIUM confidence)
- [Dan Burzo: Pinch me, I'm zooming](https://danburzo.ro/dom-gestures/) - Cross-browser gesture implementation patterns (verified with MDN)
- [Microsoft Edge Blog: Building a great touchpad experience](https://blogs.windows.com/msedgedev/2017/12/07/better-precision-touchpad-experience-ptp-pointer-events/) - Pointer Events implementation guidance
- [Three.js Issue #22525: OrbitControls touchpad support](https://github.com/mrdoob/three.js/issues/22525) - Known issues with touchpad in similar use cases

### Tertiary (LOW confidence)
- [ZingTouch gesture library](https://zingchart.github.io/zingtouch/) - Alternative approach (not recommended for this phase)
- [@use-gesture/vanilla](https://github.com/pmndrs/use-gesture) - Alternative library (not recommended)
- [The Finger](https://thefinger.dev/) - Newer gesture library (not recommended)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Pointer Events API is W3C standard, baseline widely available since 2020
- Architecture: HIGH - MDN provides official code examples, verified patterns
- Pitfalls: HIGH - Well-documented issues from MDN, GitHub issues, community posts

**Research date:** 2026-01-28
**Valid until:** 2026-02-28 (30 days - stable web standards)
