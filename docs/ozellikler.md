# Özellikler ve kullanım

SessionBoard'un her özelliğinin ayrıntılı anlatımı: ne yapıyor, nasıl kullanılıyor ve neden öyle
davranıyor. Kısa tanıtım ve kurulum için [README](../README.md).

- [Ne yapar](#ne-yapar)
- [Gömülü terminal](#gömülü-terminal)
- [Güncelleme](#güncelleme)
- [Claude'un kayıtları nasıl okunuyor](#claudeun-kayıtları-nasıl-okunuyor)
- [Veri dosyaları](#veri-dosyaları)
- [Klavye kısayolları](#klavye-kısayolları)

## Ne yapar

**Klasör ağacı.** Oturumlar Claude kaydındaki açılış klasörüne (`originCwd`) göre toplanır. Worktree'de
(`…\.claude\worktrees\…`) açılan oturumlar reponun köküne bağlanır: worktree başına ayrı bir klasör,
aynı projenin oturumlarını onlarca parçaya bölerdi. Klasörün görünen adı varsayılan olarak klasör adıdır;
sağ tık → *Yeniden adlandır* ile değişir, boş bırakmak klasör adına döndürür. Klasör satırına tıklamak
klasörü daraltır ya da açar.

**Gruplar.** Klasör satırındaki **+** (ya da sağ tık → *Grup ekle*) yeni bir grup açar ve adını hemen
düzenletir. Gruba atanmamış oturumlar *Grupsuz*'da durur. *Grupsuz* yalnızca klasörde en az bir grup
varken görünür; hiç grubu olmayan bir klasörde oturumlar doğrudan klasörün altında listelenir. Grubu
silmek onay ister: içindeki oturumlar *Grupsuz*'a döner, oturumların kendisi silinmez. Gruplar ve
*Grupsuz* ayrı ayrı daraltılabilir.

**Alt gruplar.** Bir konunun oturumlarını grubun içinde ayrıca toplamak için grup satırındaki **+** (ya da
sağ tık → *Alt grup ekle*) grubun altında bir alt grup açar. Seçili oturumlarla birlikte açmak için
oturuma sağ tık → *Yeni alt gruba…*: oturumun bulunduğu grubun altında yeni bir alt grup açılır ve seçilenler
oraya taşınır. Alt gruplar tek seviyedir: alt grubun altına alt grup açılmaz. Grubun sayısı alt gruplardaki
oturumları da içerir. Alt grubu silmek oturumlarını üst grubuna döndürür; üst grubu silmek alt gruplarını
da siler ve bütün oturumları *Grupsuz*'a döndürür. Var olan bir grup sağ tık → *Şunun altına taşı* ile
başka bir grubun alt grubu olur, *Üst seviyeye çıkar* ile geri döner.

**Sürükle-bırak.** Üç şey sürüklenir: klasör, grup ve oturum. Bırakma konumu ince turuncu bir çizgiyle
gösterilir; imleç hedef satırın üst yarısındaysa önüne, alt yarısındaysa arkasına bırakılır.

- **Klasör** başka bir klasörün önüne ya da arkasına bırakılır.
- **Grup** yalnızca kendi klasörü içinde taşınır; *Grupsuz*'un üzerine bırakmak grubu sona koyar.
  Başka klasöre taşınamaz, çünkü oturumlar klasörlerine bağlı: grup gitse içindekiler gidemezdi. Grup,
  bırakıldığı yerdeki grubun seviyesine geçer: bir alt grubun önüne ya da arkasına bırakılan grup o üst
  grubun alt grubu olur, üst seviye bir grubun yanına bırakılan alt grup üst seviyeye çıkar. Alt grubu
  olan bir grup başka bir grubun altına bırakılamaz.
- **Oturum** başka bir oturumun üzerine bırakılınca önüne ya da arkasına geçer. Hedef aynı gruptaysa
  yalnızca sıra değişir; başka bir gruptaysa oturum o gruba, tam o konuma taşınır. Grubun başlık satırına
  (ya da boş bir gruba) bırakmak oturumu en üste koyar; *Grupsuz*'a bırakmak gruptan çıkarır. Oturumlar
  yalnızca kendi klasörü içinde taşınır.

Ctrl ve Shift ile seçilen oturumlar birlikte sürüklenir ve ekrandaki sıralarını koruyarak yerleşir.
Aramadayken sıralama kapalıdır, oturumlar yalnızca gruba taşınabilir: süzülmüş bir listede "şunun
arkasına" demenin görünmeyen oturumlar için bir anlamı yok.

**Oturum sırası.** Elle verilen sıra grup başına saklanır. Sıraya hiç girmemiş oturumlar (yeni açılanlar,
menüyle taşınanlar) grubun en üstünde, kendi aralarında son etkinliğe göre durur. Yeni bir oturum en alta
ya da araya düşseydi, özenle dizdiğiniz uzun bir listede kaybolurdu. Menüyle başka gruba taşınan oturum
eski grubundaki sırasından çıkar. Sırayı sıfırlamak için sağ tık → *Oturumları son etkinliğe göre
sırala*: grup satırında o grubu, klasör satırında klasördeki bütün grupları sıfırlar. Elle sıra yokken
seçenek soluk görünür.

**Sağ tık menüleri.**

| Nerede | Neler var |
|---|---|
| Oturum | Terminalde aç, *Kopyasını terminalde aç*, Claude'da aç, Bağlantıyı kopyala; *Gruba taşı*: gruplar ve alt grupları (↳), *Grupsuz*, *Yeni gruba…*, *Yeni alt gruba…* (oturum bir gruptaysa) |
| Grup | Yeniden adlandır, *Alt grup ekle*, Yukarı / Aşağı taşı (aynı seviyede), Oturumları son etkinliğe göre sırala, *Şunun altına taşı* / *Üst seviyeye çıkar*, Grubu sil |
| Klasör | Grup ekle, Yeniden adlandır, Favori, Gizle, Yukarı / Aşağı taşı, Oturumları son etkinliğe göre sırala, *Ayır* (birleştirilmişse), *Bu klasörü şunun altına kat* |

Birden çok oturum seçiliyken oturum menüsü yalnızca taşıma seçeneklerini gösterir (*N oturumu taşı*).
*Gruba taşı* altında ilk beş grup görünür, kalanlar *Daha fazla (N)…* ile aynı yerde açılır: onlarca
grubu olan bir klasörde düz liste menüyü ekranın dışına taşıyordu. Altı grup varsa *Daha fazla (1)* yer
kazandırmadığı için hepsi doğrudan gösterilir.

**Favoriler ve gizleme.** Klasör satırındaki ☆ klasörü favorilere ekler; üst çubuktaki ★ yalnızca favori
klasörleri gösterir. Göz düğmesi klasörü gizler; gizlilerini görmek için *⋯ › Gizli klasörleri göster*
(gizli klasörler o zaman soluk görünür). Alt çubuk kaç klasörün gizli olduğunu yazar.

**Klasör birleştirme.** Bir projeyi yeni bir klasöre taşıdıysanız eski klasörde kalan oturumlar ayrı bir
klasör olarak görünür. Eski klasörde sağ tık → *Bu klasörü şunun altına kat* → yeni klasör: oturumları ve
grupları hedefte görünür. Hedefin sağ tık menüsünde *Ayır* ile geri alınır (bkz.
[Bilinen sınırlar](gelistirme.md#bilinen-sınırlar)).

**Seçim.** Tek tık yalnızca seçer, hiçbir şeyi açmaz: listeyi gezerken yanlışlıkla bir oturuma geçmek ya
da terminal başlatmak istenmez. `Ctrl` + tık seçime ekler ya da çıkarır, `Shift` + tık aralık seçer.

**Arama.** `Ctrl+F` arama kutusuna geçer. Arama Türkçe karakterlere ve büyük/küçük harfe duyarsızdır
(`cetele` → *Çetele*, `isim` → *İsim*); oturum başlığında ve dal adında arar. Aramada daraltılmış klasör
ve gruplar açık görünür, eşleşmesi olmayan klasör ve gruplar gizlenir. `Esc` aramayı temizler.

**Claude'da açma.** Oturumun ikinci düğmesi `claude://claude.ai/epitaxy/<oturum-id>` bağlantısını açar;
Windows bunu Claude masaüstü uygulamasına iletir. Claude öne gelene kadar düğme dönen bir gösterge olur
(en çok 10 saniye). *Bağlantıyı kopyala* aynı bağlantıyı panoya kopyalar.

**Kendiliğinden yenileme.** Liste 20 saniyede bir ve pencereye her dönüşte yenilenir; `F5` elle yeniler.
Siz bir şey yaparken (menü açık, ad düzenleniyor, sürükleme sürüyor, fare tuşu basılı) yenileme bekletilir
ve iş bitince uygulanır: ağaç tam o anda yeniden çizilse tık ya da sürükleme kaybolurdu.

**Arşivlenmiş oturumlar.** Claude'da arşivlenen oturumlar varsayılan olarak gizlidir;
*⋯ › Arşivlenmiş oturumları göster* ile soluk ve italik görünürler.

**Alt çubuk.** Solda görünen oturum ve klasör sayısı, seçim, favori süzgeci, gizli klasörler ve
okunamayan kayıt sayısı; sağda son yenileme saati. Hata olduğunda sağ taraf hatayı kırmızıyla yazar.
Sağ tarafın üzerine gelince oturum kayıtlarının okunduğu klasör görünür.

**Pencere.** SessionBoard tek pencere çalışır: ikinci kez açmak yeni bir kopya başlatmaz, açık pencereyi
öne getirir. Pencerenin boyutu ve konumu hatırlanır. *⋯ › Her zaman üstte* pencereyi diğerlerinin üstünde
tutar. *⋯ › Tümünü daralt / Tümünü genişlet* bütün klasörleri birden kapatır ya da açar.

## Gömülü terminal

Gömülü terminal **bağımsız Claude Code CLI** ister. Masaüstü uygulamanın içindeki kopya kullanılamaz,
çünkü onun oturum açma bilgisini masaüstü uygulama sağlıyor. Windows Terminal ya da PowerShell'de bir kez
kurup giriş yapın:

```powershell
irm https://claude.ai/install.ps1 | iex
claude   # tarayıcıdan giriş yapın, sonra /exit
```

SessionBoard `claude`'u önce PATH'te, sonra `%USERPROFILE%\.local\bin`'de ve `%APPDATA%\npm`'de arar;
son ikisi sayesinde CLI'yi SessionBoard açıkken kurduysanız uygulamayı yeniden başlatmak gerekmez. CLI
bulunamazsa terminal sekmesi kurulum komutunu yazar.

**Sürdürme.** Terminal düğmesi şu komutu oturumun kendi klasöründe çalıştırır:

```
claude --resume <cliSessionId> [--fork-session] [--permission-mode <mod>]
```

İzin modu masaüstündeki kayıttan gelir. `bypassPermissions` bilerek geçirilmez: terminalde izinsiz
çalışmayı kullanıcının kendisi seçmeli (Claude'da Shift+Tab). Oturumun klasörü artık yoksa terminal
açılmaz ve nedenini yazar. Konuşma kaydı olmayan oturumlarda terminal düğmesi soluktur.

İlk kullanımda bir kez uyarı çıkar: aynı oturumu terminalde ve Claude penceresinde **aynı anda**
kullanmayın; ikisi aynı konuşma dosyasına yazar ve terminalde yapılanlar Claude'da oturum yeniden açılana
kadar görünmez. Emin değilseniz *Kopyasını terminalde aç* (`--fork-session`): konuşmanın kopyasıyla yeni
bir oturum başlar, asıl oturum değişmez.

**Sekmeler.** Her oturum bir sekme; açık oturumun terminal düğmesi o sekmeye geçer. Kopya her seferinde
yeni bir sekmede açılır. Terminalde açık oturumlar ağaçta küçük bir terminal simgesiyle, etkin sekmenin
oturumu solda turuncu bir çizgiyle işaretlenir. Çalışan bir sekmeyi kapatmak (× ya da orta tık) onay
ister; Claude süreci sonlanır, konuşma kaydı kalır. Claude kendiliğinden kapanırsa sekme açık kalır,
terminal düğmesine yeniden basmak oturumu yeniden başlatır.

İlk terminal açıldığında pencere terminale yer açmak için en az 1180 piksele genişler (daha genişse
dokunulmaz). Ağaçla terminal arasındaki çizgi sürüklenerek genişlik ayarlanır ve hatırlanır.

**Kopyala / yapıştır.** `Ctrl+C` seçim varken kopyalar, seçim yokken Claude'a gider (çalışan işi keser).
`Ctrl+Shift+C` her zaman kopyalar, `Ctrl+V` yapıştırır. Terminalde sağ tık seçim varsa kopyalar.

**Claude'un içinden açılınca.** SessionBoard Claude'un kendi terminalinden başlatıldıysa masaüstü
uygulamanın kendi Claude Code sürecine verdiği değişkenleri miras alır (`CLAUDE*`, `ANTHROPIC_BASE_URL`,
MCP bağlantı değişkenleri). Bunlarla başlayan CLI kendini masaüstüne bağlı sanar; bu yüzden terminale
aktarılmazlar. Başlat menüsünden açılışta bu değişkenler yoktur, kullanıcının kendi ayarlarına dokunulmaz.

## Güncelleme

Uygulama açılışta ve 6 saatte bir GitHub'daki son sürümü denetler. Yeni sürüm varsa listenin üstünde
**Yeni sürüm hazır: vX.Y.Z — Güncelle** çıkar; × bildirimi bu açılış için gizler. *⋯ menüsü* çalışan
sürümü yazar, *Güncellemeleri denetle* elle denetler ve sonucu söyler.

**Güncelle** önce onay ister; terminalde çalışan Claude varsa kaç tanesinin kapanacağını yazar. Ardından
paket indirilir (alt çubukta yüzde olarak), imzası uygulamanın içindeki açık anahtarla doğrulanır,
terminaller kapatılır, pencere durumu kaydedilir ve kurulum pasif kipte (yalnızca ilerleme çubuğu)
başlar; kurulum bitince uygulama yeniden açılır. Konuşma kayıtları, gruplar ve sıralar etkilenmez.

İmzası tutmayan bir paket kurulmaz. Ağ yokken açılıştaki otomatik denetim sessizce geçer; elle denetim
hatayı yazar. v0.1.0'da güncelleyici yoktur; ondan bir kez elle geçmek gerekir.

## Claude'un kayıtları nasıl okunuyor

| Konu | Ayrıntı |
|---|---|
| Oturum listesi | `<kök>\<hesap>\<org>\local_*.json`, **yalnızca okunur**. Kök: `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\claude-code-sessions` (Claude'un Store/MSIX paketi; AppData'sı sanallaştırıldığı için kayıtlar fiziksel olarak burada) ya da klasik kurulumda `%APPDATA%\Claude\claude-code-sessions` |
| Hesap | Birden çok hesap klasörü varsa en son etkinliğin olduğu hesap gösterilir: Claude'un hangi hesapla açık olduğu dosyalardan bilinemiyor |
| Klasör | Kaydın `originCwd` alanı; worktree oturumları reponun köküne bağlanır |
| Son etkinlik | Masaüstü kaydı ile konuşma dosyasının (`~\.claude\projects\…\<cliSessionId>.jsonl`) değiştirilme zamanının büyüğü. Oturum terminalde sürdürülünce masaüstünün kaydı güncellenmez, konuşma dosyası güncellenir |
| Oturumu açma | `claude://claude.ai/epitaxy/<oturum-id>` → Windows'ta kayıtlı Claude protokol işleyicisi |

> Claude'un oturum kayıtları **belgelenmemiş bir iç biçimdir**. Bir Claude güncellemesi biçimi
> değiştirirse liste boş ya da eksik gelebilir; okunamayan kayıtlar alt çubukta "N kayıt okunamadı"
> olarak sayılır. Uygulama Claude'un dosyalarına hiçbir şey yazmaz.

## Veri dosyaları

Konum `%USERPROFILE%\.sessionboard\`:

| Dosya | İçerik |
|---|---|
| `state.json` | Klasörlerin adları, favori/gizli/daraltma durumları, gruplar ve alt grupları (`parent`), grup başına elle sıra, oturum → grup atamaları, klasör sırası, birleştirmeler ve tercihler |
| `state.json.bak` | Bir önceki kayıt; `state.json` bozulursa geri dönmek için |

Yazma her zaman "geçici dosyaya yaz → eskisini `.bak`'a kopyala → yerine taşı" biçiminde: yarım yazılmış bir
dosya bütün grupları kaybettirmesin. Dosya okunamazsa uygulama alt çubukta kalıcı bir uyarı gösterir ve
**hiç kaydetmez**; boş bir durumla mevcut dosyanın üzerine yazmamak için. O durumda `state.json.bak`'ı
`state.json` olarak kopyalamak son kayda döndürür.

**Neden AppData'da değil.** SessionBoard Claude'un terminalinden açılırsa Claude'un Store (MSIX)
paketinin içinde çalışır ve AppData'ya yazdığı her şey paketin özel klasörüne yönlenir. Başlat
menüsünden açılan SessionBoard o dosyayı göremez ve iki ayrı grup listesi oluşurdu. Kullanıcı profilinin
kökü sanallaştırılmaz; iki durumda da aynı dosya kullanılır. İlk sürüm AppData'yı kullanıyordu; yeni dosya
yoksa eski dosya bir kez oradan okunur.

**Başka bilgisayara taşımak.** `state.json`'ı kopyalamak yeterli. Klasörler tam yollarıyla saklandığı
için gruplar yalnızca aynı yolda açılmış projelerle eşleşir.

## Klavye kısayolları

**Ağaçta**

| Ne yapar | Kısayol |
|---|---|
| Oturumu seç (açmaz) | Tık |
| Seçime ekle / çıkar | Ctrl + tık |
| Aralık seç | Shift + tık |
| Oturum, grup ya da klasör menüsü | Sağ tık |
| Taşı ve sırala | Sürükle-bırak |
| Klasörü / grubu daralt ya da aç | Satıra tık |
| Oturum ara | Ctrl+F |
| Listeyi yenile | F5 / Ctrl+R |
| Menüyü kapat; yoksa aramayı temizle; yoksa seçimi kaldır | Esc |

**Ad düzenlerken ve onay pencerelerinde**

| Ne yapar | Kısayol |
|---|---|
| Kaydet / onayla | Enter (ad düzenlerken dışarı tıklamak da kaydeder) |
| Vazgeç | Esc |

**Terminalde**

| Ne yapar | Kısayol |
|---|---|
| Seçimi kopyala | Ctrl+C (seçim varken) / Ctrl+Shift+C / sağ tık |
| Çalışan işi kes | Ctrl+C (seçim yokken) |
| Yapıştır | Ctrl+V |
| Sekmeyi kapat | Orta tık ya da × |
