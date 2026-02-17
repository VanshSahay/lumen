/**
 * # lumen-react
 *
 * React hooks for the Lumen trustless Ethereum light client.
 *
 * @example
 * ```tsx
 * import { useLumen } from 'lumen-react'
 *
 * function App() {
 *   const { provider, syncState, isReady } = useLumen()
 *   // ...
 * }
 * ```
 *
 * @module
 */

export { useLumen } from './useLumen';
export type { UseLumenResult } from './useLumen';
