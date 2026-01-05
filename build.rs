// build.rs - Build script for i18next-turbo
//
// This build script is used for NAPI (Node-API) integration.
// It sets up the build environment for creating Node.js native addons.

fn main() {
    napi_build::setup();
}
