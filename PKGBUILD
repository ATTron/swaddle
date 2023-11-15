pkgname=swaddle
pkgver=0.1.1
pkgrel=1
pkgdesc="Swayidle inhibitor when watching content or listening to audio"
arch=('x86_64')
license=('GPL')
depends=('dbus' 'openssl')
makedepends=('cargo' 'rust')
source=("$pkgname-$pkgver.tar.gz::https://github.com/attron/$pkgname/archive/v$pkgver.tar.gz"
        "$pkgname.service")
sha256sums=('SKIP'
            'SKIP')


build() {
    cd "$srcdir/$pkgname-$pkgver"
    cargo build --release --locked
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm644 "$srcdir/$pkgname.service" "$pkgdir/usr/lib/systemd/system/$pkgname.service"
}

post_install() {
    systemctl daemon-reload
}

post_upgrade() {
    post_install
}
