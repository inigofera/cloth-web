/**
 * Undo manager for reversible user actions.
 */

export interface UndoAction {
  type: 'delete' | 'deactivate' | 'custom';
  itemId?: string;
  name?: string;
  timestamp: number;
  undo: () => Promise<void> | void;
}

export class UndoManager {
  private actions: UndoAction[] = [];
  private readonly MAX_UNDOS = 5;

  registerUndo(action: Omit<UndoAction, 'timestamp'>) {
    this.actions.push({ ...action, timestamp: Date.now() });
    if (this.actions.length > this.MAX_UNDOS) {
      this.actions.shift();
    }
  }

  async executeUndo(index?: number): Promise<void> {
    const idx = index ?? this.actions.length - 1;
    const action = this.actions[idx];
    if (!action) return;
    this.actions.splice(idx, 1);
    await action.undo();
  }

  getPending(): UndoAction | null {
    return this.actions[this.actions.length - 1] ?? null;
  }

  clear() {
    this.actions = [];
  }
}

export const undoManager = new UndoManager();