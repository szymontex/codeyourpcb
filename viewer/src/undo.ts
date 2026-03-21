/**
 * Undo/Redo system using the command pattern.
 *
 * Each board mutation is wrapped in a BoardCommand that knows how to
 * execute and reverse itself. The UndoStack manages a linear history
 * with a maximum depth of 100.
 *
 * Debug surface: window.__undoStack exposes { canUndo, canRedo, depth, lastCommand }
 */

import type { PcbEngine } from './wasm';

// ---------------------------------------------------------------------------
// Command interface
// ---------------------------------------------------------------------------

export interface BoardCommand {
  /** Human-readable description for logs and debug */
  description: string;
  /** Perform the mutation */
  execute(): void;
  /** Reverse the mutation */
  undo(): void;
}

// ---------------------------------------------------------------------------
// UndoStack
// ---------------------------------------------------------------------------

const MAX_DEPTH = 100;

export class UndoStack {
  private history: BoardCommand[] = [];
  /** Points to the next slot to write into — commands before this index are undoable */
  private cursor = 0;

  get canUndo(): boolean {
    return this.cursor > 0;
  }

  get canRedo(): boolean {
    return this.cursor < this.history.length;
  }

  get depth(): number {
    return this.history.length;
  }

  get lastCommand(): string | null {
    if (this.cursor === 0) return null;
    return this.history[this.cursor - 1].description;
  }

  /**
   * Execute a command and push it onto the stack.
   * Clears any redo history beyond the current cursor.
   */
  push(cmd: BoardCommand): void {
    cmd.execute();
    console.log(`[Undo] Execute: ${cmd.description}`);

    // Truncate redo tail
    this.history.length = this.cursor;

    this.history.push(cmd);
    this.cursor++;

    // Enforce max depth — drop oldest
    if (this.history.length > MAX_DEPTH) {
      this.history.shift();
      this.cursor--;
    }
  }

  /**
   * Undo the last executed command. No-op if nothing to undo.
   */
  undo(): void {
    if (!this.canUndo) {
      console.warn('[Undo] Nothing to undo');
      return;
    }
    this.cursor--;
    const cmd = this.history[this.cursor];
    cmd.undo();
    console.log(`[Undo] Undo: ${cmd.description}`);
  }

  /**
   * Redo the last undone command. No-op if nothing to redo.
   */
  redo(): void {
    if (!this.canRedo) {
      console.warn('[Undo] Nothing to redo');
      return;
    }
    const cmd = this.history[this.cursor];
    cmd.execute();
    console.log(`[Undo] Redo: ${cmd.description}`);
    this.cursor++;
  }

  /**
   * Clear all history (e.g. on file load).
   */
  clear(): void {
    this.history.length = 0;
    this.cursor = 0;
    console.log('[Undo] Stack cleared');
  }
}

// ---------------------------------------------------------------------------
// Trace commands
// ---------------------------------------------------------------------------

export interface AddTraceArgs {
  netName: string;
  layer: string;
  width: number;
  /** Flat segment array [x1,y1,x2,y2, ...] */
  segments: number[];
}

/**
 * Command: add a trace to the board.
 *
 * On execute, calls engine.add_trace and stores the returned ID.
 * On undo, calls engine.remove_trace with that ID.
 */
export class AddTraceCommand implements BoardCommand {
  description: string;
  private traceId: number = 0xFFFFFFFF;

  constructor(
    private engine: PcbEngine,
    private args: AddTraceArgs,
    private refreshSnapshot: () => void,
  ) {
    this.description = `Add trace: ${args.netName} on ${args.layer}`;
  }

  execute(): void {
    this.traceId = this.engine.add_trace(
      this.args.netName,
      this.args.layer,
      this.args.width,
      this.args.segments,
    );
    if (this.traceId === 0xFFFFFFFF) {
      console.warn(`[Undo] AddTraceCommand: engine.add_trace failed for net=${this.args.netName}`);
    }
    this.engine.run_drc_incremental();
    this.refreshSnapshot();
  }

  undo(): void {
    if (this.traceId !== 0xFFFFFFFF) {
      this.engine.remove_trace(this.traceId);
      this.engine.run_drc_incremental();
    }
    this.refreshSnapshot();
  }
}

/**
 * Command: remove a trace from the board.
 *
 * On execute, removes the trace.
 * On undo, re-adds it with the original parameters.
 */
export class RemoveTraceCommand implements BoardCommand {
  description: string;
  /** Re-created trace ID after undo (may differ from original) */
  private restoredId: number = 0xFFFFFFFF;

  constructor(
    private engine: PcbEngine,
    private traceId: number,
    private args: AddTraceArgs,
    private refreshSnapshot: () => void,
  ) {
    this.description = `Remove trace: ${args.netName} (id=${traceId})`;
  }

  execute(): void {
    this.engine.remove_trace(this.traceId);
    this.engine.run_drc_incremental();
    this.refreshSnapshot();
  }

  undo(): void {
    this.restoredId = this.engine.add_trace(
      this.args.netName,
      this.args.layer,
      this.args.width,
      this.args.segments,
    );
    if (this.restoredId === 0xFFFFFFFF) {
      console.warn(`[Undo] RemoveTraceCommand.undo: re-add failed for net=${this.args.netName}`);
    } else {
      // Update trace ID so future execute() uses the new ID
      this.traceId = this.restoredId;
    }
    this.engine.run_drc_incremental();
    this.refreshSnapshot();
  }
}

// ---------------------------------------------------------------------------
// Rotation command
// ---------------------------------------------------------------------------

/**
 * Command: rotate a component by delta millidegrees.
 *
 * On execute, rotates by +delta. On undo, rotates by -delta.
 */
export class RotateComponentCommand implements BoardCommand {
  description: string;

  constructor(
    private engine: PcbEngine,
    private refdes: string,
    private deltaMdeg: number,
    private refreshSnapshot: () => void,
  ) {
    const degrees = deltaMdeg / 1000;
    const sign = degrees > 0 ? '+' : '';
    this.description = `Rotate ${refdes} ${sign}${degrees}°`;
  }

  execute(): void {
    const ok = this.engine.rotate_component(this.refdes, this.deltaMdeg);
    if (!ok) {
      console.warn(`[Undo] RotateComponentCommand: ${this.refdes} not found`);
    }
    this.refreshSnapshot();
  }

  undo(): void {
    const ok = this.engine.rotate_component(this.refdes, -this.deltaMdeg);
    if (!ok) {
      console.warn(`[Undo] RotateComponentCommand.undo: ${this.refdes} not found`);
    }
    this.refreshSnapshot();
  }
}

// ---------------------------------------------------------------------------
// Board resize command
// ---------------------------------------------------------------------------

/**
 * Command: resize the board outline.
 *
 * Stores old and new dimensions. Execute sets new, undo restores old.
 */
export class ResizeBoardCommand implements BoardCommand {
  description: string;

  constructor(
    private engine: PcbEngine,
    private oldWidth: number,
    private oldHeight: number,
    private newWidth: number,
    private newHeight: number,
    private refreshSnapshot: () => void,
  ) {
    const owMm = (oldWidth / 1e6).toFixed(1);
    const ohMm = (oldHeight / 1e6).toFixed(1);
    const nwMm = (newWidth / 1e6).toFixed(1);
    const nhMm = (newHeight / 1e6).toFixed(1);
    this.description = `Resize board: ${owMm}×${ohMm}mm → ${nwMm}×${nhMm}mm`;
  }

  execute(): void {
    const ok = this.engine.set_board_size(this.newWidth, this.newHeight);
    if (!ok) {
      console.warn('[Undo] ResizeBoardCommand: set_board_size failed');
    }
    this.refreshSnapshot();
  }

  undo(): void {
    const ok = this.engine.set_board_size(this.oldWidth, this.oldHeight);
    if (!ok) {
      console.warn('[Undo] ResizeBoardCommand.undo: set_board_size failed');
    }
    this.refreshSnapshot();
  }
}

// ---------------------------------------------------------------------------
// Edit trace command (segment/corner drag)
// ---------------------------------------------------------------------------

/**
 * Command: edit a trace's geometry (drag segment or corner).
 *
 * Implemented as remove-old + add-new. On undo, removes the new trace
 * and re-adds the old one.
 */
export class EditTraceCommand implements BoardCommand {
  description: string;
  private newTraceId: number = 0xFFFFFFFF;

  constructor(
    private engine: PcbEngine,
    private oldTraceId: number,
    private oldArgs: AddTraceArgs,
    private newArgs: AddTraceArgs,
    private refreshSnapshot: () => void,
  ) {
    this.description = `Edit trace: ${oldArgs.netName} (id=${oldTraceId})`;
  }

  execute(): void {
    this.engine.remove_trace(this.oldTraceId);
    this.newTraceId = this.engine.add_trace(
      this.newArgs.netName,
      this.newArgs.layer,
      this.newArgs.width,
      this.newArgs.segments,
    );
    if (this.newTraceId === 0xFFFFFFFF) {
      console.warn(`[Undo] EditTraceCommand: add_trace failed for net=${this.newArgs.netName}`);
    }
    this.engine.run_drc_incremental();
    this.refreshSnapshot();
  }

  undo(): void {
    if (this.newTraceId !== 0xFFFFFFFF) {
      this.engine.remove_trace(this.newTraceId);
    }
    this.oldTraceId = this.engine.add_trace(
      this.oldArgs.netName,
      this.oldArgs.layer,
      this.oldArgs.width,
      this.oldArgs.segments,
    );
    if (this.oldTraceId === 0xFFFFFFFF) {
      console.warn(`[Undo] EditTraceCommand.undo: re-add failed for net=${this.oldArgs.netName}`);
    }
    this.engine.run_drc_incremental();
    this.refreshSnapshot();
  }
}

// ---------------------------------------------------------------------------
// Debug surface
// ---------------------------------------------------------------------------

/**
 * Install the debug surface on window.__undoStack.
 */
export function installDebugSurface(stack: UndoStack): void {
  (window as any).__undoStack = {
    get canUndo() { return stack.canUndo; },
    get canRedo() { return stack.canRedo; },
    get depth() { return stack.depth; },
    get lastCommand() { return stack.lastCommand; },
  };
}
