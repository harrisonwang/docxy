# Docxy Teknik Mimari ve Prensipler

Bu belge, Docxy projesinin arka planını, teknik prensiplerini, sistem mimarisini ve uygulama akışını detaylandırmaktadır.

## Arka Plan

### Docker Görüntü Kayıt Defterlerine Giriş

Docker görüntü kayıt defteri, Docker kapsayıcı görüntülerini depolamak ve dağıtmak için bir hizmettir ve kapsayıcılı uygulamalar için merkezi depolama sağlar. Bu kayıt defterleri, geliştiricilerin kapsayıcı görüntülerini göndermesine, depolamasına, yönetmesine ve çekmesine olanak tanıyarak uygulama dağıtım ve yerleştirme sürecini basitleştirir.

### Görüntü Kayıt Defteri Türleri

- **Resmi Kayıt Defteri**: Docker, Inc. tarafından yönetilen resmi kayıt defteri olan Docker Hub.
- **Üçüncü Taraf Bağımsız Kayıt Defterleri**: AWS ECR, Google GCR, Aliyun ACR vb. gibi tescilli görüntüleri yayınlamak ve paylaşmak için kullanılır.
- **Ayna Hizmetleri**: Tsinghua Üniversitesi'ndeki TUNA ayna sitesi, Aliyun'un ayna hızlandırıcısı vb. gibi Docker Hub için hızlandırma sağlayan hizmetler.

> [!NOTE]
> Ağ kısıtlamaları nedeniyle, Çin anakarasından Docker Hub'a doğrudan erişim zordur ve çoğu ayna hizmeti faaliyetlerini durdurmuştur.

### Neden Bir Kayıt Defteri Proxy'sine İhtiyaç Duyulur?

Bir görüntü proxy'si, Docker istemcisini Docker Hub ile bağlayan bir aracı hizmettir. Gerçek görüntüleri depolamaz, yalnızca istekleri iletir ve aşağıdaki sorunları etkili bir şekilde çözer:

- Ağ erişim kısıtlamaları sorunları
- Görüntü indirme hızlarını artırma

Docxy, ağ engellemelerini aşmayı ve görüntü indirmelerini hızlandırmayı amaçlayan böyle bir görüntü proxy hizmetidir.

### Görüntü Proxy'sinin Kullanım Limitleri

Docker Hub, görüntü çekme işlemlerinde katı hız sınırlama politikaları uygulamaktadır. Bir proxy hizmeti kullanırken aşağıdaki sınırlar geçerlidir:

- Kimliği doğrulanmamış kullanıcılar için IP adresi başına saatte en fazla 10 görüntü çekme işlemine izin verilir.
- Kişisel bir hesapla oturum açmış kullanıcılar için saatte 100 görüntü çekme işlemine izin verilir.
- Diğer hesap türleri için limitler için lütfen aşağıdaki tabloya bakın:

| Kullanıcı Türü               | Çekme Hızı Limiti        |
| ---------------------------- | ------------------------ |
| Business (authenticated)     | Sınırsız                 |
| Team (authenticated)         | Sınırsız                 |
| Pro (authenticated)          | Sınırsız                 |
| **Personal (authenticated)** | **100/saat/hesap**       |
| **Unauthenticated users**    | **10/saat/IP**           |

## Teknik Prensipler

Docxy, Docker Kayıt Defteri API'si için eksiksiz bir proxy uygular ve kullanılmak üzere Docker istemcisine yalnızca bir proxy yapılandırması eklenmesini gerektirir.

### Sistem Mimarisi

```mermaid
graph TD
    Client[Docker İstemcisi] -->|İstek Gönderir| HttpServer[HTTP Sunucusu]
    
    subgraph "Docker Görüntü Proxy Hizmeti"
        HttpServer -->|İsteği Yönlendirir| RouterHandler[Yönlendirici İşleyici]
        
        RouterHandler -->|/v2/| ChallengeHandler[Sınama İşleyici<br>proxy_challenge]
        RouterHandler -->|/auth/token| TokenHandler[Belirteç İşleyici<br>get_token]
        RouterHandler -->|/v2/namespace/image/path_type| RequestHandler[İstek İşleyici<br>handle_request]
        RouterHandler -->|/health| HealthCheck[Sağlık Kontrolü<br>health_check]
        
        ChallengeHandler --> HttpClient
        TokenHandler --> HttpClient
        RequestHandler --> HttpClient
        
    end
    
    HttpClient[HTTP İstemcisi<br>reqwest]
    
    HttpClient -->|Kimlik Doğrulama İsteği| DockerAuth[Docker Kimlik Doğrulama<br>auth.docker.io]
    HttpClient -->|Görüntü İsteği| DockerRegistry[Docker Kayıt Defteri<br>registry-1.docker.io]
```

### İstek Akışı

```mermaid
sequenceDiagram
    autonumber
    actor Client as Docker İstemcisi
    participant Proxy as Docxy Proxy
    participant Registry as Docker Kayıt Defteri
    participant Auth as Docker Kimlik Doğrulama Hizmeti
    
    %% Sınama İsteği İşleme
    Client->>Proxy: GET /v2/
    Proxy->>+Registry: GET /v2/
    Registry-->>-Proxy: 401 Yetkisiz (WWW-Authenticate)
    Proxy->>Proxy: WWW-Authenticate başlığını değiştir, realm'i yerel /auth/token'a yönlendir
    Proxy-->>Client: 401 Değiştirilmiş kimlik doğrulama başlığını döndür
    
    %% Belirteç Edinimi
    Client->>Proxy: GET /auth/token?scope=repository:library/cirros:pull
    Proxy->>+Auth: GET /token?service=registry.docker.io&scope=repository:library/cirros:pull
    Auth-->>-Proxy: 200 Belirteci döndür
    Proxy-->>Client: 200 Orijinal belirteç yanıtını döndür
    
    %% Görüntü Manifest İsteği (özet için)
    Client->>Proxy: HEAD /v2/library/cirros/manifests/latest
    Proxy->>+Registry: İsteği ilet (kimlik doğrulama ve Accept başlıklarıyla)
    Registry-->>-Proxy: Görüntü özetini döndür
    Proxy-->>Client: Görüntü özetini döndür (orijinal başlıkları ve durumu koruyarak)

    %% Görüntü Meta Veri İsteği
    Client->>Proxy: GET /v2/library/cirros/manifests/{docker-content-digest}
    Proxy->>+Registry: İsteği ilet (kimlik doğrulama ve Accept başlıklarıyla)
    Registry-->>-Proxy: Görüntü meta verilerini döndür
    Proxy-->>Client: Görüntü meta verilerini döndür (orijinal başlıkları ve durumu koruyarak)

    %% Görüntü Yapılandırma ve Katman Bilgisi İsteği
    Client->>Proxy: GET /v2/library/cirros/manifests/{digest}
    Proxy->>+Registry: İsteği ilet (kimlik doğrulama ve Accept başlıklarıyla)
    Registry-->>-Proxy: Belirtilen mimari için görüntü yapılandırmasını ve katman bilgilerini döndür
    Proxy-->>Client: Belirtilen mimari için görüntü yapılandırmasını ve katman bilgilerini döndür (orijinal başlıkları ve durumu koruyarak)

    %% Görüntü Yapılandırma Detayları İsteği
    Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
    Proxy->>+Registry: İsteği ilet (kimlik doğrulama ve Accept başlıklarıyla)
    Registry-->>-Proxy: Görüntü yapılandırma detaylarını döndür
    Proxy-->>Client: Görüntü yapılandırma detaylarını döndür (orijinal başlıkları ve durumu koruyarak)
    
    %% Görüntü Katmanı İkili Veri İsteği (her katman için döngü)
    loop Her katman için
        Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
        Proxy->>+Registry: blob isteğini ilet
        Registry-->>-Proxy: blob verilerini döndür
        Proxy-->>Client: blob verilerini geri akışla gönder
    end
```

## Diğer Çözümler

- [Görüntü Proxy'si için Cloudflare Worker](https://voxsay.com/posts/china-docker-registry-proxy-guide/): Dikkatli kullanın, Cloudflare hesabınızın askıya alınmasına neden olabilir.
- [Görüntü Proxy'si için Nginx](https://voxsay.com/posts/china-docker-registry-proxy-guide/): Bu yalnızca `registry-1.docker.io`'yu proxy'ler. `auth.docker.io`'ya yapılan istekler hala doğrudan yapılır, bu nedenle `auth.docker.io` da engellenirse bu çözüm çalışmayacaktır.