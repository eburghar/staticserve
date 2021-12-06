# staticserve

A simple and fast async http(s) server for hosting static website (JAMSTACK) under kubernetes.

`staticserve` optionally defines an API point (`/upload`) protected by a JWT token for uploading/updating the website content
by posting an archive (.tar or .tar.zst). A JWKS endpoint is used to retrieve the public keys needed to verify
the validity of the token, and a claims map with expected values can be used to restrict even more who can upload
new content to the server. This is specially useful in CI/CD pipelines.

The server support [ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag) and
[Last-Modified](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Last-Modified) headers to avoid cache errors
on static files as well as [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control)
header policies based on path prefixes and suffixes. Along with transparent compression, you should achieve a 100% score
at [PageSpeed Insights test](https://pagespeed.web.dev/?hl=en).

It also supports SPA applications by serving specific static files based on path expressions (actix route expressions)
that are listed int the `routes` dictionnary. A default page with a default status code can also be provided
in case no file nor a route match.

It uses the [fast rustls](https://jbp.io/2019/07/01/rustls-vs-openssl-performance.html) tls implementation.

## Usage

```
staticserve 0.5.0

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
  prefixes:
    '/app_/': 'max-age=259200'
  suffixes:
    'index.html': 'private,max-age=0'
```

`default`, `tls`, `jwt`, `routes` and `cache` are all optional.

## Uploading

```sh
curl -H 'Authorization: Bearer: $CI_JOB_JWT' -F file=@blog.tar.zst https://host/upload
```
