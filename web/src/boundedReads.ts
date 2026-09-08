/** Bound read-only fan-out; cancellation/failure never admits the remaining queue. */
export async function boundedReads<T, R>(items: readonly T[], concurrency: number, read: (item: T) => Promise<R>, signal: AbortSignal): Promise<R[]> {
  if (!Number.isInteger(concurrency) || concurrency < 1 || concurrency > 32) {
    throw new RangeError("Read concurrency must be an integer between 1 and 32");
  }
  const checkSignal = () => {
    if (signal.aborted) throw new DOMException("Context read cancelled", "AbortError");
  };
  checkSignal();
  const values: R[] = new Array(items.length);
  let next = 0, failed = false;
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, async () => {
    while (!failed && next < items.length) {
      checkSignal();
      const index = next++;
      try { values[index] = await read(items[index]); }
      catch (error) { failed = true; throw error; }
    }
  }));
  checkSignal();
  return values;
}
