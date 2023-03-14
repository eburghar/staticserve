# staticserve

A simple and fast asynchronous HTTP(S) server for hosting static website (JAMSTACK) under Kubernetes.

`staticserve` defines an API point (`/upload`) for uploading/updating the website content by posting
an archive (`.tar` or `.tar.zst`). It can optionally be protected by a JWT token if `jwt` is defined in
the configuration. The JWKS endpoint is used in that case, to retrieve the public keys and verify
the validity of the bearer token received in the authorization header, and a claims map with expected
values can be used to restrict even more who can upload content to the server.

This is specially useful in CI/CD pipelines, and you can also define commands to launch after
successfully updating content with the `hooks` section of the configuration file for further setup or
cleanup (like injecting secrets).

The server support [ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag) and [Last-
Modified](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Last-Modified) headers to avoid
cache errors on static files as well as [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) header policies based on path prefixes and suffixes whose order is
preserved and important as the first match wins. Suffixes are matched before prefixes.

Along with transparent compression, those features should give you a 100% score at [PageSpeed
Insights test](https://pagespeed.web.dev/?hl=en).

It also supports SPA applications by serving specific static files based on path expressions (actix
route expressions) that are listed int the `routes` dictionary. A default page with a default
status code can also be provided in case no file nor a route match.

`redirect` in `tls` section of configuration file allows to configure an automatic HTTPS
redirection per protocol to deal with differences in dual stack deployment. Generally you use a
reverse proxy with IPv4 while you expose your service directly with IPv6. Because some reverse
proxy don't handle TLS encrypted upstream you can't send them a redirect, so you can disable
it in that specific case.

When `hsts` is provided in `tls` section, then [HTTP Strict Transport
Security](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Strict-Transport-Security)
are sent according to the parameters.

It uses the [fast rustls](https://jbp.io/2019/07/01/rustls-vs-openssl-performance.html) TLS
implementation.

## Usage

```
staticserve 0.8.0

Usage: staticserve [-c <config>] [-v] [-l <addr>] [-L <addrs>] [-S]

Static file server with ability to upload content and define dynamic routes

Options:
  -c, --config      configuration file containing projects and gitlab connection
                    parameters (/etc/staticserve.yaml)
  -v, --verbose     more detailed output (false)
  -l, --addr        addr:port to bind to (0.0.0.0:8080) without tls
  -L, --addrs       addr:port to bind to (0.0.0.0:8443) when tls is used
  -S, --secure      only bind to tls (when tls config is present in
                    configuration file)
  --help            display usage information
```

## Configuration

Create a `/etc/staticserve.yaml`

```yaml
dir: /var/lib/staticserve
root: dist
default:
  file: 404.html
  status: 404
tls:
  crt: /var/run/secrets/staticserve/tls.crt
  key: /var/run/secrets/staticserve/tls.key
  # perform a redirection to https
  redirect:
    port: 443 # optional
    protocols: both # both | ipv4 | ipv6 | none
  # Strict-Transport-Security header
  hsts:
    duration: 300 # duration in s
    include_subdomains: true
    preload: false
jwt:
  jwks: https://gitlab.com/-/jwks
  # only allows a job from a particular project running on a
  # protected branch or tag to update content
  claims:
    iss: gitlab.com
    project_path: node/blog
    ref_protected: true
routes:
  "/search/category/{category}/{search}": "search/category/_category/_search.html"
  # staticserve does not serve hidden file, but you can map a dotted path to a file.
  # the mimetype sticks to the file extension as a bonus
  "/.well-known/matrix/server": "well-known/matrix/server.json"
cache:
  suffixes:
    'index.html': 'private,max-age=0'
  prefixes:
    '/app_/': 'max-age=259200'
hooks:
  updated:
    - /usr/bin/echo updated
```

`default`, `tls`, `jwt`, `routes` and `cache` are all optional.

## Uploading

```sh
curl -H "Authorization: Bearer $CI_JOB_JWT" -F file=@blog.tar.zst https://host/upload
```