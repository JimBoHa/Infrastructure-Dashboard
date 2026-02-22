export type MapPoint = { lng: number; lat: number };

export const pointFromGeometry = (geometry: unknown): MapPoint | null => {
  if (!geometry || typeof geometry !== "object") return null;
  const record = geometry as Record<string, unknown>;
  if (record.type !== "Point") return null;
  const coords = record.coordinates;
  if (!Array.isArray(coords) || coords.length < 2) return null;
  const lng = coords[0];
  const lat = coords[1];
  if (typeof lng !== "number" || typeof lat !== "number") return null;
  return { lng, lat };
};
