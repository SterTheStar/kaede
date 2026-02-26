pkgname=kaede
pkgver=1.0.0
pkgrel=1
pkgdesc="Linux desktop app to choose which GPU to use per app/game"
arch=('x86_64')
url="https://github.com/SterTheStar/kaede"
license=('GPL3')
depends=('libadwaita' 'gtk4' 'pciutils' 'mesa-utils' 'vulkan-tools')
makedepends=('rust' 'cargo')
source=("kaede-${pkgver}.tar.gz")
sha256sums=('SKIP')

build() {
  cd "$srcdir"
  cargo build --release --locked
}

package() {
  cd "$srcdir"
  install -Dm755 "target/release/kaede" "$pkgdir/usr/bin/kaede"
  install -Dm644 "com.kaede.gpu-manager.desktop" "$pkgdir/usr/share/applications/com.kaede.gpu-manager.desktop"
  install -Dm644 "src/icons/icon.png" "$pkgdir/usr/share/icons/hicolor/256x256/apps/com.kaede.gpu-manager.png"
}
