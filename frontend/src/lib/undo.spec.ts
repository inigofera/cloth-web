import { describe, it, expect, vi } from 'vitest';
import { UndoManager } from './undo';

describe('UndoManager', () => {
  it('registers and retrieves pending action', () => {
    const manager = new UndoManager();
    const fn = vi.fn();
    manager.registerUndo({
      type: 'deactivate',
      itemId: 'item-1',
      name: 'Blue Jeans',
      undo: fn
    });

    const pending = manager.getPending();
    expect(pending).not.toBeNull();
    expect(pending?.name).toBe('Blue Jeans');
    expect(pending?.type).toBe('deactivate');
  });

  it('executes the most recent undo and clears it', async () => {
    const manager = new UndoManager();
    const fn1 = vi.fn();
    const fn2 = vi.fn();

    manager.registerUndo({ type: 'deactivate', itemId: '1', undo: fn1 });
    manager.registerUndo({ type: 'delete', itemId: '2', undo: fn2 });

    await manager.executeUndo();

    expect(fn2).toHaveBeenCalledOnce();
    expect(fn1).not.toHaveBeenCalled();
    expect(manager.getPending()?.itemId).toBe('1');
  });

  it('caps at MAX_UNDOS', () => {
    const manager = new UndoManager();
    for (let i = 0; i < 10; i++) {
      manager.registerUndo({ type: 'custom', undo: vi.fn(), itemId: `id-${i}` });
    }

    expect(manager.getPending()?.itemId).toBe('id-9');
  });
});
