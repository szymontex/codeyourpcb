import { describe, it, expect, vi, beforeEach } from 'vitest';
import { UndoStack, type BoardCommand } from '../undo';

/** Create a mock command that tracks execute/undo calls via a shared log array */
function mockCmd(name: string, log: string[]): BoardCommand {
  return {
    description: name,
    execute: vi.fn(() => log.push(`exec:${name}`)),
    undo: vi.fn(() => log.push(`undo:${name}`)),
  };
}

describe('UndoStack', () => {
  let stack: UndoStack;
  let log: string[];

  beforeEach(() => {
    stack = new UndoStack();
    log = [];
  });

  it('starts empty — cannot undo or redo', () => {
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);
    expect(stack.depth).toBe(0);
    expect(stack.lastCommand).toBeNull();
  });

  it('push executes the command and adds to stack', () => {
    const cmd = mockCmd('A', log);
    stack.push(cmd);

    expect(cmd.execute).toHaveBeenCalledOnce();
    expect(stack.canUndo).toBe(true);
    expect(stack.canRedo).toBe(false);
    expect(stack.depth).toBe(1);
    expect(stack.lastCommand).toBe('A');
    expect(log).toEqual(['exec:A']);
  });

  it('undo reverts the last command', () => {
    const cmd = mockCmd('A', log);
    stack.push(cmd);
    stack.undo();

    expect(cmd.undo).toHaveBeenCalledOnce();
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(true);
    expect(log).toEqual(['exec:A', 'undo:A']);
  });

  it('redo re-applies the undone command', () => {
    const cmd = mockCmd('A', log);
    stack.push(cmd);
    stack.undo();
    stack.redo();

    expect(cmd.execute).toHaveBeenCalledTimes(2); // once in push, once in redo
    expect(stack.canUndo).toBe(true);
    expect(stack.canRedo).toBe(false);
    expect(log).toEqual(['exec:A', 'undo:A', 'exec:A']);
  });

  it('undo past empty is a no-op', () => {
    stack.undo(); // nothing to undo
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);
    expect(log).toEqual([]);
  });

  it('redo past head is a no-op', () => {
    const cmd = mockCmd('A', log);
    stack.push(cmd);
    stack.redo(); // already at head, nothing to redo
    expect(cmd.execute).toHaveBeenCalledOnce(); // only the initial push
    expect(log).toEqual(['exec:A']);
  });

  it('push after undo clears the redo branch', () => {
    const cmdA = mockCmd('A', log);
    const cmdB = mockCmd('B', log);
    const cmdC = mockCmd('C', log);

    stack.push(cmdA);
    stack.push(cmdB);
    stack.undo(); // undo B, now B is in redo
    expect(stack.canRedo).toBe(true);

    stack.push(cmdC); // should clear B from redo
    expect(stack.canRedo).toBe(false);
    expect(stack.depth).toBe(2); // A, C (B was discarded)
    expect(stack.lastCommand).toBe('C');
  });

  it('capacity limit drops oldest commands', () => {
    // Push 105 commands — stack should keep max 100
    for (let i = 0; i < 105; i++) {
      stack.push(mockCmd(`cmd${i}`, log));
    }
    expect(stack.depth).toBe(100);
    // Last command should be cmd104
    expect(stack.lastCommand).toBe('cmd104');
    // Can still undo
    expect(stack.canUndo).toBe(true);
  });

  it('multi-step undo/redo sequence', () => {
    const cmdA = mockCmd('A', log);
    const cmdB = mockCmd('B', log);
    const cmdC = mockCmd('C', log);

    stack.push(cmdA);
    stack.push(cmdB);
    stack.push(cmdC);

    stack.undo(); // undo C
    stack.undo(); // undo B
    expect(stack.lastCommand).toBe('A');
    expect(stack.canRedo).toBe(true);

    stack.redo(); // redo B
    expect(stack.lastCommand).toBe('B');

    expect(log).toEqual([
      'exec:A', 'exec:B', 'exec:C',
      'undo:C', 'undo:B',
      'exec:B',
    ]);
  });

  it('clear empties the entire stack', () => {
    stack.push(mockCmd('A', log));
    stack.push(mockCmd('B', log));
    stack.clear();

    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(false);
    expect(stack.depth).toBe(0);
    expect(stack.lastCommand).toBeNull();
  });
});
