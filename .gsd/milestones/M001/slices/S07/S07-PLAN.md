# S07: File Picker

**Goal:** Create file picker utilities and UI elements for loading .
**Demo:** Create file picker utilities and UI elements for loading .

## Must-Haves


## Tasks

- [x] **T01: File Picker Utilities & UI**
  - Create file picker utilities and UI elements for loading .cypcb and .ses files

Purpose: Enable users to load PCB designs without requiring a backend server
Output: file-picker.ts with utilities, Open button in toolbar, drag-over CSS
- [x] **T02: Viewer Integration & Drag-Drop**
  - Integrate file picker with viewer to load .cypcb and .ses files

Purpose: Wire up file selection and drag-drop to the existing PcbEngine
Output: Working file picker that loads boards and routes into the viewer
- [x] **T03: Human Verification**
  - Human verification of file picker functionality

Purpose: Confirm all file picker features work correctly in the browser
Output: Verified working file picker for Phase 8 completion

## Files Likely Touched

- `viewer/src/file-picker.ts`
- `viewer/index.html`
- `viewer/src/main.ts`
