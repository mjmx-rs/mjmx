//! Build script for the N-API addon.
//!
//! `napi_build::setup()` emits the platform-specific linker flags a Node native
//! addon needs (notably `-undefined dynamic_lookup` style handling, so the
//! `napi_*` symbols provided by the host Node process are resolved at load time
//! rather than at link time).

fn main() {
    napi_build::setup();
}
