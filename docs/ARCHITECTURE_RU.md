# Docxy Техническая архитектура и принципы

Этот документ подробно описывает фон, технические принципы, системную архитектуру и процесс реализации проекта Docxy.

## Фон

### Введение в реестры образов Docker

Реестр образов Docker — это служба для хранения и распространения образов контейнеров Docker, обеспечивающая централизованное хранение для контейнеризированных приложений. Эти реестры позволяют разработчикам отправлять, хранить, управлять и извлекать образы контейнеров, упрощая процесс распространения и развертывания приложений.

### Типы реестров образов

- **Официальный реестр**: Docker Hub, официальный реестр, поддерживаемый Docker, Inc.
- **Сторонние автономные реестры**: Такие как AWS ECR, Google GCR, Aliyun ACR и т. д., используемые для публикации и обмена проприетарными образами.
- **Зеркальные службы**: Такие как зеркальный сайт TUNA в Университете Цинхуа, ускоритель зеркал Aliyun и т. д., которые обеспечивают ускорение для Docker Hub.

> [!NOTE]
> Из-за сетевых ограничений прямой доступ к Docker Hub из материкового Китая затруднен, и большинство зеркальных служб прекратили свою работу.

### Зачем нужен прокси-сервер реестра

Прокси-сервер образов — это промежуточная служба, которая соединяет клиент Docker с Docker Hub. Он не хранит фактические образы, а только перенаправляет запросы, эффективно решая:

- Проблемы с ограничениями доступа к сети
- Увеличение скорости загрузки образов

Docxy — это такая служба прокси-сервера образов, целью которой является обход сетевых блокировок и ускорение загрузки образов путем самостоятельного размещения прокси-сервера.

### Ограничения использования прокси-сервера образов

Docker Hub налагает строгие политики ограничения скорости на извлечение образов. При использовании прокси-сервера применяются следующие ограничения:

- Для неаутентифицированных пользователей разрешено не более 10 извлечений образов в час на IP-адрес.
- Для пользователей, вошедших в систему с личной учетной записью, разрешено 100 извлечений образов в час.
- Ограничения для других типов учетных записей см. в таблице ниже:

| Тип пользователя             | Ограничение скорости извлечения |
| ---------------------------- | ------------------------------- |
| Business (authenticated)     | Без ограничений                 |
| Team (authenticated)         | Без ограничений                 |
| Pro (authenticated)          | Без ограничений                 |
| **Personal (authenticated)** | **100/час/учетная запись**      |
| **Unauthenticated users**    | **10/час/IP**                   |

## Технические принципы

Docxy реализует полный прокси для Docker Registry API, для использования требуется только добавление конфигурации прокси в клиент Docker.

### Системная архитектура

```mermaid
graph TD
    Client[Клиент Docker] -->|Отправляет запрос| HttpServer[HTTP-сервер]
    
    subgraph "Служба прокси-сервера образов Docker"
        HttpServer -->|Маршрутизирует запрос| RouterHandler[Обработчик маршрутов]
        
        RouterHandler -->|/v2/| ChallengeHandler[Обработчик вызовов<br>proxy_challenge]
        RouterHandler -->|/auth/token| TokenHandler[Обработчик токенов<br>get_token]
        RouterHandler -->|/v2/namespace/image/path_type| RequestHandler[Обработчик запросов<br>handle_request]
        RouterHandler -->|/health| HealthCheck[Проверка работоспособности<br>health_check]
        
        ChallengeHandler --> HttpClient
        TokenHandler --> HttpClient
        RequestHandler --> HttpClient
        
    end
    
    HttpClient[HTTP-клиент<br>reqwest]
    
    HttpClient -->|Запрос аутентификации| DockerAuth[Аутентификация Docker<br>auth.docker.io]
    HttpClient -->|Запрос образа| DockerRegistry[Реестр Docker<br>registry-1.docker.io]
```

### Поток запросов

```mermaid
sequenceDiagram
    autonumber
    actor Client as Клиент Docker
    participant Proxy as Docxy Proxy
    participant Registry as Реестр Docker
    participant Auth as Служба аутентификации Docker
    
    %% Обработка запроса вызова
    Client->>Proxy: GET /v2/
    Proxy->>+Registry: GET /v2/
    Registry-->>-Proxy: 401 Unauthorized (WWW-Authenticate)
    Proxy->>Proxy: Изменить заголовок WWW-Authenticate, указать realm на локальный /auth/token
    Proxy-->>Client: 401 Возвращает измененный заголовок аутентификации
    
    %% Получение токена
    Client->>Proxy: GET /auth/token?scope=repository:library/cirros:pull
    Proxy->>+Auth: GET /token?service=registry.docker.io&scope=repository:library/cirros:pull
    Auth-->>-Proxy: 200 Возвращает токен
    Proxy-->>Client: 200 Возвращает исходный ответ токена
    
    %% Запрос манифеста образа (для дайджеста)
    Client->>Proxy: HEAD /v2/library/cirros/manifests/latest
    Proxy->>+Registry: Перенаправить запрос (с заголовками аутентификации и Accept)
    Registry-->>-Proxy: Возвращает дайджест образа
    Proxy-->>Client: Возвращает дайджест образа (сохраняя исходные заголовки и статус)

    %% Запрос метаданных образа
    Client->>Proxy: GET /v2/library/cirros/manifests/{docker-content-digest}
    Proxy->>+Registry: Перенаправить запрос (с заголовками аутентификации и Accept)
    Registry-->>-Proxy: Возвращает метаданные образа
    Proxy-->>Client: Возвращает метаданные образа (сохраняя исходные заголовки и статус)

    %% Запрос конфигурации образа и информации о слоях
    Client->>Proxy: GET /v2/library/cirros/manifests/{digest}
    Proxy->>+Registry: Перенаправить запрос (с заголовками аутентификации и Accept)
    Registry-->>-Proxy: Возвращает конфигурацию образа и информацию о слоях для указанной архитектуры
    Proxy-->>Client: Возвращает конфигурацию образа и информацию о слоях (сохраняя исходные заголовки и статус)

    %% Запрос подробной информации о конфигурации образа
    Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
    Proxy->>+Registry: Перенаправить запрос (с заголовками аутентификации и Accept)
    Registry-->>-Proxy: Возвращает подробную информацию о конфигурации образа
    Proxy-->>Client: Возвращает подробную информацию о конфигурации образа (сохраняя исходные заголовки и статус)
    
    %% Запрос двоичных данных каждого слоя образа (цикл для каждого слоя)
    loop Для каждого слоя
        Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
        Proxy->>+Registry: Перенаправить запрос blob
        Registry-->>-Proxy: Возвращает данные blob
        Proxy-->>Client: Потоковая передача данных blob обратно
    end
```

## Другие решения

- [Cloudflare Worker для прокси образов](https://voxsay.com/posts/china-docker-registry-proxy-guide/): Используйте с осторожностью, так как это может привести к блокировке вашей учетной записи Cloudflare.
- [Nginx для прокси образов](https://voxsay.com/posts/china-docker-registry-proxy-guide/): Это проксирует только `registry-1.docker.io`. Запросы к `auth.docker.io` по-прежнему выполняются напрямую, поэтому, если `auth.docker.io` также заблокирован, это решение не будет работать.
