# Phase 2: Rendering - Research

**Researched:** 2026-01-21
**Domain:** Web-based PCB Visualization, WASM Integration, Hot Reload
**Confidence:** HIGH

## Summary

Phase 2 establishes the visual rendering layer for CodeYourPCB. The research confirms that a **Canvas 2D approach** is optimal for the MVP given the user's explicit goal of "minimal UI for backend verification." The key technical challenges are:

1. **WASM-to-JavaScript data flow**: Exposing the Rust ECS world state to browser rendering
2. **Coordinate transformation**: Converting nanometer coordinates to screen pixels with zoom/pan
3. **Hot reload architecture**: File watching with debounced re-parse and viewport preservation
4. **Layer visualization**: Color-coded copper layers with visibility toggles

The architecture follows a clear separation: Rust handles parsing, ECS world management, and data serialization; JavaScript/TypeScript handles canvas rendering, user interaction, and UI. This keeps rendering code simple while leveraging the existing Phase 1 foundation.

**Primary recommendation:** Use Canvas 2D API for rendering (simpler, fast enough for MVP, browser-native), wasm-bindgen + serde-wasm-bindgen for data transfer, and notify-debouncer-full for file watching. Focus on "does it work" not "does it look good."

---

## Standard Stack

The established libraries/tools for this phase:

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wasm-bindgen | 0.2.x | Rust-to-JS bridge | Official wasm-bindgen project, mature |
| wasm-pack | 0.12+ | Build WASM modules | Official tooling, handles wasm-bindgen-cli |
| serde-wasm-bindgen | 0.6.5 | Serialize Rust structs to JS | Faster than JSON, smaller code size, officially preferred |
| web-sys | 0.3.x | DOM and Canvas API bindings | Official wasm-bindgen companion |
| notify | 8.2.0 | Cross-platform file watching | Used by rust-analyzer, cargo-watch, mdBook |
| notify-debouncer-full | 0.6.0 | Debounced file events | Smart rename handling, duplicate prevention |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| js-sys | 0.3.x | JS interop types | When using JS built-ins (Date, Array, etc.) |
| tokio | 1.x | Async runtime | File watcher async channel integration |
| crossbeam-channel | 0.5.x | Thread-safe channels | Alternative to std::sync::mpsc for notify |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Canvas 2D | wgpu/WebGPU | More complex setup, overkill for 2D MVP, longer initial render time |
| Canvas 2D | WebGL | Middle ground, but requires GLSL shaders, more complexity |
| serde-wasm-bindgen | JSON via serde_json | Larger code size, slower, but simpler debugging |
| notify-debouncer-full | notify-debouncer-mini | Less features but lighter; mini lacks file ID tracking |

**Installation:**

```toml
# crates/cypcb-render/Cargo.toml (new crate for WASM rendering)
[package]
name = "cypcb-render"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
cypcb-core = { path = "../cypcb-core" }
cypcb-parser = { path = "../cypcb-parser" }
cypcb-world = { path = "../cypcb-world" }

wasm-bindgen = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"

[dependencies.web-sys]
version = "0.3"
features = [
    "Window",
    "Document",
    "Element",
    "HtmlCanvasElement",
    "CanvasRenderingContext2d",
    "console",
]

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
notify = "8.2"
notify-debouncer-full = "0.6"
tokio = { version = "1", features = ["sync", "rt"] }
```

---

## Architecture Patterns

### Recommended Project Structure

```
src/                          # Frontend (TypeScript/JavaScript)
  viewer/
    canvas.ts                 # Canvas 2D rendering engine
    viewport.ts               # Zoom/pan state and transformations
    layers.ts                 # Layer visibility and colors
    interaction.ts            # Mouse events, selection
    renderer.ts               # Main render loop, orchestrates drawing
  wasm/
    bridge.ts                 # WASM module loading and API wrapper
  components/
    Viewer.tsx                # Main viewer component (if using React)
    LayerPanel.tsx            # Layer visibility controls
    StatusBar.tsx             # Reload status, coordinates display
  App.tsx

crates/
  cypcb-render/               # NEW: WASM rendering bridge
    src/
      lib.rs                  # wasm-bindgen exports
      render_data.rs          # Serializable render primitives
      board_snapshot.rs       # Board state snapshot for JS
  cypcb-watcher/              # NEW: File watching (non-WASM)
    src/
      lib.rs
      watcher.rs              # notify-debouncer-full integration
```

### Pattern 1: Data Transfer via Serializable Snapshots

**What:** Instead of exposing ECS directly, create a serializable "snapshot" struct that JS can consume.

**When to use:** Any time JS needs board state.

**Example:**

```rust
// crates/cypcb-render/src/board_snapshot.rs
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Serializable board snapshot for JavaScript rendering
#[derive(Serialize)]
pub struct BoardSnapshot {
    pub board: BoardInfo,
    pub components: Vec<ComponentInfo>,
    pub nets: Vec<NetInfo>,
}

#[derive(Serialize)]
pub struct BoardInfo {
    pub name: String,
    pub width_nm: i64,
    pub height_nm: i64,
    pub layer_count: u8,
}

#[derive(Serialize)]
pub struct ComponentInfo {
    pub refdes: String,
    pub value: String,
    pub x_nm: i64,
    pub y_nm: i64,
    pub rotation_mdeg: i32,  // millidegrees
    pub footprint: String,
    pub pads: Vec<PadInfo>,
    pub kind: String,
}

#[derive(Serialize)]
pub struct PadInfo {
    pub number: String,
    pub x_nm: i64,           // relative to component
    pub y_nm: i64,
    pub width_nm: i64,
    pub height_nm: i64,
    pub shape: String,       // "circle", "rect", "roundrect", "oblong"
    pub layer_mask: u32,
    pub drill_nm: Option<i64>,
}

#[derive(Serialize)]
pub struct NetInfo {
    pub name: String,
    pub id: u32,
    pub connections: Vec<PinRef>,
}

#[derive(Serialize)]
pub struct PinRef {
    pub component: String,
    pub pin: String,
}
```

### Pattern 2: WASM API Entry Points

**What:** Clean entry points for JavaScript to call.

**Example:**

```rust
// crates/cypcb-render/src/lib.rs
use wasm_bindgen::prelude::*;
use cypcb_parser::parse;
use cypcb_world::{BoardWorld, sync_ast_to_world};
use cypcb_world::footprint::FootprintLibrary;

mod board_snapshot;
use board_snapshot::BoardSnapshot;

#[wasm_bindgen]
pub struct PcbEngine {
    world: BoardWorld,
    footprint_lib: FootprintLibrary,
    source: String,
    has_errors: bool,
}

#[wasm_bindgen]
impl PcbEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> PcbEngine {
        PcbEngine {
            world: BoardWorld::new(),
            footprint_lib: FootprintLibrary::new(),
            source: String::new(),
            has_errors: false,
        }
    }

    /// Parse source and update world, returns error messages or empty string
    pub fn load_source(&mut self, source: &str) -> String {
        self.source = source.to_string();
        let parse_result = parse(source);

        let mut errors = String::new();
        for err in &parse_result.errors {
            errors.push_str(&format!("{}\n", err));
        }

        self.world.clear();
        let sync_result = sync_ast_to_world(
            &parse_result.value,
            source,
            &mut self.world,
            &self.footprint_lib,
        );

        for err in &sync_result.errors {
            errors.push_str(&format!("{}\n", err));
        }

        self.has_errors = !errors.is_empty();
        errors
    }

    /// Get board snapshot for rendering (returns JsValue)
    pub fn get_snapshot(&mut self) -> JsValue {
        let snapshot = self.create_snapshot();
        serde_wasm_bindgen::to_value(&snapshot).unwrap_or(JsValue::NULL)
    }

    /// Query components at a point (for selection)
    pub fn query_point(&self, x_nm: i64, y_nm: i64) -> JsValue {
        let point = cypcb_core::Point::new(
            cypcb_core::Nm(x_nm),
            cypcb_core::Nm(y_nm),
        );
        let entities = self.world.query_point(point);
        let refdes_list: Vec<String> = entities
            .iter()
            .filter_map(|e| self.world.get::<cypcb_world::RefDes>(*e))
            .map(|r| r.as_str().to_string())
            .collect();
        serde_wasm_bindgen::to_value(&refdes_list).unwrap_or(JsValue::NULL)
    }

    fn create_snapshot(&self) -> BoardSnapshot {
        // Convert ECS world to snapshot...
        // Implementation omitted for brevity
    }
}
```

### Pattern 3: Canvas 2D Rendering with Viewport Transform

**What:** Manage zoom/pan state and apply transformations consistently.

**Example (TypeScript):**

```typescript
// src/viewer/viewport.ts
export interface Viewport {
    // View center in world coordinates (nanometers)
    centerX: number;
    centerY: number;
    // Pixels per nanometer
    scale: number;
    // Canvas dimensions
    width: number;
    height: number;
}

export function worldToScreen(vp: Viewport, worldX: number, worldY: number): [number, number] {
    const screenX = (worldX - vp.centerX) * vp.scale + vp.width / 2;
    // Y is flipped: world Y-up to screen Y-down
    const screenY = vp.height / 2 - (worldY - vp.centerY) * vp.scale;
    return [screenX, screenY];
}

export function screenToWorld(vp: Viewport, screenX: number, screenY: number): [number, number] {
    const worldX = (screenX - vp.width / 2) / vp.scale + vp.centerX;
    // Y is flipped
    const worldY = vp.centerY - (screenY - vp.height / 2) / vp.scale;
    return [worldX, worldY];
}

export function zoomAtPoint(vp: Viewport, screenX: number, screenY: number, factor: number): Viewport {
    // Get world point before zoom
    const [worldX, worldY] = screenToWorld(vp, screenX, screenY);

    // Apply zoom
    const newScale = vp.scale * factor;

    // Adjust center so world point stays at same screen position
    const newCenterX = worldX - (screenX - vp.width / 2) / newScale;
    const newCenterY = worldY + (screenY - vp.height / 2) / newScale;

    return {
        ...vp,
        scale: newScale,
        centerX: newCenterX,
        centerY: newCenterY,
    };
}
```

### Pattern 4: Layer Colors (KiCad-style)

**What:** Standard PCB layer color scheme familiar to users.

**Example:**

```typescript
// src/viewer/layers.ts
export interface LayerConfig {
    name: string;
    color: string;
    visible: boolean;
    opacity: number;
}

// KiCad default colors
export const DEFAULT_LAYER_COLORS: Record<string, string> = {
    'top_copper': '#C83434',      // Red
    'bottom_copper': '#3434C8',   // Blue
    'inner_1': '#C8C800',         // Yellow
    'inner_2': '#C800C8',         // Magenta
    'top_silk': '#C8C8C8',        // Light gray
    'bottom_silk': '#808080',     // Gray
    'top_mask': '#14780A80',      // Green, semi-transparent
    'bottom_mask': '#1480AA80',   // Teal, semi-transparent
    'pads_th': '#C8C8C8',         // Light gray for through-hole pads
    'pads_smd': '#C83434',        // Same as top copper
    'drill': '#FFFFFF',           // White drill holes
    'outline': '#FFFF00',         // Yellow board outline
    'grid': '#30303080',          // Dark gray, semi-transparent
};

// Light mode background
export const LIGHT_BACKGROUND = '#FFFFFF';
// Dark mode background (for later)
export const DARK_BACKGROUND = '#1E1E1E';
```

### Anti-Patterns to Avoid

- **Exposing ECS directly to JS:** Creates tight coupling, hard to serialize. Use snapshot pattern.
- **Rendering in WASM:** Canvas 2D API is browser-native, faster to call from JS than bridge from WASM.
- **Polling for file changes:** Use notify's event-driven approach with debouncing.
- **Storing viewport in WASM:** JS owns UI state; WASM is stateless data processor.
- **Complex state sync:** Keep it simple - on file change, re-parse entire file, replace world.

---

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WASM-JS interop | Custom JS bindings | wasm-bindgen | Generates optimal bindings, handles memory |
| Rust struct to JS | Manual conversion | serde-wasm-bindgen | Fast, automatic, handles complex types |
| File watching | `std::fs` polling | notify + debouncer | Cross-platform, efficient, debounces events |
| Canvas transforms | Manual matrix math | ctx.setTransform() | Browser-native, GPU-accelerated |
| Zoom to cursor | Custom calculation | Standard formula | Well-documented pattern, easy to get wrong |

**Key insight:** The rendering layer should be thin. The hard work (parsing, ECS, spatial queries) is already done in Rust. JavaScript just draws what Rust computed.

---

## Common Pitfalls

### Pitfall 1: Coordinate System Confusion

**What goes wrong:** Y-axis direction mismatch. Components appear flipped or inverted.

**Why it happens:**
- Rust uses Y-up (mathematical, matches Gerber)
- Canvas uses Y-down (screen coordinates)
- Confusion at the transformation boundary

**How to avoid:**
1. All Rust code uses Y-up (already established in Phase 1)
2. Single transformation point in viewport.ts handles flip
3. Test with asymmetric design early
4. Board outline should have origin marker visible

**Warning signs:** Components appear mirrored, text is backwards.

### Pitfall 2: WASM Memory Leaks

**What goes wrong:** Memory grows continuously as user reloads files.

**Why it happens:** WASM linear memory isn't garbage collected. Each reload creates new objects.

**How to avoid:**
1. Clear world state on reload (already have `BoardWorld::clear()`)
2. Reuse PcbEngine instance, don't recreate
3. Monitor browser memory in dev tools
4. Avoid holding JS references to WASM objects

**Warning signs:** Memory tab shows steady growth, page becomes sluggish.

### Pitfall 3: Debounce Too Short/Long

**What goes wrong:**
- Too short: Multiple rebuilds per save (especially on slow machines)
- Too long: Perceived lag between save and update

**Why it happens:** File systems emit multiple events per save (write, metadata, rename on atomic save).

**How to avoid:**
1. Start with 200ms debounce
2. Make configurable if needed
3. Use notify-debouncer-full which handles rename pairs
4. Test with real editor saves, not manual touch

**Warning signs:** Console shows multiple "reloaded" messages, or user complains of lag.

### Pitfall 4: Selection Click Tolerance

**What goes wrong:** Can't click on small pads, or wrong component selected.

**Why it happens:** Click point must be converted to world coordinates and tolerance must be zoom-aware.

**How to avoid:**
1. Convert click to world coordinates first
2. Expand query region by a fixed screen-space tolerance (e.g., 5px)
3. Convert that tolerance to world units based on current zoom
4. If multiple hits, prioritize by: 1) exact hit, 2) smallest bounding box

**Warning signs:** Selection feels "off," especially at different zoom levels.

### Pitfall 5: Re-render on Every Mouse Move

**What goes wrong:** Performance degrades, CPU spins.

**Why it happens:** mousemove events fire very frequently.

**How to avoid:**
1. Use requestAnimationFrame for rendering
2. Set dirty flag on state change, don't render directly
3. Only re-render when: viewport changed, data changed, selection changed
4. Hover effects can update more frequently but throttle to ~30fps

**Warning signs:** High CPU usage when mouse is moving over canvas.

---

## Code Examples

Verified patterns from research:

### Canvas 2D Setup

```typescript
// src/viewer/canvas.ts
export function setupCanvas(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Canvas 2D not supported');

    // Handle high-DPI displays
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    return ctx;
}
```

### Drawing Pads

```typescript
// src/viewer/renderer.ts
function drawPad(ctx: CanvasRenderingContext2D, vp: Viewport, pad: PadInfo, compX: number, compY: number, rotation: number) {
    const [screenX, screenY] = worldToScreen(vp, compX + pad.x_nm, compY + pad.y_nm);
    const width = pad.width_nm * vp.scale;
    const height = pad.height_nm * vp.scale;

    ctx.save();
    ctx.translate(screenX, screenY);
    ctx.rotate(-rotation * Math.PI / 180000);  // Convert millidegrees, negate for Y-flip

    switch (pad.shape) {
        case 'circle':
            ctx.beginPath();
            ctx.arc(0, 0, width / 2, 0, Math.PI * 2);
            ctx.fill();
            break;
        case 'rect':
            ctx.fillRect(-width / 2, -height / 2, width, height);
            break;
        case 'roundrect':
            drawRoundRect(ctx, -width / 2, -height / 2, width, height, Math.min(width, height) * 0.25);
            ctx.fill();
            break;
        case 'oblong':
            drawOblong(ctx, -width / 2, -height / 2, width, height);
            ctx.fill();
            break;
    }

    // Drill hole for through-hole pads
    if (pad.drill_nm) {
        const drillRadius = pad.drill_nm * vp.scale / 2;
        ctx.fillStyle = '#000000';
        ctx.beginPath();
        ctx.arc(0, 0, drillRadius, 0, Math.PI * 2);
        ctx.fill();
    }

    ctx.restore();
}
```

### Hot Reload File Watcher (Rust, non-WASM)

```rust
// crates/cypcb-watcher/src/watcher.rs
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct FileWatcher {
    _debouncer: notify_debouncer_full::Debouncer<notify::RecommendedWatcher, notify_debouncer_full::FileIdMap>,
}

impl FileWatcher {
    pub fn new(
        path: &Path,
        sender: mpsc::Sender<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let path_owned = path.to_path_buf();

        let debouncer = new_debouncer(
            Duration::from_millis(200),
            None,
            move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    for event in events {
                        if event.paths.iter().any(|p| p.extension().map(|e| e == "cypcb").unwrap_or(false)) {
                            let _ = sender.blocking_send(path_owned.to_string_lossy().to_string());
                        }
                    }
                }
            },
        )?;

        debouncer.watcher().watch(path, notify::RecursiveMode::NonRecursive)?;

        Ok(FileWatcher { _debouncer: debouncer })
    }
}
```

### Mouse Interaction Setup

```typescript
// src/viewer/interaction.ts
export function setupInteraction(canvas: HTMLCanvasElement, state: ViewerState) {
    // Wheel zoom (zoom to cursor)
    canvas.addEventListener('wheel', (e) => {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        const factor = e.deltaY < 0 ? 1.1 : 0.9;
        state.viewport = zoomAtPoint(state.viewport, x, y, factor);
        state.dirty = true;
    }, { passive: false });

    // Middle-click pan
    let isPanning = false;
    let lastX = 0, lastY = 0;

    canvas.addEventListener('mousedown', (e) => {
        if (e.button === 1) {  // Middle button
            isPanning = true;
            lastX = e.clientX;
            lastY = e.clientY;
            e.preventDefault();
        }
    });

    canvas.addEventListener('mousemove', (e) => {
        if (isPanning) {
            const dx = e.clientX - lastX;
            const dy = e.clientY - lastY;
            // Pan in world space (invert Y due to coordinate flip)
            state.viewport.centerX -= dx / state.viewport.scale;
            state.viewport.centerY += dy / state.viewport.scale;
            lastX = e.clientX;
            lastY = e.clientY;
            state.dirty = true;
        }
    });

    canvas.addEventListener('mouseup', () => {
        isPanning = false;
    });
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| JSON for WASM-JS | serde-wasm-bindgen | 2023+ | Smaller code, faster serialization |
| Raw notify events | notify-debouncer-full | 2024+ | Better rename handling, less noise |
| wgpu for all 2D | Canvas 2D for simple cases | Always valid | Simpler MVP, wgpu for complex later |
| React for everything | Vanilla TS acceptable for canvas | Always valid | Less overhead for rendering-heavy apps |

**Deprecated/outdated:**
- **wasm-bindgen serde feature**: Superseded by serde-wasm-bindgen crate
- **notify 5.x/6.x**: Use 8.x for current API, debouncer compatibility

---

## Open Questions

Things that couldn't be fully resolved:

1. **Flip view implementation**
   - What we know: User wants flip view to mirror display like turning board over
   - What's unclear: Should layers actually swap (top->bottom) or just mirror visually?
   - Recommendation: Mirror visually AND swap layer visibility (top becomes bottom view)

2. **Error display during hot reload**
   - What we know: Parse errors should be shown, viewport preserved
   - What's unclear: Show errors inline on canvas? Separate panel? Toast notification?
   - Recommendation: Start with console + simple status bar message. Enhance later.

3. **Grid snap implementation**
   - What we know: Need grid display and possibly snap (RND-06 requirement)
   - What's unclear: Grid is for display only in Phase 2 (no editing). Defer snap?
   - Recommendation: Grid display only for Phase 2. Snap is editing feature (later phase).

---

## Sources

### Primary (HIGH confidence)
- [wasm-bindgen Guide](https://rustwasm.github.io/docs/wasm-bindgen/) - WASM-JS interop patterns
- [serde-wasm-bindgen docs](https://docs.rs/serde-wasm-bindgen/0.6.5/serde_wasm_bindgen/) - Serialization API
- [notify 8.2 docs](https://docs.rs/notify/8.2.0/notify/) - File watching API
- [notify-debouncer-full docs](https://docs.rs/notify-debouncer-full/0.6.0/notify_debouncer_full/) - Debouncing configuration
- [web-sys CanvasRenderingContext2d](https://docs.rs/web-sys/latest/web_sys/struct.CanvasRenderingContext2d.html) - Canvas API in Rust

### Secondary (MEDIUM confidence)
- [Tauri Calling Rust from Frontend](https://v2.tauri.app/develop/calling-rust/) - Tauri IPC patterns
- [Canvas Pan and Zoom examples](https://harrisonmilbradt.com/blog/canvas-panning-and-zooming) - Zoom/pan implementation
- [2D Web Rendering with Rust](https://medium.com/lagierandlagier/2d-web-rendering-with-rust-4401cf133f31) - WASM rendering approaches
- [KiCad PCB Editor docs](https://docs.kicad.org/8.0/en/pcbnew/pcbnew.html) - Layer color reference

### Tertiary (LOW confidence)
- [Canvas vs WebGL performance](https://semisignal.com/a-look-at-2d-vs-webgl-canvas-performance/) - Performance comparison (may vary)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries well-documented, production-proven
- Architecture patterns: HIGH - Based on established wasm-bindgen patterns
- Hot reload: HIGH - notify is battle-tested, used by major Rust tools
- Canvas rendering: HIGH - Browser-native API, extremely stable
- Layer colors: MEDIUM - Based on KiCad defaults, customizable

**Research date:** 2026-01-21
**Valid until:** 2026-03-21 (60 days - stable domain, libraries mature)

---

## Recommended Plan Structure

Based on research, Phase 2 should be structured as:

### Task Groups

1. **WASM Crate Setup** (2-3 tasks)
   - Create cypcb-render crate with wasm-bindgen
   - Set up wasm-pack build
   - Create BoardSnapshot types

2. **WASM API** (3-4 tasks)
   - PcbEngine struct with load_source
   - get_snapshot for rendering data
   - query_point for selection
   - Error message handling

3. **Frontend Scaffolding** (2-3 tasks)
   - TypeScript project setup
   - WASM module loading
   - Basic HTML/CSS layout

4. **Canvas Rendering** (4-5 tasks)
   - Viewport and coordinate transformation
   - Layer color configuration
   - Pad rendering (all shapes)
   - Component outline rendering
   - Board outline and grid

5. **Interaction** (3-4 tasks)
   - Zoom to cursor (scroll wheel)
   - Pan (middle-click drag)
   - Selection (left-click)
   - Layer visibility toggles

6. **Hot Reload** (2-3 tasks)
   - File watcher integration (Tauri or separate server)
   - Viewport preservation on reload
   - Status message display

7. **Polish** (2-3 tasks)
   - Flip view implementation
   - Net highlighting on selection
   - Light/dark mode toggle

### Critical Path

```
WASM Crate Setup
    |
    v
WASM API --> Frontend Scaffolding
    |              |
    v              v
BoardSnapshot --> Canvas Rendering
                       |
                       v
                   Interaction --> Hot Reload --> Polish
```

### Estimated Scope

- **Total tasks:** 18-25
- **Estimated effort:** Medium (1-2 weeks for experienced developer)
- **Risk areas:** WASM build tooling (can be fiddly), coordinate transform (must get right)

### Key Constraints from CONTEXT.md

- KiCad-style layer colors (red top, blue bottom)
- Light mode as default
- Zoom to cursor (scroll wheel)
- Pan via middle-click
- Viewport preserved on reload
- "Reloaded" status notification
- Flip view button
