/** Options for parallel operations. */
export interface ThreadOptions<C = unknown> {
  /** Data to make available inside the worker function as the second argument. Structured-cloned to each worker. */
  context?: C;
  /** Number of workers to use. Defaults to navigator.hardwareConcurrency or os.cpus().length. */
  concurrency?: number;
}

/** Message sent from main thread to worker. */
export interface WorkerRequest {
  type: 'map' | 'filter' | 'exec';
  chunk?: unknown[];
  fn: string;
  context?: unknown;
}

/** Message sent from worker back to main thread. */
export interface WorkerResponse {
  type: 'result' | 'error';
  data?: unknown;
  message?: string;
  stack?: string;
}
