import type { ThreadOptions } from './types';
import { hasWorkerSupport, getDefaultConcurrency, distributeChunks, dispatchExec } from './pool';

/** Minimum array length before we bother with workers. Below this, inline is faster. */
const MIN_PARALLEL_SIZE = 1024;

/**
 * Apply `fn` to every element of `data` in parallel across Web Workers.
 *
 * Functions are serialized via `.toString()` — they must be self-contained
 * (no references to outer scope). Use `options.context` to pass data in.
 *
 * Returns a Promise that resolves to the mapped array in original order.
 */
export async function parallelMap<T, U, C = unknown>(
  data: T[],
  fn: (item: T, context: C) => U,
  options?: ThreadOptions<C>,
): Promise<U[]> {
  if (data.length === 0) return [];

  const concurrency = options?.concurrency ?? getDefaultConcurrency();

  // Fast path: small arrays or no Worker support — run inline
  if (!hasWorkerSupport() || data.length < MIN_PARALLEL_SIZE || concurrency <= 1) {
    const ctx = options?.context as C;
    return data.map(item => fn(item, ctx));
  }

  const fnStr = fn.toString();
  const numChunks = Math.min(concurrency, data.length);
  const chunkSize = Math.ceil(data.length / numChunks);
  const chunks: T[][] = [];
  for (let i = 0; i < data.length; i += chunkSize) {
    chunks.push(data.slice(i, i + chunkSize));
  }

  const results = await distributeChunks(chunks, fnStr, 'map', options?.context);

  // Flatten chunks back into a single array, preserving order
  const out: U[] = [];
  for (const chunk of results) {
    for (const item of chunk) {
      out.push(item as unknown as U);
    }
  }
  return out;
}

/**
 * Filter `data` in parallel across Web Workers, keeping elements where `fn` returns truthy.
 *
 * Functions are serialized via `.toString()` — they must be self-contained.
 * Use `options.context` to pass data in.
 *
 * Returns a Promise that resolves to the filtered array, preserving original order.
 */
export async function parallelFilter<T, C = unknown>(
  data: T[],
  fn: (item: T, context: C) => boolean,
  options?: ThreadOptions<C>,
): Promise<T[]> {
  if (data.length === 0) return [];

  const concurrency = options?.concurrency ?? getDefaultConcurrency();

  if (!hasWorkerSupport() || data.length < MIN_PARALLEL_SIZE || concurrency <= 1) {
    const ctx = options?.context as C;
    return data.filter(item => fn(item, ctx));
  }

  const fnStr = fn.toString();
  const numChunks = Math.min(concurrency, data.length);
  const chunkSize = Math.ceil(data.length / numChunks);
  const chunks: T[][] = [];
  for (let i = 0; i < data.length; i += chunkSize) {
    chunks.push(data.slice(i, i + chunkSize));
  }

  const results = await distributeChunks(chunks, fnStr, 'filter', options?.context);

  // Flatten filtered chunks, preserving order
  const out: T[] = [];
  for (const chunk of results) {
    for (const item of chunk) {
      out.push(item as unknown as T);
    }
  }
  return out;
}

/**
 * Run `fn` on a background Web Worker and return a Promise for the result.
 *
 * The function is serialized via `.toString()` — it must be self-contained.
 * Use `options.context` to pass data in.
 */
export async function spawn<T, C = unknown>(
  fn: (context: C) => T,
  options?: Omit<ThreadOptions<C>, 'concurrency'>,
): Promise<T> {
  if (!hasWorkerSupport()) {
    return fn(options?.context as C);
  }

  const result = await dispatchExec(fn.toString(), options?.context);
  return result as T;
}
