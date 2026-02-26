#!/bin/bash

# Exit immediately if any command fails
set -e

echo "--- Starting Kaede build process ---"

# Ensure the output directory exists and is clean
mkdir -p build
rm -rf build/*

# --------------------------------------------------
# 1. Build release binary
# --------------------------------------------------
echo "Building release binary..."
cargo build --release

# --------------------------------------------------
# 2. Attempt to build distribution bundles via cargo-bundle
# --------------------------------------------------
BUNDLED=false

if command -v cargo-bundle &> /dev/null; then
    echo "cargo-bundle detected. Building distribution packages..."
    cargo bundle --release

    # Copy generated artifacts if present
    [ -d "target/release/bundle/deb" ] && cp target/release/bundle/deb/*.deb build/ || true
    [ -d "target/release/bundle/rpm" ] && cp target/release/bundle/rpm/*.rpm build/ || true
    [ -d "target/release/bundle/appimage" ] && cp target/release/bundle/appimage/*.AppImage build/ || true

    BUNDLED=true
fi

# --------------------------------------------------
# 3. Fallback to alternative packaging tools
# --------------------------------------------------
if [ "$BUNDLED" = false ]; then
    echo "cargo-bundle not available."
    echo "Install with: cargo install cargo-bundle"
    echo "Attempting fallback packaging methods..."

    # Debian package
    if command -v cargo-deb &> /dev/null; then
        echo "Building Debian package (.deb)..."
        cargo deb
        cp target/debian/*.deb build/ || true
    fi

    # RPM package
    if command -v cargo-generate-rpm &> /dev/null; then
        echo "Building RPM package (.rpm)..."
        cargo generate-rpm
        cp target/generate-rpm/*.rpm build/ || true
    fi
fi

# --------------------------------------------------
# 4. Build Arch Linux package using makepkg
# --------------------------------------------------
if command -v makepkg &> /dev/null; then
    echo "Building Arch Linux package..."

    # Extract version from Cargo.toml
    VERSION=$(grep -m 1 '^version =' Cargo.toml | cut -d '"' -f 2)
    TARBALL="kaede-${VERSION}.tar.gz"

    echo "Creating source archive: $TARBALL"

    TMP_ARCHIVE=$(mktemp -d)

    # Create a clean source tarball excluding build artifacts and VCS data
    tar \
        --exclude="./target" \
        --exclude="./build" \
        --exclude="./.git" \
        --exclude="./pkg" \
        --exclude="./$TARBALL" \
        -czf "$TMP_ARCHIVE/$TARBALL" .

    # Use an isolated directory to prevent makepkg from modifying the source tree
    ARCH_BUILD_DIR=$(mktemp -d)

    cp PKGBUILD "$ARCH_BUILD_DIR/"
    mv "$TMP_ARCHIVE/$TARBALL" "$ARCH_BUILD_DIR/"
    rmdir "$TMP_ARCHIVE"

    echo "Running makepkg in isolated directory: $ARCH_BUILD_DIR"

    pushd "$ARCH_BUILD_DIR" > /dev/null
    PKGEXT='.pkg.tar.zst'
    makepkg -f --noprogressbar --nodeps
    cp ./*.pkg.tar.zst "$OLDPWD/build/" || true
    popd > /dev/null

    # Clean up temporary build directory
    rm -rf "$ARCH_BUILD_DIR"
else
    echo "makepkg not found. Skipping Arch Linux package."
fi

# --------------------------------------------------
# 5. Build Nix package
# --------------------------------------------------
if command -v nix-build &> /dev/null; then
    echo "Building Nix package (kaede-beta)..."
    nix-build default.nix -o result-nix
    cp -rL result-nix build/kaede-beta-nixos
    rm result-nix
else
    echo "nix-build not available. Skipping Nix package."
fi

echo "--- Build completed successfully ---"
echo "Artifacts available in ./build"
ls -lh build/
