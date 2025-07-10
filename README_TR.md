# Docxy

![og-image](og-image.png)

[![English](https://img.shields.io/badge/English-Click-orange)](README_EN.md)
[![简体中文](https://img.shields.io/badge/简体中文-点击查看-blue)](README.md)
[![Русский](https://img.shields.io/badge/Русский-Нажмите-orange)](README_RU.md)
[![Español](https://img.shields.io/badge/Español-Clic-blue)](README_ES.md)
[![한국어](https://img.shields.io/badge/한국어-클릭-orange)](README_KR.md)
[![العربية](https://img.shields.io/badge/العربية-انقر-blue)](README_AR.md)
[![Türkçe](https://img.shields.io/badge/Türkçe-Tıkla-orange)](README_TR.md)

Hafif bir Docker görüntü proxy hizmeti, Çin anakarasında Docker Hub'a kısıtlı erişim sorununu çözmek için tasarlanmıştır.

> 📢 **Blog Eğitimi:** [**Docker Hub Bağlantı Zaman Aşımlarına Elveda Deyin! Docxy ile Özel Görüntü Hızlandırıcınızı Oluşturun**](https://voxsay.com/posts/docxy-docker-proxy-tutorial-for-china/)

## Temel Özellikler

*   🚀 **Tek Tıkla Dağıtım**: Ortam kurulumu, sertifika başvurusu (Let's Encrypt) ve hizmet dağıtımını tek tıklamayla tamamlamak için `install.sh` otomasyon betiği sağlar, manuel müdahale gerektirmez.

*   📦 **Çoklu Dağıtım Modları**:
    *   **Bağımsız**: Dahili TLS işlevselliği, doğrudan HTTPS hizmeti sağlar.
    *   **Nginx Proxy**: Nginx ile bir arka uç hizmeti olarak çalışabilir.
    *   **CDN Kaynağı**: HTTP modunu destekler, CDN entegrasyonu için uygundur.

*   ⚡ **Çekme Hızını Artırmak için Giriş**: Kullanıcıların `docker login` aracılığıyla kişisel Docker Hub hesaplarıyla kimlik doğrulaması yapmasına olanak tanır, anonim kullanıcıların çekme hızı limitini (IP başına saatte 10 çekme) kimliği doğrulanmış kullanıcılarınkine (hesap başına saatte 100 çekme) yükseltir.

*   💎 **Tamamen Şeffaf Proxy**: Docker Registry V2 API ile tamamen uyumludur. İstemcilerin yalnızca ayna kaynağı adresini değiştirmesi gerekir, ek öğrenme eğrisi veya kullanım alışkanlıklarında değişiklik yoktur.

*   🛡️ **Yüksek Performans ve Güvenlik**: **Rust** ve **Actix Web** ile inşa edilmiştir, mükemmel performans ve bellek güvenliği sunar. Görüntü aktarımı için akış kullanır, minimum ek yük ile.

## Kurulum ve Dağıtım

Dağıtım sürecini basitleştirmek için tek tıklamayla kurulum betiği sağlıyoruz. Başlamadan önce, lütfen alan adınızın hedef ana bilgisayara çözümlendiğinden emin olun.

```bash
bash <(curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/install.sh)
```

Betik, kurulum boyunca size rehberlik edecek ve aşağıdaki üç dağıtım modunu sunacaktır:

---

### Mod Bir: Bağımsız (HTTPS)

Bu en basit ve en çok önerilen moddur. Docxy, doğrudan 80 ve 443 numaralı bağlantı noktalarını dinleyecek ve tam bir HTTPS proxy hizmeti sağlayacaktır.

**Özellikler:**
- Ek web sunucusu yapılandırmasına gerek yok.
- HTTP'den HTTPS'ye yönlendirmeyi otomatik olarak yönetir.
- Let's Encrypt sertifikalarını otomatik olarak uygulamak veya kendi sertifikalarınızı kullanmak için seçenek.

**Kurulum Süreci:**
1.  Tek tıklamayla kurulum betiğini çalıştırın.
2.  Mod seçimi istendiğinde `1` girin veya sadece Enter tuşuna basın.
3.  Alan adınızı girmek ve sertifika işleme yöntemini seçmek için istemleri izleyin.
4.  Betik, tüm yapılandırmaları otomatik olarak tamamlayacak ve hizmeti başlatacaktır.

---

<details>
<summary>Mod İki: Nginx Ters Proxy (Gelişmiş)</summary>

### Mod İki: Nginx Ters Proxy

Bu mod, zaten Nginx'iniz varsa ve web hizmetlerini merkezi olarak yönetmek istiyorsanız uygundur.

**Özellikler:**
- Nginx, HTTPS şifrelemesini ve sertifika yönetimini ele alır, Docxy ise düz bir HTTP arka ucu olarak çalışır.
- Docxy, belirtilen bir bağlantı noktasında (örneğin, 9000) bir arka uç HTTP hizmeti olarak çalışır.
- Diğer hizmetlerle entegrasyon için uygundur.

**Kurulum Süreci:**
1.  Tek tıklamayla kurulum betiğini çalıştırın.
2.  Mod seçimi istendiğinde `2` girin.
3.  Alan adınızı, Docxy arka uç dinleme bağlantı noktasını ve sertifika bilgilerini girmek için istemleri izleyin.
4.  Betik, sizin için otomatik olarak örnek bir Nginx yapılandırma dosyası oluşturacaktır. Bunu Nginx yapılandırmanıza manuel olarak eklemeniz ve Nginx hizmetini yeniden yüklemeniz gerekecektir.

</details>

---

<details>
<summary>Mod Üç: CDN Kaynağı (HTTP) (Gelişmiş)</summary>

### Mod Üç: CDN Kaynağı (HTTP)

Bu mod, daha iyi küresel hızlandırma elde etmek için Docxy'yi bir CDN için kaynak olarak kullanmak istiyorsanız uygundur.

**Özellikler:**
- Docxy yalnızca HTTP bağlantı noktalarını dinler.
- CDN sağlayıcısı HTTPS isteklerini ve sertifikalarını yönetir.
- Docxy, istemci IP'sini ve protokolünü doğru bir şekilde tanımlamak için `X-Forwarded-*` başlıklarına güvenir ve bunları işler.

**Kurulum Süreci:**
1.  Tek tıklamayla kurulum betiğini çalıştırın.
2.  Mod seçimi istendiğinde `3` girin.
3.  Docxy'nin dinlemesi gereken HTTP bağlantı noktasını girmek için istemleri izleyin.
4.  CDN hizmetinizi, kaynağını Docxy hizmet adresine ve bağlantı noktasına işaret edecek şekilde yapılandırın.

</details>


## Docker İstemci Kullanımı

Proxy hizmetinizi kullanmak için Docker istemcinizi yapılandırın.

### Yöntem Bir: Anonim Kullanım (Temel Yapılandırma)

Bu, Docker'ın varsayılan isteklerini proxy hizmetinize yönlendiren temel yapılandırmadır.

1.  **Docker Daemon'ı Yapılandırın**

    `/etc/docker/daemon.json` dosyasını düzenleyin (yoksa oluşturun) ve aşağıdaki içeriği ekleyin. `your-domain.com`'u alan adınızla değiştirin.

    ```json
    {
      "registry-mirrors": ["https://your-domain.com"]
    }
    ```

2.  **Docker Hizmetini Yeniden Başlatın**

    ```bash
    sudo systemctl restart docker
    ```
    Şimdi, `docker pull` görüntüleri proxy'niz aracılığıyla çekecektir.

<details>
<summary>Yöntem İki: Giriş Kullanımı (Çekme Hızını Artırın)</summary>

Bu yöntem, anonim kullanıma ek olarak Docker Hub hesabınızla oturum açarak daha yüksek bir görüntü çekme hızı elde etmenizi sağlar.

1.  **Temel Yapılandırmayı Tamamlayın**

    Lütfen **Yöntem Bir**'deki tüm adımları tamamladığınızdan emin olun.

2.  **Proxy Hizmetine Giriş Yapın**

    `docker login` komutunu kullanın ve Docker Hub kullanıcı adınızı ve şifrenizi girin.

    ```bash
    docker login your-domain.com
    ```

3.  **Kimlik Doğrulama Bilgilerini Senkronize Edin**

    Başarılı bir şekilde giriş yaptıktan sonra, `~/.docker/config.json` dosyasını manuel olarak düzenlemeniz gerekir. `your-domain.com` için oluşturulan `auth` bilgilerini kopyalayın ve `https://index.docker.io/v1/` için yapıştırın.

    Değişiklikten önce:
    ```json
    {
        "auths": {
            "your-domain.com": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            }
        }
    }
    ```

    Değişiklikten sonra:
    ```json
    {
        "auths": {
            "your-domain.com": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            },
            "https://index.docker.io/v1/": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            }
        }
    }
    ```
    Dosyayı kaydettikten sonra, `docker pull` istekleriniz kimliği doğrulanmış bir kullanıcı olarak gönderilecek ve böylece daha yüksek hız limitlerinden yararlanacaksınız.

</details>

## Geliştirme

> [!NOTE]
> Ayrıntılı teknik arka plan, sistem mimarisi ve uygulama prensipleri için lütfen [**Teknik Mimari ve Prensipler Belgesi**](docs/ARCHITECTURE.md)'ne bakın.

1.  **Depoyu Klonlayın**
    ```bash
    git clone https://github.com/harrisonwang/docxy.git
    cd docxy
    ```

2.  **Yapılandırma Dosyasını Değiştirin**
    `config/default.toml` dosyasını açın ve HTTP hizmetinin etkinleştirildiğinden ve HTTPS hizmetinin devre dışı bırakıldığından emin olmak için `[server]` bölümünü değiştirin. Geliştirme ortamında ayrıcalıklı bağlantı noktalarını kullanmaktan kaçınmak için bağlantı noktasını 8080 olarak ayarlayabilirsiniz.

    ```toml
    # config/default.toml

    [server]
    http_port = 8080      # Ayrıcalıklı olmayan bağlantı noktası kullanın
    https_port = 8443
    http_enabled = true   # HTTP'yi etkinleştir
    https_enabled = false # HTTPS'yi devre dışı bırak
    behind_proxy = true
    ```

3.  **Projeyi Çalıştırın**
    Şimdi, projeyi doğrudan `cargo` ile çalıştırabilirsiniz.
    ```bash
    cargo run
    ```
    Hizmet başlayacak ve `http://0.0.0.0:8080` adresini dinleyecektir.

4.  **Sürüm Oluşturun**
    ```bash
    cargo build --release
    ```

## Lisans

Bu proje MIT Lisansı altında lisanslanmıştır. Daha fazla bilgi için [LICENSE](LICENSE)'a bakın.