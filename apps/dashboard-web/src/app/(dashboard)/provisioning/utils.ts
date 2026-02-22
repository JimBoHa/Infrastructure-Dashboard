export const isHex = (value: string) => /^[0-9a-fA-F]+$/.test(value);

export const randomHex = (bytes: number) => {
  const cryptoObj = globalThis.crypto;
  if (cryptoObj?.getRandomValues) {
    const buf = new Uint8Array(bytes);
    cryptoObj.getRandomValues(buf);
    return Array.from(buf)
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
  }
  return Array.from({ length: bytes * 2 })
    .map(() => Math.floor(Math.random() * 16).toString(16))
    .join("");
};

export const parseOptionalNumber = (value: string) => {
  if (!value.trim()) return null;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return null;
  return parsed;
};
