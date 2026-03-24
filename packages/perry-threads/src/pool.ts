import type { WorkerRequest, WorkerResponse } from './types';

// Worker script source — executed inside each Web Worker.
// Self-contained: no imports, no outer scope references.
const WORKER_SCRIPT = `
"use strict";
self.onmessage = function(e) {
  var msg = e.data;
  try {
    var fn = new Function("return " + msg.fn)();
    var ctx = msg.context;
    if (msg.type === "map") {
      var results = [];
      var chunk = msg.chunk;
      for (var i = 0; i < chunk.length; i++) {
        results.push(ctx !== undefined ? fn(chunk[i], ctx) : fn(chunk[i]));
      }
      self.postMessage({ type: "result", data: results });
    } else if (msg.type === "filter") {
      var filtered = [];
      var chunk = msg.chunk;
      for (var i = 0; i < chunk.length; i++) {
        if (ctx !== undefined ? fn(chunk[i], ctx) : fn(chunk[i])) {
          filtered.push(chunk[i]);
        }
      }
      self.postMessage({ type: "result", data: filtered });
    } else if (msg.type === "exec") {
      var result = ctx !== undefined ? fn(ctx) : fn();
      self.postMessage({ type: "result", data: result });
    }
  } catch (err) {
    self.postMessage({ type: "error", message: String(err), stack: err && err.stack });
  }
};
`;

interface PooledWorker {
  worker: Worker;
  busy: boolean;
}

let pool: PooledWorker[] | null = null;
let blobUrl: string | null = null;

function getDefaultConcurrency(): number {
  if (typeof navigator !== 'undefined' && navigator.hardwareConcurrency) {
    return navigator.hardwareConcurrency;
  }
  // Node.js fallback
  try {
    return require('os').cpus().length;
  } catch {
    return 4;
  }
}

function ensurePool(size: number): PooledWorker[] {
  if (pool && pool.length >= size) return pool;

  if (!blobUrl) {
    const blob = new Blob([WORKER_SCRIPT], { type: 'text/javascript' });
    blobUrl = URL.createObjectURL(blob);
  }

  pool = pool || [];
  while (pool.length < size) {
    pool.push({ worker: new Worker(blobUrl), busy: false });
  }
  return pool;
}

/** Returns true if Web Workers are available in this environment. */
export function hasWorkerSupport(): boolean {
  return typeof Worker !== 'undefined';
}

/** Get the default concurrency level for this environment. */
export { getDefaultConcurrency };

/**
 * Send a task to a specific worker and return a promise for the result.
 */
export function dispatch(worker: Worker, request: WorkerRequest): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const handler = (e: MessageEvent<WorkerResponse>) => {
      worker.removeEventListener('message', handler);
      worker.removeEventListener('error', errorHandler);
      if (e.data.type === 'error') {
        const err = new Error(e.data.message || 'Worker error');
        if (e.data.stack) (err as any).workerStack = e.data.stack;
        reject(err);
      } else {
        resolve(e.data.data);
      }
    };
    const errorHandler = (e: ErrorEvent) => {
      worker.removeEventListener('message', handler);
      worker.removeEventListener('error', errorHandler);
      reject(new Error(e.message || 'Worker error'));
    };
    worker.addEventListener('message', handler);
    worker.addEventListener('error', errorHandler);
    worker.postMessage(request);
  });
}

/**
 * Distribute chunks across the worker pool and collect results.
 */
export async function distributeChunks<T>(
  chunks: T[][],
  fn: string,
  type: 'map' | 'filter',
  context?: unknown,
): Promise<T[][]> {
  const workers = ensurePool(chunks.length);
  const promises: Promise<unknown>[] = [];

  for (let i = 0; i < chunks.length; i++) {
    const w = workers[i % workers.length];
    promises.push(dispatch(w.worker, { type, chunk: chunks[i], fn, context }));
  }

  return (await Promise.all(promises)) as T[][];
}

/**
 * Run a single function on a worker.
 */
export async function dispatchExec(fn: string, context?: unknown): Promise<unknown> {
  const workers = ensurePool(1);
  return dispatch(workers[0].worker, { type: 'exec', fn, context });
}
