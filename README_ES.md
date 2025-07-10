# Docxy

![og-image](og-image.png)

[![English](https://img.shields.io/badge/English-Click-orange)](README_EN.md)
[![简体中文](https://img.shields.io/badge/简体中文-点击查看-blue)](README.md)
[![Русский](https://img.shields.io/badge/Русский-Нажмите-orange)](README_RU.md)
[![Español](https://img.shields.io/badge/Español-Clic-blue)](README_ES.md)
[![한국어](https://img.shields.io/badge/한국어-클릭-orange)](README_KR.md)
[![العربية](https://img.shields.io/badge/العربية-انقر-blue)](README_AR.md)
[![Türkçe](https://img.shields.io/badge/Türkçe-Tıkla-orange)](README_TR.md)

Un servicio ligero de proxy de imágenes Docker, diseñado para resolver el problema del acceso restringido a Docker Hub en China continental.

> 📢 **Tutorial del Blog:** [**¡Diga adiós a los tiempos de espera de conexión de Docker Hub! Construya su acelerador de imágenes exclusivo con Docxy**](https://voxsay.com/posts/docxy-docker-proxy-tutorial-for-china/)

## Características Principales

*   🚀 **Despliegue con un Clic**: Proporciona un script de automatización `install.sh` para la configuración del entorno, la aplicación de certificados (Let's Encrypt) y el despliegue del servicio con un solo clic, sin necesidad de intervención manual.

*   📦 **Múltiples Modos de Despliegue**:
    *   **Autónomo**: Funcionalidad TLS incorporada, proporciona directamente el servicio HTTPS.
    *   **Proxy Nginx**: Puede trabajar con Nginx como un servicio de backend.
    *   **Origen CDN**: Soporta el modo HTTP, conveniente para la integración con CDN.

*   ⚡ **Inicio de Sesión para Mayor Tasa de Extracción**: Permite a los usuarios autenticarse con sus cuentas personales de Docker Hub a través de `docker login`, aumentando el límite de tasa de extracción de usuarios anónimos (10 extracciones/hora/IP) a usuarios autenticados (100 extracciones/hora/cuenta).

*   💎 **Proxy Completamente Transparente**: Totalmente compatible con la API de Docker Registry V2. Los clientes solo necesitan modificar la dirección de la fuente del espejo, sin curva de aprendizaje adicional ni cambios en los hábitos de uso.

*   🛡️ **Alto Rendimiento y Seguridad**: Construido con **Rust** y **Actix Web**, ofreciendo un excelente rendimiento y seguridad de memoria. Utiliza la transmisión por secuencias para la transferencia de imágenes, con una sobrecarga mínima.

## Instalación y Despliegue

Proporcionamos un script de instalación con un solo clic para simplificar el proceso de despliegue. Antes de comenzar, asegúrese de que su nombre de dominio esté resuelto al host de destino.

```bash
bash <(curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/install.sh)
```

El script le guiará a través de la instalación y ofrece los siguientes tres modos de despliegue:

---

### Modo Uno: Autónomo (HTTPS)

Este es el modo más simple y recomendado. Docxy escuchará directamente en los puertos 80 y 443, proporcionando un servicio de proxy HTTPS completo.

**Características:**
- No necesita configuración adicional del servidor web.
- Maneja automáticamente la redirección de HTTP a HTTPS.
- Opción de solicitar automáticamente certificados Let's Encrypt o usar sus propios certificados.

**Proceso de Instalación:**
1.  Ejecute el script de instalación con un solo clic.
2.  Cuando se le solicite la selección de modo, ingrese `1` o simplemente presione Enter.
3.  Siga las indicaciones para ingresar su nombre de dominio y elija el método de manejo de certificados.
4.  El script completará automáticamente todas las configuraciones e iniciará el servicio.

---

<details>
<summary>Modo Dos: Proxy Inverso Nginx (Avanzado)</summary>

### Modo Dos: Proxy Inverso Nginx

Este modo es adecuado si ya tiene Nginx y desea administrar los servicios web de forma centralizada a través de él.

**Características:**
- Nginx maneja el cifrado HTTPS y la gestión de certificados, con Docxy ejecutándose como un backend HTTP simple.
- Docxy se ejecuta como un servicio HTTP de backend en un puerto especificado (por ejemplo, 9000).
- Conveniente para la integración con otros servicios.

**Proceso de Instalación:**
1.  Ejecute el script de instalación con un solo clic.
2.  Cuando se le solicite la selección de modo, ingrese `2`.
3.  Siga las indicaciones para ingresar su nombre de dominio, el puerto de escucha del backend de Docxy y la información del certificado.
4.  El script generará automáticamente un archivo de configuración de Nginx de ejemplo para usted. Deberá agregarlo manualmente a su configuración de Nginx y recargar el servicio de Nginx.

</details>

---

<details>
<summary>Modo Tres: Origen CDN (HTTP) (Avanzado)</summary>

### Modo Tres: Origen CDN (HTTP)

Este modo es adecuado si desea utilizar Docxy como origen para una CDN para lograr una mejor aceleración global.

**Características:**
- Docxy solo escucha en puertos HTTP.
- El proveedor de CDN maneja las solicitudes HTTPS y los certificados.
- Docxy confía y procesa los encabezados `X-Forwarded-*` para identificar correctamente la IP del cliente y el protocolo.

**Proceso de Instalación:**
1.  Ejecute el script de instalación con un solo clic.
2.  Cuando se le solicite la selección de modo, ingrese `3`.
3.  Siga las indicaciones para ingresar el puerto HTTP en el que Docxy debe escuchar.
4.  Configure su servicio CDN para que apunte su origen a la dirección y puerto del servicio Docxy.

</details>


## Uso del Cliente Docker

Configure su cliente Docker para usar su servicio de proxy.

### Método Uno: Uso Anónimo (Configuración Básica)

Esta es la configuración básica, que apunta las solicitudes predeterminadas de Docker a su servicio de proxy.

1.  **Configurar el Demonio Docker**

    Edite el archivo `/etc/docker/daemon.json` (créelo si no existe) y agregue el siguiente contenido. Reemplace `your-domain.com` con su nombre de dominio.

    ```json
    {
      "registry-mirrors": ["https://your-domain.com"]
    }
    ```

2.  **Reiniciar el Servicio Docker**

    ```bash
    sudo systemctl restart docker
    ```
    Ahora, `docker pull` extraerá imágenes a través de su proxy.

<details>
<summary>Método Dos: Uso con Inicio de Sesión (Mayor Tasa de Extracción)</summary>

Este método le permite obtener una mayor tasa de extracción de imágenes iniciando sesión con su cuenta de Docker Hub, además del uso anónimo.

1.  **Completar la Configuración Básica**

    Asegúrese de haber completado todos los pasos del **Método Uno**.

2.  **Iniciar Sesión en el Servicio de Proxy**

    Use el comando `docker login` e ingrese su nombre de usuario y contraseña de Docker Hub.

    ```bash
    docker login your-domain.com
    ```

3.  **Sincronizar la Información de Autenticación**

    Después de iniciar sesión correctamente, debe editar manualmente el archivo `~/.docker/config.json`. Copie la información `auth` generada para `your-domain.com` y péguela para `https://index.docker.io/v1/`.

    Antes de la modificación:
    ```json
    {
        "auths": {
            "your-domain.com": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            }
        }
    }
    ```

    Después de la modificación:
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
    Después de guardar el archivo, sus solicitudes `docker pull` se enviarán como un usuario autenticado, disfrutando así de límites de tasa más altos.

</details>

## Desarrollo

> [!NOTE]
> Para obtener información técnica detallada, arquitectura del sistema y principios de implementación, consulte el [**Documento de Arquitectura Técnica y Principios**](docs/ARCHITECTURE.md).

1.  **Clonar Repositorio**
    ```bash
    git clone https://github.com/harrisonwang/docxy.git
    cd docxy
    ```

2.  **Modificar Archivo de Configuración**
    Abra `config/default.toml` y modifique la sección `[server]` para asegurarse de que el servicio HTTP esté habilitado y el servicio HTTPS esté deshabilitado. Puede establecer el puerto en 8080 para evitar el uso de puertos privilegiados en el entorno de desarrollo.

    ```toml
    # config/default.toml

    [server]
    http_port = 8080      # Usar puerto no privilegiado
    https_port = 8443
    http_enabled = true   # Habilitar HTTP
    https_enabled = false # Deshabilitar HTTPS
    behind_proxy = true
    ```

3.  **Ejecutar Proyecto**
    Ahora, puede ejecutar el proyecto directamente con `cargo`.
    ```bash
    cargo run
    ```
    El servicio se iniciará y escuchará en `http://0.0.0.0:8080`.

4.  **Construir Versión de Lanzamiento**
    ```bash
    cargo build --release
    ```

## Licencia

Este proyecto está bajo la licencia MIT. Consulte [LICENSE](LICENSE) para obtener más información.