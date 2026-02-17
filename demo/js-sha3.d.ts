declare module 'js-sha3' {
  interface Hasher {
    (message: string | ArrayBuffer | Uint8Array): string;
    arrayBuffer(message: string | ArrayBuffer | Uint8Array): ArrayBuffer;
    digest(message: string | ArrayBuffer | Uint8Array): number[];
    hex(message: string | ArrayBuffer | Uint8Array): string;
    update(message: string | ArrayBuffer | Uint8Array): Hasher;
  }

  export const keccak256: Hasher;
  export const keccak_256: Hasher;
}
