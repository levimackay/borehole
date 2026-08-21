/**
 * Tauri's invoke() rejects with whatever the Rust side serialized as its
 * error — today that's almost always a plain string, but be defensive.
 * Never swallow this: every caller surfaces it verbatim to the user.
 */
export function describeError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  if (err && typeof err === "object") {
    try {
      return JSON.stringify(err);
    } catch {
      // fall through
    }
  }
  return String(err);
}
