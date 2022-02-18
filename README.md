# staticserve

A simple and fast async http(s) server for hosting static website (JAMSTACK) under kubernetes.

`staticserve` defines an API point (`/upload`) for uploading/updating the website content by posting an archive
(.tar or .tar.zst). It can optionally be protected by a JWT token if `jwt` is defined in the configuration. The
JWKS endpoint is used in that case, to retrieve the public keys and verify the validity of the token received in
the authorization bearer token, and a claims map with expected values can be used to restrict even more who can
upload new content to the server.

This is specially useful in CI/CD pipelines, and you can also define commands to launch after successfully updating
content with the `hooks` section of the configuration file for futher setup or cleanup (like injecting secrets).

The server support [ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag) and
[Last-Modified](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Last-Modified) headers to avoid cache errors
on static files as well as [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control)
header policies based on path prefixes and suffixes. The order of declaration of thoses is preserved and important
as the first match wins. Suffixes are matched before prefixes.

Along with transparent compression, thoses features should give you a 100% score at [PageSpeed Insights
test](https://pagespeed.web.dev/?hl=en).

It also supports SPA applications by serving specific static files based on path expressions (actix route expressions)
that are listed int the `routes` dictionnary. A default page with a default status code can also be provided in
case no file nor a route match.

It uses the [fast rustls](https://jbp.io/2019/07/01/rustls-vs-openssl-performance.html) tls implementation.

## Usage

```
staticserve 0.5.2

Usage: staticserve [-c <config>] [-v] [-a <addr>]

Static file server with ability to upload content and define dynamic routes

Options:
  -c, --config      configuration file containing projects and gitlab connection
                    parameters
  -v, --verbose     more detailed output
  -a, --addr        addr:port to bind to
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
jwt:
  jwks: https://gitlab.com/-/jwks
  # only allow a job from a particular project running on a protected branch or tag to update content
  claims:
    iss: gitlab.com
    project_path: node/blog
    ref_protected: true
routes:
  "/search/category/{category}/{search}": "search/category/_category/_search.html"
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
curl -H 'Authorization: Bearer: $CI_JOB_JWT' -F file=@blog.tar.zst https://host/upload
```
