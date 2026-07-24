/// <reference types="vite/client" />

// Build-time config. All optional -- the app falls back to same-origin and to
// plain STUN when these are unset.
interface ImportMetaEnv {
  /** Base URL of the backend. Empty/unset means same origin. */
  readonly VITE_ARNA_API?: string;
  /** TURN relay, for calls between networks that block direct connections. */
  readonly VITE_ARNA_TURN_URL?: string;
  readonly VITE_ARNA_TURN_USERNAME?: string;
  readonly VITE_ARNA_TURN_CREDENTIAL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
