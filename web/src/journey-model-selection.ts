export const MAX_JOURNEY_MODELS = 32;

/** A large registry is not permission to pick an arbitrary subset for data sharing. */
export function journeyModelSelection(available: string[], selected?: string[]): string[] {
  const unique = [...new Set(available)];
  if (selected) return [...new Set(selected)].filter(id => unique.includes(id));
  return unique.length <= MAX_JOURNEY_MODELS ? unique : [];
}
