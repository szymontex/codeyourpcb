# Feature Research

**Domain:** Code-first PCB Design Tool (EDA)
**Researched:** 2026-01-21
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Component placement | Can't design PCB without placing parts | MEDIUM | Need footprint library support |
| Net connections | Defining what connects to what is the whole point | LOW | Core of the DSL |
| Multi-layer support | Even simple boards are 2-layer | MEDIUM | Stackup definition |
| Design Rule Check (DRC) | Every EDA tool has this | HIGH | Clearance, width, drill rules |
| Board outline definition | Manufacturing requires it | LOW | Polygon/shape definition |
| Gerber export | Universal manufacturing format | MEDIUM | Multiple layers, drill files |
| 2D board view | Must see what you're designing | MEDIUM | Top/bottom copper, silk, mask |
| Undo/redo | Expected in any editor | MEDIUM | Command pattern, history |
| Zoom/pan | Basic navigation | LOW | Standard canvas interactions |
| Grid snapping | Alignment is critical | LOW | Configurable grid sizes |
| Net highlighting | See where connections go | LOW | Visual feedback on selection |
| Component rotation | 0/90/180/270 at minimum | LOW | Transform in DSL |

### Differentiators (Competitive Advantage)

Features that set this code-first approach apart.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Git-friendly file format** | Teams can collaborate, diff, merge, review | HIGH | Core value proposition |
| **LLM/AI editable** | "Claude, move this trace" | MEDIUM | Clear DSL = AI can edit |
| **Deterministic builds** | Same file = same output, always | HIGH | Must avoid floating-point randomness |
| **Hot reload** | Edit file → see changes instantly | MEDIUM | File watch + incremental parse |
| **LSP/IDE integration** | Autocomplete, hover, go-to-definition | HIGH | Makes code-first practical |
| **Electrical-aware constraints** | System knows signal types, not just geometry | HIGH | crosstalk_sensitive, high_speed |
| **Declarative modules** | Reusable circuit blocks | MEDIUM | Import/compose patterns |
| **CI/CD testable** | Run DRC in pipeline, fail on violations | LOW | CLI interface |
| **Constraint-based routing** | Say what, not how | HIGH | Autorouter with constraints |
| **AI hints in syntax** | @ai-hint comments for LLM context | LOW | Comment convention |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Real-time collaboration | "Like Figma for PCB" | Complexity explosion, conflict resolution hell | Git-based async workflow |
| Schematic-driven layout | Traditional EDA workflow | Couples two complex systems, harder to maintain | Unified DSL for both (later) |
| Built-in component marketplace | Convenience | Licensing complexity, hosting costs, curation burden | Import from existing (KiCad, etc.) |
| Automatic schematic generation | Derive schematic from code | Hard to make readable, loses design intent | Optional separate schematic file |
| Unlimited undo history | Users want infinite undo | Memory explosion on complex boards | Configurable limit (100-1000) |
| Visual-first mode | Some users prefer GUI | Dilutes code-first value prop | Code-first with visual feedback |
| Manufacturing integration | One-click order | Business complexity, liability, certification | Export files, user orders |

## Feature Dependencies

```
[Parser] ─────────────────────┐
    │                         │
    ▼                         │
[Board Model] ───────────────┬┴──────────────┐
    │                        │               │
    ▼                        ▼               ▼
[Renderer 2D]          [DRC Engine]    [LSP Server]
    │                        │
    ▼                        ▼
[3D Preview]          [Autorouter]
                            │
                            ▼
                     [Gerber Export]
```

### Dependency Notes

- **Renderer requires Board Model:** Can't draw what doesn't exist
- **DRC requires Board Model:** Checks operate on board state
- **LSP requires Parser:** Needs AST for hover/completion
- **Autorouter requires DRC:** Must validate routes against rules
- **3D Preview requires 2D Renderer:** Shares component geometry
- **Gerber Export requires DRC passing:** Don't export invalid designs

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the code-first concept.

- [ ] **Custom DSL parser** — The language IS the product
- [ ] **Board model with components and nets** — Core data structure
- [ ] **2D board view renderer** — Must see results
- [ ] **Hot reload** — Edit-see cycle must be fast
- [ ] **Basic DRC (clearance, width)** — Prevent obvious errors
- [ ] **Gerber export** — Must be manufacturable
- [ ] **Simple footprint support** — At least basic SMD/through-hole
- [ ] **CLI interface** — For CI/CD integration

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **LSP server** — When users want IDE integration
- [ ] **Autorouter integration (FreeRouting)** — When manual routing gets tedious
- [ ] **3D preview** — When users want to check mechanical fit
- [ ] **Undo/redo** — When editing becomes complex
- [ ] **KiCad footprint import** — When component variety matters
- [ ] **Multi-board projects** — When users have complex systems

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Custom autorouter (GPU-accelerated)** — After proving concept with FreeRouting
- [ ] **Ngspice simulation integration** — When users need electrical verification
- [ ] **Schematic view** — When users want traditional EDA workflow option
- [ ] **WASM plugin system** — When extension ecosystem is needed
- [ ] **IPC-2581 export** — When manufacturers request it
- [ ] **Impedance calculator integration** — When high-speed design is common

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| DSL parser | HIGH | HIGH | **P0** |
| Board model | HIGH | MEDIUM | **P0** |
| 2D renderer | HIGH | MEDIUM | **P0** |
| Hot reload | HIGH | LOW | **P0** |
| Gerber export | HIGH | MEDIUM | **P0** |
| Basic DRC | HIGH | MEDIUM | **P1** |
| Footprint support | HIGH | MEDIUM | **P1** |
| CLI interface | MEDIUM | LOW | **P1** |
| LSP server | HIGH | HIGH | **P2** |
| Autorouter | HIGH | LOW (FreeRouting) | **P2** |
| 3D preview | MEDIUM | MEDIUM | **P2** |
| Undo/redo | MEDIUM | MEDIUM | **P2** |
| KiCad import | MEDIUM | MEDIUM | **P2** |
| Plugin system | MEDIUM | HIGH | **P3** |
| Simulation | MEDIUM | HIGH | **P3** |

**Priority key:**
- P0: Must have for MVP launch
- P1: Required for usable product
- P2: Should have, add post-launch
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | KiCad | Eagle | Altium | EasyEDA | **CodeYourPCB** |
|---------|-------|-------|--------|---------|-----------------|
| File format | S-expr (text) | XML | Binary | Cloud | **Custom DSL** |
| Git-friendly | Partial | Poor | Poor | N/A | **Excellent** |
| AI editable | Poor | Poor | Poor | Poor | **Excellent** |
| Learning curve | High | Medium | High | Low | **Medium*** |
| Autorouter | External | Built-in | Built-in | Built-in | External (MVP) |
| Simulation | Basic | Basic | Advanced | Basic | External (ngspice) |
| Price | Free | $$ | $$$$ | Free/$ | **Free/Open** |
| Collaboration | Manual merge | Poor | PDM | Cloud | **Git native** |

*Learning curve for programmers is low; for traditional EDA users is higher initially.

## Sources

- KiCad user feedback and feature requests
- Eagle/Altium marketing materials and documentation
- tscircuit project (similar code-first approach)
- JITX marketing (commercial code-first EDA)
- Brainstorm session requirements discussion

---
*Feature research for: Code-first PCB Design Tool*
*Researched: 2026-01-21*
