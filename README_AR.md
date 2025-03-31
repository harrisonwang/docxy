# Docxy

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-28%2B-blue.svg)](https://www.docker.com)
[![GitHub release](https://img.shields.io/github/v/release/harrisonwang/docxy)](https://github.com/harrisonwang/docxy/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<div align="center">
  <a href="./README.md"><img alt="README in English" src="https://img.shields.io/badge/English-d9d9d9"></a>
  <a href="./README_CN.md"><img alt="简体中文版自述文件" src="https://img.shields.io/badge/简体中文-d9d9d9"></a>
  <a href="./README_RU.md"><img alt="README на русском" src="https://img.shields.io/badge/Русский-d9d9d9"></a>
  <a href="./README_ES.md"><img alt="README en Español" src="https://img.shields.io/badge/Español-d9d9d9"></a>
  <a href="./README_KR.md"><img alt="README in Korean" src="https://img.shields.io/badge/한국어-d9d9d9"></a>
  <a href="./README_AR.md"><img alt="README بالعربية" src="https://img.shields.io/badge/العربية-d9d9d9"></a>
  <a href="./README_TR.md"><img alt="Türkçe README" src="https://img.shields.io/badge/Türkçe-d9d9d9"></a>
</div>

خدمة وكيل خفيفة لصور دوكر، تحل مشكلة الوصول المقيد إلى Docker Hub في مناطق معينة.

## الخلفية

### مقدمة لمستودعات صور دوكر

مستودعات صور دوكر هي خدمات لتخزين وتوزيع صور حاويات دوكر، توفر تخزينًا مركزيًا للتطبيقات المحوسبة. تسمح هذه المستودعات للمطورين بدفع وتخزين وإدارة وسحب صور الحاويات، مما يبسط عملية توزيع ونشر التطبيقات.

### أنواع مستودعات الصور

- **مستودع الصور الرسمي**: Docker Hub، المستودع الرسمي الذي تديره شركة دوكر
- **مستودعات الصور المستقلة من طرف ثالث**: مثل AWS ECR، Google GCR، Alibaba Cloud ACR، إلخ، المستخدمة لنشر ومشاركة الصور الخاصة
- **خدمات تسريع الصور**: مثل مرآة Tsinghua TUNA، ومسرع صور Alibaba Cloud، إلخ، التي توفر خدمات تسريع صور لـ Docker Hub

> [!NOTE]
> بسبب قيود الشبكة، يصعب الوصول المباشر إلى Docker Hub في بعض المناطق، كما تم إيقاف معظم خدمات تسريع الصور أيضًا.

### لماذا نحتاج إلى وكيل الصور

وكلاء الصور هي خدمات وسيطة تربط عملاء دوكر بـ Docker Hub. إنها لا تخزن الصور الفعلية، بل تعيد توجيه الطلبات فقط، مما يحل بفعالية:

- مشاكل قيود الوصول إلى الشبكة
- تحسين سرعة تنزيل الصور

Docxy هي خدمة وكيل صور تهدف إلى تجاوز حظر الشبكة وتسريع تنزيلات الصور من خلال وكيل ذاتي الاستضافة.

### قيود استخدام وكلاء الصور

يطبق Docker Hub سياسات صارمة لتحديد معدل سحب الصور. عند استخدام خدمات الوكيل، توجد القيود التالية:

- إذا لم تكن مسجلاً، فإن كل عنوان IP محدود بـ 10 سحب للصور في الساعة
- إذا كنت مسجلاً بحساب شخصي، يمكنك سحب 100 صورة في الساعة
- بالنسبة لأنواع الحسابات الأخرى، يرجى الرجوع إلى الجدول أدناه:

| نوع المستخدم                | حد معدل السحب            |
| --------------------------- | ------------------------ |
| Business (مصادق عليه)       | غير محدود                |
| Team (مصادق عليه)           | غير محدود                |
| Pro (مصادق عليه)            | غير محدود                |
| **Personal (مصادق عليه)**   | **100/ساعة/حساب**       |
| **المستخدمون غير المصادق عليهم** | **10/ساعة/IP**          |

> [!WARNING]
> ملاحظة: سيدخل هذا القيد حيز التنفيذ اعتبارًا من 1 أبريل 2025

## المبادئ التقنية

تنفذ Docxy وكيلًا كاملًا لـ Docker Registry API، والذي يتطلب فقط إضافة تكوين وكيل عميل دوكر للاستخدام.

### هيكل النظام

```mermaid
graph TD
    Client[عميل دوكر] -->|إرسال طلب| HttpServer[خادم HTTP]
    
    subgraph "خدمة وكيل صور دوكر"
        HttpServer -->|توجيه الطلب| RouterHandler[معالج التوجيه]
        
        RouterHandler -->|/v2/| ChallengeHandler[معالج التحدي<br>proxy_challenge]
        RouterHandler -->|/auth/token| TokenHandler[معالج الرمز المميز<br>get_token]
        RouterHandler -->|/v2/namespace/image/path_type| RequestHandler[معالج الطلبات<br>handle_request]
        RouterHandler -->|/health| HealthCheck[فحص الصحة]
        
        ChallengeHandler --> HttpClient
        TokenHandler --> HttpClient
        RequestHandler --> HttpClient
        
    end
    
    HttpClient[عميل HTTP<br>reqwest]
    
    HttpClient -->|طلب المصادقة| DockerAuth[مصادقة دوكر<br>auth.docker.io]
    HttpClient -->|طلب الصورة| DockerRegistry[سجل دوكر<br>registry-1.docker.io]
```

### تدفق الطلب

```mermaid
sequenceDiagram
    actor Client as عميل دوكر
    participant Proxy as وكيل Docxy
    participant Registry as سجل دوكر
    participant Auth as خدمة مصادقة دوكر
    
    %% معالجة طلب التحدي
    Client->>Proxy: GET /v2/
    Proxy->>+Registry: GET /v2/
    Registry-->>-Proxy: 401 Unauthorized (WWW-Authenticate)
    Proxy->>Proxy: تعديل رأس WWW-Authenticate، توجيه إلى /auth/token المحلي
    Proxy-->>Client: 401 إرجاع رأس المصادقة المعدل
    
    %% الحصول على الرمز المميز
    Client->>Proxy: GET /auth/token?scope=repository:redis:pull
    Proxy->>+Auth: GET /token?service=registry.docker.io&scope=repository:library/redis:pull
    Auth-->>-Proxy: 200 إرجاع الرمز المميز
    Proxy-->>Client: 200 إرجاع استجابة الرمز المميز الأصلية
    
    %% معالجة طلب بيانات وصف الصورة
    Client->>Proxy: GET /v2/library/redis/manifests/latest
    Proxy->>+Registry: إعادة توجيه الطلب (مع رأس المصادقة ورأس Accept)
    Registry-->>-Proxy: إرجاع بيان الصورة
    Proxy-->>Client: إرجاع بيان الصورة (الحفاظ على رؤوس الاستجابة الأصلية ورمز الحالة)
    
    %% معالجة البيانات الثنائية
    Client->>Proxy: GET /v2/library/redis/blobs/{digest}
    Proxy->>+Registry: إعادة توجيه طلب blob
    Registry-->>-Proxy: إرجاع بيانات blob
    Proxy-->>Client: تدفق بيانات blob مرة أخرى
```

### عملية معالجة الشهادات

```mermaid
flowchart LR
    A[بدء الخدمة] --> B{التحقق من متغيرات البيئة}
    B -->|موجودة| C[استخدام مسار الشهادة المحدد]
    B -->|غير موجودة| D[استخدام مسار الشهادة الافتراضي]
    C --> E[تحميل ملفات الشهادة]
    D --> E
    E --> F{تحديد نوع الشهادة}
    F -->|ECC| G[تحميل المفتاح الخاص ECC]
    F -->|RSA| H[تحميل المفتاح الخاص RSA]
    F -->|PKCS8| I[تحميل المفتاح الخاص PKCS8]
    G --> J[تهيئة تكوين TLS]
    H --> J
    I --> J
    J --> K[بدء خدمة HTTPS]
```

## الميزات

- **وكيل شفاف**: متوافق تمامًا مع Docker Registry API v2
- **تكامل سلس**: يتطلب فقط تكوين مصدر الصورة، دون تغيير عادات الاستخدام
- **نقل عالي الأداء**: يستخدم معالجة البث للبيانات المستجابة، يدعم تنزيل الصور الكبيرة
- **تشفير TLS**: دعم HTTPS مدمج، يضمن نقل البيانات الآمن
- **تسريع تنزيل الصور الرسمية**: يوفر اتصالات أكثر استقرارًا
- **تجاوز حظر الشبكة**: يحل مشاكل تقييد الوصول في مناطق معينة

## البدء السريع

> [!TIP]
> قبل النشر، يرجى حل اسم المجال إلى المضيف الهدف مسبقًا.

### نشر بنقرة واحدة

```bash
bash <(curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/install.sh)
```

> [!WARNING]
> ملاحظة: تتطلب هيئة إصدار شهادات ZeroSSL تسجيل حساب قبل إصدار الشهادات. لسهولة الاستخدام، يفرض البرنامج النصي استخدام Let's Encrypt كهيئة إصدار شهادات ويفرض إعادة إصدار الشهادة.

### التطوير

1. استنساخ المستودع

   ```bash
   cd /opt
   git clone https://github.com/harrisonwang/docxy.git
   ```

2. الدخول إلى دليل المشروع

   ```bash
   cd /opt/docxy
   ```

3. تكوين الشهادات (باستخدام اسم المجال test.com كمثال)

   ```bash
   export CERT_PATH=/root/.acme.sh/test.com_ecc/fullchain.cer
   export KEY_PATH=/root/.acme.sh/test.com_ecc/test.com.key
   ```

> [!TIP]
> يرجى التقدم بطلب للحصول على شهادات TLS مسبقًا باستخدام acme.sh

4. بدء الخدمة

   ```bash
   cargo run
   ```

5. بناء الحزمة الثنائية

   ```bash
   cargo build --release
   ```

### تكوين عميل دوكر

قم بتحرير ملف التكوين `/etc/docker/daemon.json` وأضف إعدادات الوكيل التالية:

```json
{
  "registry-mirrors": ["https://test.com"]
}
```

### فحص الصحة

يمكنك التحقق مما إذا كانت الخدمة تعمل بشكل صحيح من خلال الوصول إلى نقطة النهاية التالية:

```bash
curl https://test.com/health
```

## مرجع API

| نقطة النهاية | الطريقة | الوصف |
|--------------|---------|-------|
| `/health` | GET | واجهة فحص الصحة |
| `/v2/` | GET | نقطة دخول Docker Registry API v2 وتحدي المصادقة |
| `/auth/token` | GET | واجهة الحصول على رمز المصادقة |
| `/v2/{namespace}/{image}/{path_type}/{reference}` | GET/HEAD | واجهة الوصول إلى موارد الصورة، تدعم manifests وblobs وغيرها |

## حلول أخرى

- [تنفيذ وكيل الصور باستخدام Cloudflare Worker](https://voxsay.com/posts/china-docker-registry-proxy-guide/): استخدم بحذر، قد يؤدي إلى تعليق حساب Cloudflare.
- [تنفيذ وكيل الصور باستخدام Nginx](https://voxsay.com/posts/china-docker-registry-proxy-guide/): يقوم بعمل وكيل لـ registry-1.docker.io فقط، ولكن لا يزال هناك طلبات مرسلة إلى auth.docker.io. بمجرد حظر auth.docker.io أيضًا، لن يعمل بشكل صحيح.

## الترخيص

هذا المشروع مرخص بموجب ترخيص MIT - انظر ملف [LICENSE](LICENSE) لمزيد من التفاصيل.
