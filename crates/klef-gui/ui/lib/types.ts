// Mirrors `klef_core::dto::KeyDto`. Kept in sync manually for now; if it
// drifts a future sprint will generate this from `cargo run --bin
// klef-gen-bindings` (out of scope for S2.2b).
export interface KeyDto {
  name: string;
  env_var: string;
  tags?: string[];
  note?: string;
  added_at: string;
  updated_at: string;
  /** RFC3339 timestamp; absent if the key has never been accessed via the GUI. */
  last_used_at?: string;
}
