# Requirements: CodeYourPCB

**Defined:** 2026-01-21
**Core Value:** The source file is the design — git-friendly, AI-editable, deterministic

## v1 Requirements

### Parser & DSL

- [ ] **DSL-01**: Custom Tree-sitter grammar for .pcb files
- [ ] **DSL-02**: Board definition (size, layers, stackup)
- [ ] **DSL-03**: Component instantiation with footprint reference
- [ ] **DSL-04**: Net connections with constraint syntax
- [ ] **DSL-05**: Module/import system for reusable blocks
- [ ] **DSL-06**: Hot reload on file save

### Board Model

- [ ] **BRD-01**: Component placement (absolute and relative)
- [ ] **BRD-02**: Multi-layer support (2-32 layers)
- [ ] **BRD-03**: Net/connection tracking
- [ ] **BRD-04**: Board outline definition
- [ ] **BRD-05**: Zones and keepouts
- [ ] **BRD-06**: Spatial indexing (R*-tree)

### Rendering

- [ ] **RND-01**: 2D top/bottom board view
- [ ] **RND-02**: Layer visibility toggle
- [ ] **RND-03**: Zoom/pan navigation
- [ ] **RND-04**: Component selection and highlighting
- [ ] **RND-05**: Net highlighting
- [ ] **RND-06**: Grid display and snapping

### Design Rules

- [ ] **DRC-01**: Clearance checking (trace-trace, trace-pad)
- [ ] **DRC-02**: Minimum trace width validation
- [ ] **DRC-03**: Minimum drill size validation
- [ ] **DRC-04**: Unconnected pin detection
- [ ] **DRC-05**: Real-time DRC feedback

### Export

- [x] **EXP-01**: Gerber X2 export (all layers)
- [x] **EXP-02**: Excellon drill file export
- [x] **EXP-03**: BOM generation (CSV/JSON)
- [x] **EXP-04**: Pick and place file

### Footprints

- [ ] **FTP-01**: Basic SMD footprints (0402-2512)
- [ ] **FTP-02**: Basic through-hole footprints
- [ ] **FTP-03**: QFP/SOIC/SOT packages
- [ ] **FTP-04**: Custom footprint definition in DSL
- [x] **FTP-05**: KiCad footprint import

### Developer Experience

- [x] **DEV-01**: CLI interface for headless operation
- [x] **DEV-02**: LSP server for IDE integration
- [ ] **DEV-03**: Error messages with line/column info
- [ ] **DEV-04**: Web-based viewer

### Intelligence

- [x] **INT-01**: Autorouter integration (FreeRouting)
- [x] **INT-02**: Trace width calculator (IPC-2221)
- [x] **INT-03**: Electrical-aware constraints (crosstalk, impedance hints)

## v2 Requirements

### Advanced Features

- **ADV-01**: 3D board preview
- **ADV-02**: Schematic view generation
- **ADV-03**: Ngspice simulation integration
- **ADV-04**: Custom GPU autorouter
- **ADV-05**: WASM plugin system
- **ADV-06**: IPC-2581 export
- **ADV-07**: Impedance calculator with stackup
- **ADV-08**: Differential pair routing
- **ADV-09**: Length matching

### Desktop

- **DSK-01**: Tauri desktop application
- **DSK-02**: Native file dialogs
- **DSK-03**: Undo/redo system
- **DSK-04**: Project templates

## Out of Scope

| Feature | Reason |
|---------|--------|
| Real-time collaboration | Git-based workflow is the collaboration model |
| Component marketplace | Use existing libraries (KiCad, etc.) |
| Manufacturing ordering | Export files, user chooses fab |
| Mobile app | Desktop/web first |
| Schematic-driven layout | Unified DSL approach instead |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DSL-01 | Phase 1 | Complete |
| DSL-02 | Phase 1 | Complete |
| DSL-03 | Phase 1 | Complete |
| DSL-04 | Phase 1 | Complete |
| DSL-05 | Phase 2 | Pending |
| DSL-06 | Phase 2 | Pending |
| BRD-01 | Phase 1 | Complete |
| BRD-02 | Phase 1 | Complete |
| BRD-03 | Phase 1 | Complete |
| BRD-04 | Phase 1 | Complete |
| BRD-05 | Phase 3 | Pending |
| BRD-06 | Phase 1 | Complete |
| RND-01 | Phase 2 | Pending |
| RND-02 | Phase 2 | Pending |
| RND-03 | Phase 2 | Pending |
| RND-04 | Phase 2 | Pending |
| RND-05 | Phase 2 | Pending |
| RND-06 | Phase 2 | Pending |
| DRC-01 | Phase 3 | Pending |
| DRC-02 | Phase 3 | Pending |
| DRC-03 | Phase 3 | Pending |
| DRC-04 | Phase 3 | Pending |
| DRC-05 | Phase 3 | Pending |
| EXP-01 | Phase 4 | Complete |
| EXP-02 | Phase 4 | Complete |
| EXP-03 | Phase 4 | Complete |
| EXP-04 | Phase 4 | Complete |
| FTP-01 | Phase 1 | Complete |
| FTP-02 | Phase 1 | Complete |
| FTP-03 | Phase 3 | Pending |
| FTP-04 | Phase 3 | Pending |
| FTP-05 | Phase 5 | Complete |
| DEV-01 | Phase 4 | Complete |
| DEV-02 | Phase 5 | Complete |
| DEV-03 | Phase 1 | Complete |
| DEV-04 | Phase 2 | Pending |
| INT-01 | Phase 5 | Complete |
| INT-02 | Phase 5 | Complete |
| INT-03 | Phase 5 | Complete |

**Coverage:**
- v1 requirements: 35 total
- Mapped to phases: 35
- Unmapped: 0 ✓

---
*Requirements defined: 2026-01-21*
*Last updated: 2026-01-21 after initial definition*
