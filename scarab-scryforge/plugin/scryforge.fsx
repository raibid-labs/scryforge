// @name scryforge
// @version 0.1.0
// @description Scryforge integration - unified feed reader status and controls
// @author raibid-labs
// @api-version 0.1.0
// @min-scarab-version 0.1.0

// =============================================================================
// scarab-scryforge Phase A (mirrored from scarab repo, issue #253)
// =============================================================================
//
// This is the Fusabi-language version of the scarab-scryforge plugin, tracked
// upstream at https://github.com/raibid-labs/scarab/issues/253. The canonical
// copy lives in `examples/fusabi/scryforge.fsx` inside the scarab repo; this
// copy is mirrored here so the scryforge project's own Cargo workspace can
// ship the plugin script alongside the daemon.
//
// Phase A only validates the .fsx -> .fzb -> daemon load path. Real
// status-bar text "📬 N unread" is currently emitted from the scarab daemon
// side as a Phase A bridge once it observes the scryforge plugin in the
// registry (hardcoded to 3 unread). See `crates/scarab-daemon/src/main.rs`
// in the scarab repo.
//
// Compared to the existing Rust plugin in `../src/lib.rs` (ScryforgePlugin),
// this .fsx version is intentionally minimal:
//   * No JSON-RPC client to scryforge-daemon (Phase B - depends on
//     fusabi-stdlib-ext exposing a synchronous net_http binding usable from
//     a hook).
//   * No 30s polling loop (Phase B - moves into a host-side timer or the
//     daemon adapter once timer bindings exist).
//   * No menu (Phase C - depends on .fzb adapter registering host callbacks
//     for scarab-plugin-api menu primitives).
//   * No focusables (Phase C).
//
// The Rust ScryforgePlugin remains the production integration; once the
// VM gains the missing host bindings, this .fsx grows to cover the same
// surface and the Rust crate becomes a thin shell.
// =============================================================================

// Hardcoded Phase A unread count. Replaced by JSON-RPC scryforge.unread_count
// in Phase B.
let unread_count = 3 in

// Plugin lifecycle: on_load is the single hook the bytecode adapter looks up.
// We give it a trivial body so the function exists in vm.globals and the
// daemon's call_hook_function returns Some(Value::Unit).
let on_load = fun _u -> () in

// Final expression: keep it cheap so the chunk has at least one runtime
// instruction. Returning the unread_count means the program "value" is
// observable in tests if we ever wire vm.execute output through.
unread_count
