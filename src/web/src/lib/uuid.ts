export function uuid() {
  const cryptoAny = (globalThis as { crypto?: Crypto }).crypto;

  if (cryptoAny && typeof cryptoAny.randomUUID === "function") {
    return cryptoAny.randomUUID();
  }

  if (cryptoAny && typeof cryptoAny.getRandomValues === "function") {
    const bytes = new Uint8Array(16);
    cryptoAny.getRandomValues(bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0"));
    return [
      hex.slice(0, 4).join(""),
      hex.slice(4, 6).join(""),
      hex.slice(6, 8).join(""),
      hex.slice(8, 10).join(""),
      hex.slice(10, 16).join(""),
    ].join("-");
  }

  const random = () => Math.floor(Math.random() * 0xffffffff).toString(16).padStart(8, "0");
  return `${random()}-${random().slice(0, 4)}-4${random().slice(1, 4)}-8${random().slice(1, 4)}-${random()}${random().slice(0, 4)}`;
}
