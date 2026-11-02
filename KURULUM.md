# Kurulum

Kısa yol: derle, kur, çalıştır. Ayrıntılar ve gerekçeler için
[docs/gelistirme.md](docs/gelistirme.md).

---

## 1. Bağımlılıklar (bir kez)

Depoyu indir ve proje klasörüne geç:

```bash
git clone https://github.com/NYAYAN/claude-session-board.git
cd claude-session-board
```

```bash
npm install
```

Bilgisayarda olması gerekenler:

| | Windows | macOS |
|---|---|---|
| Node.js | 20+ | 20+ |
| Rust | 1.82+ | 1.82+ |
| Derleyici | Visual Studio, "C++ ile masaüstü geliştirme" bileşeni | Xcode Command Line Tools (`xcode-select --install`) |
| Sistem | Windows 10 / 11 | macOS 11+ |

Windows 10 kullanıyorsan ayrıca
[WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
gerekiyor; Windows 11'de yerleşik.

## 2. Kurucuyu üret

Kurulum paketi derlenirken güncelleme imzası da üretiliyor, bu yüzden önce imza anahtarını aynı
pencerede tanıt (anahtarın nereden geldiği:
[docs/gelistirme.md](docs/gelistirme.md#sürüm-çıkarma)). Windows'ta PowerShell'de:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key" -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key.password" -Raw).Trim()
```

macOS'ta Terminal'de:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/sessionboard.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/sessionboard.key.password)"
```

```bash
npm run build
```

İlk derleme birkaç dakika sürer, sonrakiler daha kısa. Sonunda ekrana üretilen dosyaların yolları
yazılır.

**Yalnız kendi bilgisayarın için, anahtarsız.** Güncelleme paketi gerekmiyorsa imzayı kapatıp derle:

```bash
npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

### Windows'ta oluşan dosyalar

`src-tauri\target\release\bundle\nsis\` altında:

| Dosya | Ne işe yarar |
|---|---|
| `SessionBoard_<sürüm>_x64-setup.exe` | Kurulum paketi |
| `SessionBoard_<sürüm>_x64-setup.exe.sig` | Güncelleyicinin doğruladığı imza; yalnızca sürüm yayınlarken gerekir |

Ayrıca `src-tauri\target\release\sessionboard.exe` var: kurulum yapmadan doğrudan çalıştırılabilen
ikili. Başlat menüsüne kısayol koymaz.

> **`LNK1104: cannot open file 'msvcrt.lib'` alırsan** Rust en yeni Visual Studio kurulumunu seçmiş ve
> orada C++ kütüphaneleri eksik. Komutu tam bir kurulumun geliştirici ortamında çalıştır:
> `cmd /c '"<Visual Studio klasörü>\VC\Auxiliary\Build\vcvars64.bat" && npm run build'`.
> Ayrıntı: [docs/gelistirme.md](docs/gelistirme.md#lnk1104-cannot-open-file-msvcrtlib).

### macOS'ta oluşan dosyalar

`src-tauri/target/release/bundle/` altında:

| Dosya | Ne işe yarar |
|---|---|
| `macos/SessionBoard.app` | Uygulamanın kendisi |
| `dmg/SessionBoard_<sürüm>_aarch64.dmg` | Dağıtım için disk görüntüsü (Intel Mac'te `_x64`) |
| `macos/SessionBoard.app.tar.gz` + `.sig` | Güncelleyicinin indirdiği paket ve imzası; yalnızca sürüm yayınlarken gerekir |

Hem Apple Silicon hem Intel'de çalışan tek paket için iki hedefi bir kez ekleyip öyle derle (CI böyle
derliyor):

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run build -- --target universal-apple-darwin
```

O zaman dosyalar `src-tauri/target/universal-apple-darwin/release/bundle/` altında, DMG'nin adı
`_universal.dmg` ile biter.

> **DMG adımı "Not authorized to send Apple events to Finder" ile düşerse** derlemeyi çalıştıran
> uygulamanın Finder'ı yönetme izni yok (DMG penceresinin düzeni Finder'a yaptırılıyor). macOS ilk
> seferde sorar; sorduysa ve reddedildiyse **Sistem Ayarları › Gizlilik ve Güvenlik › Otomasyon**'dan aç.
> `.app` bu hatadan önce üretilmiş olur, yalnızca DMG eksik kalır.

> **"A public key has been found, but no private key" alırsan** imza anahtarı tanıtılmamış; yukarıdaki
> iki satırı aynı pencerede çalıştırıp yeniden dene ya da anahtarsız derle.

## 3. Kur

**Windows:** `SessionBoard_<sürüm>_x64-setup.exe` dosyasına çift tıkla. Kurulum kullanıcı klasörüne
yapılır, yönetici hakkı istemez.

Kurulduğu yer:

```
C:\Users\<kullanıcı>\AppData\Local\SessionBoard\
```

**macOS:** `SessionBoard.app`'i **Uygulamalar** klasörüne kopyala (ya da DMG'yi açıp oraya sürükle):

```bash
cp -R src-tauri/target/release/bundle/macos/SessionBoard.app /Applications/
```

Kendi derlediğin uygulama ilk açılışta uyarı vermez; uyarı yalnızca internetten indirilen pakette çıkar
(bkz. [README](README.md#paketi-indirip-kurmak)).

Gruplar ve tercihler ayrı bir yerde durur; kaldırma işlemi onlara dokunmaz:

```
C:\Users\<kullanıcı>\.sessionboard\     (Windows)
~/.sessionboard/                       (macOS)
```

## 4. Geliştirirken

Kurmak yerine kaynaktan çalıştırmak için:

```bash
npm run dev
```

Rust tarafını değiştirince uygulama yeniden derlenip başlar.

> **Kurulu SessionBoard açıksa önce onu kapat.** Tek pencere kuralı yüzünden yeni süreç açılmaz,
> kurulu olanın penceresi öne gelir. Geliştirme sürümü de gerçek `state.json`'ı okur ve yazar.

Testler:

```bash
npm run test:rust
```
