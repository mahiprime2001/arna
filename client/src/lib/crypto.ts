// End-to-end encryption for chat (NaCl box: X25519 + XSalsa20-Poly1305).
// The private key never leaves this device. The server only ever sees the
// public key and ciphertext.
import nacl from "tweetnacl";
import util from "tweetnacl-util";

let kp: nacl.BoxKeyPair | null = null;

// Load (or create) the keypair for this account, kept per-user in localStorage.
export function initCrypto(uid: number) {
  const key = `arna_kp_${uid}`;
  const raw = localStorage.getItem(key);
  if (raw) {
    try {
      const j = JSON.parse(raw);
      kp = {
        publicKey: util.decodeBase64(j.pub),
        secretKey: util.decodeBase64(j.sec),
      };
      return;
    } catch {
      // fall through and regenerate
    }
  }
  const fresh = nacl.box.keyPair();
  localStorage.setItem(
    key,
    JSON.stringify({
      pub: util.encodeBase64(fresh.publicKey),
      sec: util.encodeBase64(fresh.secretKey),
    }),
  );
  kp = fresh;
}

export function myPublicKey(): string {
  if (!kp) throw new Error("crypto not initialized");
  return util.encodeBase64(kp.publicKey);
}

export function encryptFor(
  theirPub: string,
  plaintext: string,
): { nonce: string; ciphertext: string } {
  if (!kp) throw new Error("crypto not initialized");
  const nonce = nacl.randomBytes(nacl.box.nonceLength);
  const box = nacl.box(
    util.decodeUTF8(plaintext),
    nonce,
    util.decodeBase64(theirPub),
    kp.secretKey,
  );
  return { nonce: util.encodeBase64(nonce), ciphertext: util.encodeBase64(box) };
}

export function decryptFrom(
  theirPub: string,
  nonce: string,
  ciphertext: string,
): string | null {
  if (!kp) return null;
  try {
    const opened = nacl.box.open(
      util.decodeBase64(ciphertext),
      util.decodeBase64(nonce),
      util.decodeBase64(theirPub),
      kp.secretKey,
    );
    return opened ? util.encodeUTF8(opened) : null;
  } catch {
    return null;
  }
}
