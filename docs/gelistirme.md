# Geliştirme

Kaynaktan derleme, mimari, testler ve yayın süreci. Adım adım derleme için
[KURULUM.md](../KURULUM.md); özelliklerin kullanıcı gözünden anlatımı için
[ozellikler.md](ozellikler.md).

- [Gereksinimler](#gereksinimler)
- [Kurulum ve çalıştırma](#kurulum-ve-çalıştırma)
- [Proje yapısı](#proje-yapısı)
- [Testler](#testler)
- [Sürüm çıkarma](#sürüm-çıkarma)
- [Bilinen sınırlar](#bilinen-sınırlar)

## Gereksinimler

| | |
|---|---|
| Windows | 10 / 11 (ConPTY gerektirir) |
| WebView2 | 11'de yerleşik; 10'da [Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) |
| Node.js | 20+ |
| Rust | 1.82+ (`x86_64-pc-windows-msvc`) |
| Visual Studio | 2022 ya da Build Tools, "C++ ile masaüstü geliştirme" bileşeni |

Uygulama yalnızca Windows'ta çalışıyor: Claude masaüstü kayıtlarının yeri, ConPTY ve `claude://`
protokol işleyicisi Windows'a özgü.

## Kurulum ve çalıştırma

```bash
npm install
```

Geliştirme kipinde başlat:

```bash
npm run dev
```

Kurulum paketi üret (imza anahtarı gerekir, aşağıya bakın):

```bash
npm run build
```

Testleri koş:

```bash
npm run test:rust
```

Çıktı `src-tauri\target\release\bundle\nsis\SessionBoard_<sürüm>_x64-setup.exe` ve yanında güncelleyicinin
okuduğu imza dosyası `.sig`.

**Geliştirme sürümü gerçek veriyi kullanır.** `npm run dev` ile açılan uygulama da
`%USERPROFILE%\.sessionboard\state.json`'ı okur ve yazar. Kurulu SessionBoard açıksa önce onu kapatın:
tek pencere kuralı yüzünden yeni süreç açılmaz, kurulu olanın penceresi öne gelir.

**`npm run build` imza anahtarı ister.** Güncelleme paketi derlemede imzalanıyor; anahtar yoksa paketleme
sonunda "A public key has been found, but no private key" hatasıyla düşer. `npm run dev` istemez.

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key" -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key.password" -Raw).Trim()
```

### `LNK1104: cannot open file 'msvcrt.lib'`

Rust bilgisayardaki **en yeni** Visual Studio kurulumunu seçer. O kurulumda C++ masaüstü (x64)
kütüphaneleri eksikse bağlayıcı bu hatayla düşer: linker vardır ama `msvcrt.lib` yoktur. Ya o kurulumu
Visual Studio Installer'dan tamamlayın ya da komutu tam bir kurulumun x64 geliştirici ortamında çalıştırın:

```powershell
cmd /c '"<Visual Studio klasörü>\VC\Auxiliary\Build\vcvars64.bat" && npm run build'
```

## Proje yapısı

```
src/                        arayüz: paketleyicisiz düz JS
  index.html
  app.js                      ağaç, gruplar, sürükle-bırak, menüler, güncelleme bildirimi
  terminal.js                 terminal sekmeleri (xterm.js), ağaç/terminal bölücüsü
  styles.css
  vendor/                     xterm.js; npm run vendor üretir, depoda yok
src-tauri/
  src/main.rs                 komutlar, eklentiler, pencere
  src/sessions.rs             Claude oturum kayıtlarını okur (yalnızca okuma)
  src/store.rs                state.json okuma/yazma (+ .bak)
  src/pty.rs                  ConPTY sözde terminal, Claude CLI'yi bulma
  src/updater.rs              güncelleme denetimi ve kurulumu
  capabilities/default.json   pencere izinleri (yalnızca çekirdek)
  tauri.conf.json             pencere, CSP, paketleme, güncelleyici açık anahtarı
scripts/vendor.mjs          xterm dosyalarını src/vendor'a kopyalar
.github/workflows/          check.yml (her push), release.yml (v* etiketi)
docs/                       belgeler ve README görüntüleri
```

### Neden bazı kararlar böyle

**Paketleyici yok, kütüphaneler gömülü.** Arayüz üç düz JS dosyası; CSP yalnızca uygulamanın kendi
betiklerine izin veriyor (`script-src 'self'`). xterm.js bu yüzden CDN'den değil, uygulamayla birlikte
geliyor: `scripts/vendor.mjs` dosyaları `node_modules`'tan `src/vendor`'a kopyalıyor.

**xterm kopyası `prebuild` değil, `beforeBuildCommand`.** CI'daki tauri-action `npm run tauri build`
çağırıyor; npm'in `prebuild` kancası o yolda çalışmıyor ve paket xterm dosyaları eksik çıkıyordu.
Tauri'nin `beforeBuildCommand` / `beforeDevCommand` alanı her derleme yolunda çalışıyor.

**`dragDropEnabled: false`.** Açıkken Tauri pencereye işletim sistemi düzeyinde bir dosya-bırakma
yakalayıcısı takıyor ve o yakalayıcı sayfa içindeki HTML5 sürükleme olaylarını yutuyor. Kodda hiçbir
belirti vermeyen bir sessizlik: sürükleme başlar ama hiçbir hedef bırakmayı kabul etmez.

**Sürüklerken ve tıklarken ağaç yeniden çizilmez.** Otomatik yenileme ağacı baştan çiziyor. Fare tuşu
basılıyken çizilirse `mousedown` ile `click` farklı öğelere düşüyor ve tık kayboluyor; sürükleme
sırasında çizilirse sürüklenen öğe DOM'dan kalktığı için tarayıcı sürüklemeyi iptal ediyor. Menü, ad
düzenleme, onay penceresi, sürükleme ve basılı fare tuşu bir sayaçla izleniyor; yenileme o sürede
bekletilip iş bitince uygulanıyor.

**Durum dosyası profil kökünde.** SessionBoard Claude'un terminalinden açılırsa Claude'un MSIX paketinin
içinde çalışıyor ve AppData yazımları paketin özel klasörüne yönleniyor. Başlat menüsünden açılan kopya
o dosyayı görmüyor ve iki ayrı grup listesi oluşuyordu. Kullanıcı profilinin kökü sanallaştırılmıyor.

**Okunamayan durum dosyasının üstüne yazılmaz.** `state.json` okunamazsa uygulama boş bir durumla
açılıyor ama kaydı tümden kapatıyor: aksi hâlde ilk tıklamada boş durum eski dosyanın yerine geçerdi.

**Elle sıranın veri modeli.** Sıra grup başına bir kimlik dizisi (`groups[].order`, klasör başına
`ungroupedOrder`). Dizide olmayan oturumlar en üstte, son etkinliğe göre dizilir. Bırakma, ekranda görünen
sırayı diziye yazar; görünmeyen (arşivlenmiş) oturumlar eski sıralarıyla sona eklenir, böylece arşiv
gizliyken yapılan bir sıralama onları yerinden oynatmaz. Başka gruba taşınan oturum klasördeki bütün
dizilerden çıkarılır.

**ConPTY imleç sorusu.** ConPTY açılışta imleç konumunu sorar (`ESC[6n`) ve yanıt gelene kadar süreci
bekletir. Soru, terminalin kimliği arayüze ulaşmadan gelebiliyor; xterm'in yanıtı ve erken tuşlar kimlik
gelene kadar tutulup sonra gönderiliyor. Yoksa Claude açılışta asılı kalıyordu.

**Yarım UTF-8.** PTY okuması çok baytlı bir karakteri ikiye bölebiliyor; tamamlanmamış baytlar bir
sonraki okumaya devrediliyor (testi var).

**`bypassPermissions` geçirilmez.** Masaüstündeki izin modu terminale taşınıyor, ama izinsiz çalışma
dışarıda bırakılıyor: terminalde izinsiz çalışmayı kullanıcının kendisi seçmeli.

**Claude'un ortam değişkenleri temizlenir.** SessionBoard Claude'un içinden açıldıysa `CLAUDE*`,
`ANTHROPIC_BASE_URL` ve MCP bağlantı değişkenleri terminaldeki CLI'ye aktarılmıyor; bunlarla başlayan CLI
kendini masaüstüne bağlı sanıyor. Yalnızca masaüstünden açılış algılandığında (`CLAUDECODE` ya da
`CLAUDE_CODE_ENTRYPOINT` varken) temizleniyor; kullanıcının kendi ayarlarına dokunulmuyor.

**Kayıt kökü aranıyor, sabit yazılmıyor.** Claude Ekim 2026'da Store sürümünün verisini
`%LOCALAPPDATA%\Claude-Data\` altına taşıdı ve yalnızca bilinen yollara bakan sürüm "Claude oturum klasörü
bulunamadı" demeye başladı. Uygulamayı farklı kurulumlarla kullanan bir ekip için tek tek yol yazmak
yetmiyor: kök, `%LOCALAPPDATA%` ve `%APPDATA%`'da adında "claude"/"anthropic" geçen klasörlerde (bir alt
seviye) ve Store paketlerinin `LocalCache`'inde aranıyor (`sessions::roots_in`). Arama her yenilemede
yapılıyor ama yalnızca adı uyan birkaç klasöre iniyor; maliyeti önemsiz. Son çare elle seçim:
`prefs.sessionsDir` doluysa yalnızca o klasöre bakılıyor (`sessions::resolve_custom`), klasör seçme
penceresi `tauri-plugin-dialog` ile Rust tarafında açılıyor (arayüze ek izin gerekmiyor).

**Hesap seçimi.** Kayıtlar hesap klasörlerine ayrılıyor ama Claude'un hangi hesapla açık olduğu dosyalardan
okunamıyor. En son etkinliğin olduğu hesap gösteriliyor.

**Güncelleme Rust tarafında.** Denetim ve kurulum iki Rust komutu (`check_update`, `install_update`);
arayüz yalnızca bildirimi çiziyor ve ilerlemeyi bir kanaldan alıyor. Windows'ta kurulum başlayınca eklenti
süreci hemen sonlandırıyor ve `RunEvent::Exit` gelmiyor: terminaller ve pencere durumu bu yüzden kurulumdan
**önce** kapatılıp kaydediliyor, yoksa Claude süreçleri sahipsiz kalırdı. İndirme yarıda kalırsa bulunan
sürüm unutulmuyor, yeniden denenebiliyor.

**Sürüm ve imza denetimi derlemeden önce.** Etiket ile üç dosyadaki sürümün ayrışması ve imza sırlarının
eksikliği ikisi de derlemenin sonunda, on dakika sonra patlıyordu. İş akışı ikisini en başta denetliyor.

## Testler

```bash
npm run test:rust
```

Rust birim testleri:

| Test | Ne doğruluyor |
|---|---|
| `pty::conpty_ciktiyi_okur_ve_kapanir` | Gerçek bir ConPTY süreci açılıyor, çıktısı okunuyor, kapanıyor (imleç sorusu yanıtlanarak) |
| `pty::yarim_utf8_sonraki_okumaya_kalir` | Bölünmüş çok baytlı karakter bozulmadan birleşiyor |
| `pty::uuid_dogrulama` | `--resume`'a yalnızca biçimi doğru bir konuşma kimliği gidiyor; komut satırına sızabilecek karakterler reddediliyor |
| `sessions::kok_klasorler_bulunur` | Bilinen bütün düzenler (Claude-Data, eski Store paketi, klasik kurulum, kurumsal sürüm) bulunuyor; verisi taşınmış eski yer, Claude'a ait olmayan paket ve klasörler atlanıyor |
| `sessions::elle_secilen_klasor_cozulur` | Elle seçilen klasör (kök, onu içeren klasör ya da içindeki hesap/org klasörü) aynı köke çözülüyor; kayıt olmayan klasör reddediliyor |
| `sessions::worktree_reposuna_baglanir` | Worktree yolu (alt klasörü ve harf büyüklüğü ne olursa olsun) reponun köküne indiriliyor |
| `sessions::duz_klasor_normallesir` | Sondaki ayırıcı atılıyor, `/` ters bölüye çevriliyor; Türkçe karakterli yollar korunuyor |
| `sessions::kimlik_dogrulama` | `claude://` bağlantısına yalnızca biçimi doğru bir oturum kimliği giriyor |

`src-tauri` içinde `cargo test -- --include-ignored` bir de duman testi koşar
(`sessions::gercek_kayitlar_okunur`): bilgisayardaki gerçek Claude kayıtlarını okuyup özetler. CI'da
Claude kurulu olmadığı için varsayılan olarak atlanıyor.

Arayüz (`src/*.js`) için otomatik test yok; CI yalnızca sözdizimini denetliyor (`node --check`).

### GitHub Actions

**`check.yml`**: her push ve pull request'te Windows'ta koşar: JS sözdizimi, `npm run vendor`,
`cargo check --all-targets`, `cargo test`.

**`release.yml`**: `v*` etiketi gönderilince koşar:

1. Etiket, `tauri.conf.json`, `Cargo.toml` ve `package.json`'daki sürüm aynı mı?
2. İmza sırları tanımlı mı?
3. `cargo test`.
4. tauri-action: kurulum paketini derler, imzalar ve `SessionBoard vX.Y.Z` adıyla yayınlar; sürüme
   güncelleyicinin okuduğu `latest.json`'ı ekler.

İlk iki denetimden biri tutmazsa iş akışı derlemeye başlamadan durur.

## Sürüm çıkarma

**Bir kerelik hazırlık: imza anahtarı.** Kurulu uygulamalar yalnızca bu anahtarla imzalanmış güncellemeleri
kabul ediyor (açık anahtar `tauri.conf.json` → `plugins.updater.pubkey`). Gizli anahtar depoda **değil**:
`%USERPROFILE%\.tauri\sessionboard.key` (+ `.key.password`). **Yedekleyin**: kaybolursa kurulu uygulamalar
sonraki sürümleri kabul etmez, herkes bir kez elle yeniden kurmak zorunda kalır. GitHub'a depo sırrı
olarak bir kez ekleyin:

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo NYAYAN/claude-session-board --body (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key" -Raw).Trim()
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo NYAYAN/claude-session-board --body (Get-Content "$env:USERPROFILE\.tauri\sessionboard.key.password" -Raw).Trim()
```

**Her sürümde:**

1. Sürümü üç yerde artırın: `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `package.json`
   (`Cargo.lock` derlemede kendiliğinden güncellenir); commit'leyip push'layın.
2. Etiketi atıp gönderin:

   ```powershell
   git tag v0.2.2
   git push origin v0.2.2
   ```

İş akışı bitince sürüm sayfasında kurulum dosyası, `.sig` ve `latest.json` bulunur; kurulu uygulamalar
yeni sürümü en geç 6 saat içinde (ya da bir sonraki açılışta) görür.

## Bilinen sınırlar

- Yalnızca Windows.
- Claude'un oturum kayıtları belgelenmemiş bir iç biçim; bir Claude güncellemesi listeyi bozabilir.
- Birden çok hesap varsa yalnızca en son etkinliğin olduğu hesabın oturumları görünür.
- SessionBoard'un grupları Claude'un kenar çubuğundaki özel gruplarla eşitlenmez; ikisi ayrı.
- Birleştirilmiş bir klasör *Ayır* ile ayrılınca oturumları eski klasörüne döner, ama birleştirirken
  taşınan gruplar hedef klasörde kalır. Ayrılan oturumlar eski klasörde grupsuz görünür.
- Durum dosyası klasörleri tam yollarıyla tutar; başka bir bilgisayarda gruplar yalnızca aynı yoldaki
  projelerle eşleşir.
- Aynı oturumu terminalde ve Claude penceresinde aynı anda kullanmak konuşma dosyasını bozabilir;
  uygulama yalnızca uyarır, engellemez.
- Kurulum paketi kod imzalı değil; Windows SmartScreen ilk kurulumda uyarır.
- Arayüz için otomatik test yok.
