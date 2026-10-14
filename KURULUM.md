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

| | |
|---|---|
| Node.js | 20+ |
| Rust | 1.82+ |
| Visual Studio | "C++ ile masaüstü geliştirme" bileşeni |
| Windows | 10 / 11 |

Windows 10 kullanıyorsan ayrıca
[WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
gerekiyor; Windows 11'de yerleşik.

## 2. Kurucuyu üret

Kurulum paketi derlenirken güncelleme imzası da üretiliyor, bu yüzden önce imza anahtarını aynı
PowerShell penceresinde tanıt (anahtarın nereden geldiği:
[docs/gelistirme.md](docs/gelistirme.md#sürüm-çıkarma)):

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key" -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key.password" -Raw).Trim()
```

```bash
npm run build
```

İlk derleme birkaç dakika sürer, sonrakiler daha kısa. Sonunda ekrana üretilen dosyaların yolları
yazılır.

**Oluşan dosyalar**: `src-tauri\target\release\bundle\nsis\` altında:

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

> **"A public key has been found, but no private key" alırsan** imza anahtarı tanıtılmamış; yukarıdaki
> iki satırı aynı pencerede çalıştırıp yeniden dene.

## 3. Kur

`SessionBoard_<sürüm>_x64-setup.exe` dosyasına çift tıkla. Kurulum kullanıcı klasörüne yapılır, yönetici
hakkı istemez.

Kurulduğu yer:

```
C:\Users\<kullanıcı>\AppData\Local\SessionBoard\
```

Gruplar ve tercihler ayrı bir yerde durur; kaldırma işlemi onlara dokunmaz:

```
C:\Users\<kullanıcı>\.sessionboard\
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
