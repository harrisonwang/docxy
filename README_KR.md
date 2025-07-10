# Docxy

![og-image](og-image.png)

[![English](https://img.shields.io/badge/English-Click-orange)](README_EN.md)
[![简体中文](https://img.shields.io/badge/简体中文-点击查看-blue)](README.md)
[![Русский](https://img.shields.io/badge/Русский-Нажмите-orange)](README_RU.md)
[![Español](https://img.shields.io/badge/Español-Clic-blue)](README_ES.md)
[![한국어](https://img.shields.io/badge/한국어-클릭-orange)](README_KR.md)
[![العربية](https://img.shields.io/badge/العربية-انقر-blue)](README_AR.md)
[![Türkçe](https://img.shields.io/badge/Türkçe-Tıkla-orange)](README_TR.md)

Docker Hub에 대한 제한된 액세스 문제를 해결하기 위해 설계된 경량 Docker 이미지 프록시 서비스입니다.

> 📢 **블로그 튜토리얼:** [**Docker Hub 연결 시간 초과에 작별을 고하세요! Docxy로 나만의 이미지 가속기 구축하기**](https://voxsay.com/posts/docxy-docker-proxy-tutorial-for-china/)

## 핵심 기능

*   🚀 **원클릭 배포**: `install.sh` 자동화 스크립트를 제공하여 환경 설정, 인증서 신청 (Let's Encrypt), 서비스 배포를 한 번의 클릭으로 완료할 수 있어 수동 개입이 필요 없습니다.

*   📦 **다중 배포 모드**:
    *   **독립 실행**: 내장 TLS 기능으로 HTTPS 서비스를 직접 제공합니다.
    *   **Nginx 프록시**: Nginx와 함께 백엔드 서비스로 작동할 수 있습니다.
    *   **CDN 원본**: HTTP 모드를 지원하여 CDN 통합에 편리합니다.

*   ⚡ **로그인으로 풀 속도 증가**: `docker login`을 통해 개인 Docker Hub 계정으로 인증하여 익명 사용자 (IP당 시간당 10회 풀)의 풀 속도 제한을 인증된 사용자 (계정당 시간당 100회 풀)로 증가시킬 수 있습니다.

*   💎 **완전히 투명한 프록시**: Docker Registry V2 API와 완벽하게 호환됩니다. 클라이언트는 미러 소스 주소만 수정하면 되므로 추가 학습 곡선이나 사용 습관 변경이 필요 없습니다.

*   🛡️ **고성능 및 보안**: **Rust** 및 **Actix Web**으로 구축되어 뛰어난 성능과 메모리 안전성을 제공합니다. 이미지 전송에 스트리밍을 사용하여 오버헤드가 최소화됩니다.

## 설치 및 배포

배포 프로세스를 단순화하기 위해 원클릭 설치 스크립트를 제공합니다. 시작하기 전에 도메인 이름이 대상 호스트로 확인되었는지 확인하십시오.

```bash
bash <(curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/install.sh)
```

스크립트는 설치 과정을 안내하며 다음 세 가지 배포 모드를 제공합니다.

---

### 모드 1: 독립 실행 (HTTPS)

가장 간단하고 권장되는 모드입니다. Docxy는 80번 및 443번 포트에서 직접 수신 대기하며 완전한 HTTPS 프록시 서비스를 제공합니다.

**특징:**
- 추가 웹 서버 구성이 필요 없습니다.
- HTTP에서 HTTPS로의 리디렉션을 자동으로 처리합니다.
- Let's Encrypt 인증서를 자동으로 신청하거나 자체 인증서를 사용할 수 있습니다.

**설치 과정:**
1.  원클릭 설치 스크립트를 실행합니다.
2.  모드 선택 프롬프트에서 `1`을 입력하거나 Enter 키를 누릅니다.
3.  프롬프트에 따라 도메인 이름을 입력하고 인증서 처리 방법을 선택합니다.
4.  스크립트가 모든 구성을 자동으로 완료하고 서비스를 시작합니다.

---

<details>
<summary>모드 2: Nginx 역방향 프록시 (고급)</summary>

### 모드 2: Nginx 역방향 프록시

이 모드는 Nginx를 이미 가지고 있고 이를 통해 웹 서비스를 중앙에서 관리하려는 경우에 적합합니다.

**특징:**
- Nginx가 HTTPS 암호화 및 인증서 관리를 처리하며, Docxy는 일반 HTTP 백엔드로 실행됩니다.
- Docxy는 지정된 포트 (예: 9000)에서 백엔드 HTTP 서비스로 실행됩니다.
- 다른 서비스와의 통합에 편리합니다.

**설치 과정:**
1.  원클릭 설치 스크립트를 실행합니다.
2.  모드 선택 프롬프트에서 `2`를 입력합니다.
3.  프롬프트에 따라 도메인 이름, Docxy 백엔드 수신 대기 포트 및 인증서 정보를 입력합니다.
4.  스크립트가 Nginx 구성 파일 예시를 자동으로 생성합니다. 이를 Nginx 구성에 수동으로 추가하고 Nginx 서비스를 다시 로드해야 합니다.

</details>

---

<details>
<summary>모드 3: CDN 원본 (HTTP) (고급)</summary>

### 모드 3: CDN 원본 (HTTP)

이 모드는 Docxy를 CDN의 원본으로 사용하여 더 나은 전역 가속을 달성하려는 경우에 적합합니다.

**특징:**
- Docxy는 HTTP 포트에서만 수신 대기합니다.
- CDN 공급자가 HTTPS 요청 및 인증서를 처리합니다.
- Docxy는 클라이언트 IP 및 프로토콜을 올바르게 식별하기 위해 `X-Forwarded-*` 헤더를 신뢰하고 처리합니다.

**설치 과정:**
1.  원클릭 설치 스크립트를 실행합니다.
2.  모드 선택 프롬프트에서 `3`을 입력합니다.
3.  프롬프트에 따라 Docxy가 수신 대기해야 하는 HTTP 포트를 입력합니다.
4.  CDN 서비스를 구성하여 원본을 Docxy 서비스 주소 및 포트로 지정합니다.

</details>


## Docker 클라이언트 사용

프록시 서비스를 사용하도록 Docker 클라이언트를 구성합니다.

### 방법 1: 익명 사용 (기본 구성)

이것은 Docker의 기본 요청을 프록시 서비스로 지정하는 가장 기본적인 구성입니다.

1.  **Docker 데몬 구성**

    `/etc/docker/daemon.json` 파일을 편집하고 (없으면 생성) 다음 내용을 추가합니다. `your-domain.com`을 도메인 이름으로 바꿉니다.

    ```json
    {
      "registry-mirrors": ["https://your-domain.com"]
    }
    ```

2.  **Docker 서비스 재시작**

    ```bash
    sudo systemctl restart docker
    ```
    이제 `docker pull`은 프록시를 통해 이미지를 풀합니다.

<details>
<summary>방법 2: 로그인 사용 (풀 속도 증가)</summary>

이 방법은 익명 사용 외에도 Docker Hub 계정으로 로그인하여 더 높은 이미지 풀 속도를 얻을 수 있습니다.

1.  **기본 구성 완료**

    **방법 1**의 모든 단계를 완료했는지 확인하십시오.

2.  **프록시 서비스에 로그인**

    `docker login` 명령을 사용하고 Docker Hub 사용자 이름과 암호를 입력합니다.

    ```bash
    docker login your-domain.com
    ```

3.  **인증 정보 동기화**

    로그인 성공 후 `~/.docker/config.json` 파일을 수동으로 편집해야 합니다. `your-domain.com`에 대해 생성된 `auth` 정보를 `https://index.docker.io/v1/`에 복사하여 붙여넣습니다.

    수정 전:
    ```json
    {
        "auths": {
            "your-domain.com": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            }
        }
    }
    ```

    수정 후:
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
    파일을 저장하면 `docker pull` 요청이 인증된 사용자로 전송되어 더 높은 속도 제한을 누릴 수 있습니다.

</details>

## 개발

> [!NOTE]
> 자세한 기술 배경, 시스템 아키텍처 및 구현 원리는 [**기술 아키텍처 및 원리 문서**](docs/ARCHITECTURE.md)를 참조하십시오.

1.  **리포지토리 복제**
    ```bash
    git clone https://github.com/harrisonwang/docxy.git
    cd docxy
    ```

2.  **구성 파일 수정**
    `config/default.toml`을 열고 `[server]` 섹션을 수정하여 HTTP 서비스가 활성화되고 HTTPS 서비스가 비활성화되었는지 확인합니다. 개발 환경에서 권한 있는 포트를 사용하지 않도록 포트를 8080으로 설정할 수 있습니다.

    ```toml
    # config/default.toml

    [server]
    http_port = 8080      # 비권한 포트 사용
    https_port = 8443
    http_enabled = true   # HTTP 활성화
    https_enabled = false # HTTPS 비활성화
    behind_proxy = true
    ```

3.  **프로젝트 실행**
    이제 `cargo`로 프로젝트를 직접 실행할 수 있습니다.
    ```bash
    cargo run
    ```
    서비스가 시작되어 `http://0.0.0.0:8080`에서 수신 대기합니다.

4.  **릴리스 버전 빌드**
    ```bash
    cargo build --release
    ```

## 라이선스

이 프로젝트는 MIT 라이선스에 따라 라이선스가 부여됩니다. 자세한 내용은 [LICENSE](LICENSE)를 참조하십시오.