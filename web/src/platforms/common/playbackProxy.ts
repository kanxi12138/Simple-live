import { invoke } from '../../services/platformInvoke';

let operations: Promise<unknown> = Promise.resolve();

/**
 * Serializes mutations of the app's single playback proxy across player instances.
 * @param operation A stream-store or proxy operation.
 * @returns The operation result.
 * @throws The original IPC error; subsequent cleanup remains available.
 */
export const queueStreamProxy = <Result>(operation: () => Promise<Result>): Promise<Result> => {
  const pending = operations.then(operation, operation);
  operations = pending.catch(() => undefined);
  return pending;
};

/** Stops playback forwarding and clears its state; propagates IPC errors. */
export const stopStreamProxy = (): Promise<void> => queueStreamProxy(() => invoke<void>('stop_proxy'));
