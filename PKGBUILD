# Maintainer: wioletowa
# Builds the checkout this PKGBUILD sits in: run `makepkg -si` here.
pkgname=morph-converter
pkgver=0.1.0
pkgrel=1
pkgdesc="Native file converter for GNOME (images, audio, video) with GPU encoding, inspired by VERT"
arch=('x86_64' 'aarch64')
license=('MIT')
depends=('gtk4' 'libadwaita' 'libvips' 'ffmpeg')
makedepends=('cargo' 'glib2')
optdepends=('nvidia-utils: NVENC video encoding on NVIDIA GPUs'
            'intel-media-driver: VAAPI video encoding on Intel GPUs'
            'libva-mesa-driver: VAAPI video encoding on AMD GPUs')
# Cargo already does LTO and stripping (see Cargo.toml).
options=('!lto' '!debug')

prepare() {
  cd "$startdir"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "$startdir"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo build --frozen --release
}

check() {
  cd "$startdir"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo test --frozen --release
}

package() {
  cd "$startdir"
  install -Dm755 "$srcdir/target/release/morph" "$pkgdir/usr/bin/morph"
  install -Dm644 data/io.github.wioletowa.Morph.desktop -t "$pkgdir/usr/share/applications"
  install -Dm644 data/io.github.wioletowa.Morph.metainfo.xml -t "$pkgdir/usr/share/metainfo"
  install -Dm644 data/icons/hicolor/scalable/apps/io.github.wioletowa.Morph.svg -t "$pkgdir/usr/share/icons/hicolor/scalable/apps"
  install -Dm644 LICENSE -t "$pkgdir/usr/share/licenses/$pkgname"
}
