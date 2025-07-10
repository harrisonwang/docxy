# Docxy البنية التقنية والمبادئ

توضح هذه الوثيقة الخلفية والمبادئ التقنية وهندسة النظام وتدفق التنفيذ لمشروع Docxy.

## الخلفية

### مقدمة إلى سجلات صور Docker

سجل صور Docker هو خدمة لتخزين وتوزيع صور حاويات Docker، مما يوفر تخزينًا مركزيًا للتطبيقات المعبأة في حاويات. تسمح هذه السجلات للمطورين بدفع وتخزين وإدارة وسحب صور الحاويات، مما يبسط عملية توزيع التطبيقات ونشرها.

### أنواع سجلات الصور

- **السجل الرسمي**: Docker Hub، السجل الرسمي الذي تحتفظ به شركة Docker, Inc.
- **سجلات الجهات الخارجية المستقلة**: مثل AWS ECR، Google GCR، Aliyun ACR، وما إلى ذلك، المستخدمة لنشر ومشاركة الصور الخاصة.
- **خدمات المرآة**: مثل موقع مرآة TUNA في جامعة تسينغهوا، ومسرع مرآة Aliyun، وما إلى ذلك، والتي توفر تسريعًا لـ Docker Hub.

> [!NOTE]
> بسبب قيود الشبكة، يصعب الوصول المباشر إلى Docker Hub من داخل البر الرئيسي للصين، وقد توقفت معظم خدمات المرآة عن العمل.

### لماذا هناك حاجة إلى وكيل السجل

وكيل الصور هو خدمة وسيطة تربط عميل Docker بـ Docker Hub. لا يخزن الصور الفعلية ولكنه يعيد توجيه الطلبات فقط، مما يحل بفعالية:

- مشاكل قيود الوصول إلى الشبكة
- تحسين سرعات تنزيل الصور

Docxy هي خدمة وكيل صور تهدف إلى تجاوز الحصار الشبكي وتسريع تنزيلات الصور عن طريق استضافة وكيل ذاتي.

### حدود استخدام وكيل الصور

يفرض Docker Hub سياسات صارمة لتحديد معدل سحب الصور. عند استخدام خدمة وكيل، تنطبق القيود التالية:

- للمستخدمين غير المصادق عليهم، يُسمح بحد أقصى 10 سحوبات للصور في الساعة لكل عنوان IP.
- للمستخدمين الذين قاموا بتسجيل الدخول باستخدام حساب شخصي، يُسمح بـ 100 سحب للصور في الساعة.
- للاطلاع على حدود أنواع الحسابات الأخرى، يرجى الرجوع إلى الجدول أدناه:

| نوع المستخدم                 | حد معدل السحب           |
| ---------------------------- | ------------------------ |
| Business (authenticated)     | غير محدود                |
| Team (authenticated)         | غير محدود                |
| Pro (authenticated)          | غير محدود                |
| **Personal (authenticated)** | **100/ساعة/حساب**        |
| **Unauthenticated users**    | **10/ساعة/IP**          |

## المبادئ التقنية

ينفذ Docxy وكيلًا كاملاً لواجهة برمجة تطبيقات سجل Docker، ويتطلب فقط إضافة تكوين وكيل في عميل Docker لاستخدامه.

### هندسة النظام

```mermaid
graph TD
    Client[عميل Docker] -->|يرسل الطلب| HttpServer[خادم HTTP]
    
    subgraph "خدمة وكيل صور Docker"
        HttpServer -->|يوجه الطلب| RouterHandler[معالج التوجيه]
        
        RouterHandler -->|/v2/| ChallengeHandler[معالج التحدي<br>proxy_challenge]
        RouterHandler -->|/auth/token| TokenHandler[معالج الرمز المميز<br>get_token]
        RouterHandler -->|/v2/namespace/image/path_type| RequestHandler[معالج الطلب<br>handle_request]
        RouterHandler -->|/health| HealthCheck[فحص الصحة<br>health_check]
        
        ChallengeHandler --> HttpClient
        TokenHandler --> HttpClient
        RequestHandler --> HttpClient
        
    end
    
    HttpClient[عميل HTTP<br>reqwest]
    
    HttpClient -->|طلب المصادقة| DockerAuth[مصادقة Docker<br>auth.docker.io]
    HttpClient -->|طلب الصورة| DockerRegistry[سجل Docker<br>registry-1.docker.io]
```

### تدفق الطلب

```mermaid
sequenceDiagram
    autonumber
    actor Client as عميل Docker
    participant Proxy as وكيل Docxy
    participant Registry as سجل Docker
    participant Auth as خدمة مصادقة Docker
    
    %% معالجة طلب التحدي
    Client->>Proxy: GET /v2/
    Proxy->>+Registry: GET /v2/
    Registry-->>-Proxy: 401 غير مصرح به (WWW-Authenticate)
    Proxy->>Proxy: تعديل رأس WWW-Authenticate، توجيه المجال إلى /auth/token المحلي
    Proxy-->>Client: 401 إرجاع رأس المصادقة المعدل
    
    %% الحصول على الرمز المميز
    Client->>Proxy: GET /auth/token?scope=repository:library/cirros:pull
    Proxy->>+Auth: GET /token?service=registry.docker.io&scope=repository:library/cirros:pull
    Auth-->>-Proxy: 200 إرجاع الرمز المميز
    Proxy-->>Client: 200 إرجاع استجابة الرمز المميز الأصلية
    
    %% طلب بيان الصورة (للتلخيص)
    Client->>Proxy: HEAD /v2/library/cirros/manifests/latest
    Proxy->>+Registry: إعادة توجيه الطلب (مع رؤوس المصادقة والقبول)
    Registry-->>-Proxy: إرجاع تلخيص الصورة
    Proxy-->>Client: إرجاع تلخيص الصورة (مع الاحتفاظ بالرؤوس والحالة الأصلية)

    %% طلب بيانات تعريف الصورة
    Client->>Proxy: GET /v2/library/cirros/manifests/{docker-content-digest}
    Proxy->>+Registry: إعادة توجيه الطلب (مع رؤوس المصادقة والقبول)
    Registry-->>-Proxy: إرجاع بيانات تعريف الصورة
    Proxy-->>Client: إرجاع بيانات تعريف الصورة (مع الاحتفاظ بالرؤوس والحالة الأصلية)

    %% طلب تكوين الصورة ومعلومات الطبقة
    Client->>Proxy: GET /v2/library/cirros/manifests/{digest}
    Proxy->>+Registry: إعادة توجيه الطلب (مع رؤوس المصادقة والقبول)
    Registry-->>-Proxy: إرجاع تكوين الصورة ومعلومات الطبقة للبنية المحددة
    Proxy-->>Client: إرجاع تكوين الصورة ومعلومات الطبقة (مع الاحتفاظ بالرؤوس والحالة الأصلية)

    %% طلب تفاصيل تكوين الصورة
    Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
    Proxy->>+Registry: إعادة توجيه الطلب (مع رؤوس المصادقة والقبول)
    Registry-->>-Proxy: إرجاع تفاصيل تكوين الصورة
    Proxy-->>Client: إرجاع تفاصيل تكوين الصورة (مع الاحتفاظ بالرؤوس والحالة الأصلية)
    
    %% طلب بيانات ثنائية لطبقة الصورة (حلقة لكل طبقة)
    loop لكل طبقة
        Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
        Proxy->>+Registry: إعادة توجيه طلب الكائن الثنائي كبير الحجم
        Registry-->>-Proxy: إرجاع بيانات الكائن الثنائي كبير الحجم
        Proxy-->>Client: تدفق بيانات الكائن الثنائي كبير الحجم مرة أخرى
    end
```

## حلول أخرى

- [Cloudflare Worker لوكيل الصور](https://voxsay.com/posts/china-docker-registry-proxy-guide/): استخدم بحذر، فقد يؤدي ذلك إلى حظر حسابك على Cloudflare.
- [Nginx لوكيل الصور](https://voxsay.com/posts/china-docker-registry-proxy-guide/): هذا يقوم فقط بوكالة `registry-1.docker.io`. لا تزال الطلبات إلى `auth.docker.io` تتم مباشرة، لذلك إذا تم حظر `auth.docker.io` أيضًا، فسيفشل هذا الحل.
