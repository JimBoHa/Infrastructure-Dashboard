export interface BinaryMetricsSeries {
  sensor_id: string;
  sensor_name: string | null;
  base_timestamp_ms: number;
  point_count: number;
  data_offset: number;
  data_buffer: ArrayBuffer;
}

const MAGIC = 0x31424446; // "FDB1" as u32 LE

export function decodeBinaryMetrics(buffer: ArrayBuffer): BinaryMetricsSeries[] {
  const view = new DataView(buffer);

  if (buffer.byteLength < 10) {
    throw new Error("Binary metrics buffer too short");
  }

  const magic = view.getUint32(0, true);
  if (magic !== MAGIC) {
    throw new Error(
      `Invalid binary metrics magic: expected FDB1, got 0x${magic.toString(16)}`,
    );
  }

  const seriesCount = view.getUint16(4, true);
  // total_point_count at bytes 6..9 (used for validation, not needed for decode)

  const result: BinaryMetricsSeries[] = [];
  let pos = 10;

  // Parse headers
  for (let i = 0; i < seriesCount; i++) {
    // sensor_id
    const idLen = view.getUint16(pos, true);
    pos += 2;
    const idBytes = new Uint8Array(buffer, pos, idLen);
    const sensor_id = new TextDecoder().decode(idBytes);
    pos += idLen;

    // sensor_name
    const nameLen = view.getUint16(pos, true);
    pos += 2;
    let sensor_name: string | null = null;
    if (nameLen > 0) {
      const nameBytes = new Uint8Array(buffer, pos, nameLen);
      sensor_name = new TextDecoder().decode(nameBytes);
      pos += nameLen;
    }

    // point_count
    const point_count = view.getUint32(pos, true);
    pos += 4;

    // base_timestamp_ms
    const base_timestamp_ms = view.getFloat64(pos, true);
    pos += 8;

    result.push({
      sensor_id,
      sensor_name,
      base_timestamp_ms,
      point_count,
      data_offset: 0, // filled in below
      data_buffer: buffer,
    });
  }

  // Assign data offsets (bulk data follows all headers sequentially)
  for (const series of result) {
    series.data_offset = pos;
    pos += series.point_count * 8;
  }

  return result;
}

/**
 * Read a single point from a decoded binary series.
 * Returns [timestamp_ms, value].
 */
export function readBinaryPoint(
  series: BinaryMetricsSeries,
  index: number,
): [number, number] {
  const view = new DataView(series.data_buffer);
  const byteOffset = series.data_offset + index * 8;
  const offsetSeconds = view.getUint32(byteOffset, true);
  const value = view.getFloat32(byteOffset + 4, true);
  return [series.base_timestamp_ms + offsetSeconds * 1000, value];
}
