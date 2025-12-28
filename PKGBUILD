# Maintainer: lonerOrz <2788892716-at-qq-dot-com>

pkgname=pantry-git
pkgver=r20250710.1234567
pkgrel=1
pkgdesc="A generic selector tool for handling various types of entries with text and image preview modes"
arch=('x86_64')
url="https://github.com/lonerOrz/pantry"
license=('BSD')
depends=('gtk4' 'gdk-pixbuf2' 'glibc')
makedepends=('rust' 'cargo' 'git')
provides=('pantry')
conflicts=('pantry')
source=('git+https://github.com/lonerOrz/pantry.git')
sha256sums=('SKIP')
options=('!strip')

pkgver() {
  cd pantry
  printf "r%s.%s" "$(git log -1 --format='%cd' --date=unix)" "$(git rev-parse --short HEAD)"
}

build() {
  cd pantry
  export RUST_BACKTRACE=1
  cargo build --release --locked
}

package() {
  cd pantry
  install -Dm755 "target/release/pantry" "${pkgdir}/usr/bin/pantry"

  # Install documentation
  install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
  install -Dm644 "README.md" "${pkgdir}/usr/share/doc/${pkgname}/README.md"

  # Install example configuration files
  install -dm755 "${pkgdir}/usr/share/pantry/examples"
  cp -r doc/* "${pkgdir}/usr/share/pantry/examples/"
}

# vim:set ts=2 sw=2 et:
