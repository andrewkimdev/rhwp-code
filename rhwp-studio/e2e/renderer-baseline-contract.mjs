export function inspectCanvasKitRuntimeImageFailures(runtime) {
  const imageFailures = runtime !== null
    && typeof runtime === 'object'
    && Array.isArray(runtime.imageFailures)
    ? runtime.imageFailures
    : null;
  return {
    available: imageFailures !== null,
    failures: imageFailures ?? [],
    hasFailures: (imageFailures?.length ?? 0) > 0,
  };
}
