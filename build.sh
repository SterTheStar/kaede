#!/bin/bash

# Exit on error
set -e

echo "--- Starting Kaede Build Process ---"

# 1. Create build directory for all artifacts
mkdir -p build
rm -rf build/*

# 2. Build Release Binary
echo "Building release binary..."
cargo build --release

# 3. Check for cargo-bundle
BUNDLED=false
if command -v cargo-bundle &> /dev/null; then
    echo "Building bundles (.deb, .rpm) using cargo-bundle..."
    cargo bundle --release
    
    # Copy produced bundles
    [ -d "target/release/bundle/deb" ] && cp target/release/bundle/deb/*.deb build/
    [ -d "target/release/bundle/rpm" ] && cp target/release/bundle/rpm/*.rpm build/
    [ -d "target/release/bundle/appimage" ] && cp target/release/bundle/appimage/*.AppImage build/
    BUNDLED=true
fi

# 4. Fallback if cargo-bundle not found or failed
if [ "$BUNDLED" = false ]; then
    echo "Notice: cargo-bundle not found. To install it, run: cargo install cargo-bundle"
    echo "Attempting fallback to cargo-deb/cargo-generate-rpm if available..."
    
    # Fallback to cargo-deb
    if command -v cargo-deb &> /dev/null; then
        echo "Building .deb package..."
        cargo deb
        cp target/debian/*.deb build/
    fi
    
    # Fallback to cargo-generate-rpm
    if command -v cargo-generate-rpm &> /dev/null; then
        echo "Building .rpm package..."
        cargo generate-rpm
        cp target/generate-rpm/*.rpm build/
    fi
fi

# 5. Build Arch Linux Package (makepkg)
if command -v makepkg &> /dev/null; then
    echo "Building Arch Linux package..."
    # Get version from Cargo.toml
    VERSION=$(grep -m 1 '^version =' Cargo.toml | cut -d '"' -f 2)
    TARBALL="kaede-${VERSION}.tar.gz"
    
    # Create the source tarball makepkg expects
    echo "Creating source tarball $TARBALL..."
    TMP_ARCHIVE=$(mktemp -d)
    # Exclude only build artifacts and temp directories, keep source tree
    tar --exclude="./target" --exclude="./build" --exclude="./.git" \
        --exclude="./pkg" --exclude="./$TARBALL" \
        -czf "$TMP_ARCHIVE/$TARBALL" .

    # Use a separate temporary directory for makepkg so its src/ doesn't touch our project src/
    ARCH_BUILD_DIR=$(mktemp -d)
    cp PKGBUILD "$ARCH_BUILD_DIR/"
    mv "$TMP_ARCHIVE/$TARBALL" "$ARCH_BUILD_DIR/"
    rmdir "$TMP_ARCHIVE"

    echo "Running makepkg in isolated dir: $ARCH_BUILD_DIR"
    pushd "$ARCH_BUILD_DIR" > /dev/null
    PKGEXT='.pkg.tar.zst'
    makepkg -f --noprogressbar --nodeps
    cp ./*.pkg.tar.zst "$OLDPWD/build/"
    popd > /dev/null

    # Cleanup temporary Arch build directory
    rm -rf "$ARCH_BUILD_DIR"
else
    echo "Warning: makepkg not found. Skipping Arch Linux build."
fi

# 6. Build NixOS Package
if command -v nix-build &> /dev/null; then
    echo "Building NixOS package (kaede-beta)..."
    nix-build default.nix -o result-nix
    cp -rL result-nix build/kaede-beta-nixos
    rm result-nix
else
    echo "Warning: nix-build not found. Skipping NixOS build."
fi

echo "--- Build Finished! Artifacts are in the 'build' directory ---"
ls -lh build/
