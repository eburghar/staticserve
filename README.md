# Static serve

A simple async multithreaded http server for hosting static website. (JAMSTACK)

- upload compressed .tar.zst, stream decompress and deploy without downtime
- configure aliases routes (for vuerouter for example)

TODO:
- serve brotli files if present (fork actix_files)
- automatically compress files to .br on deploy (can be on the archive for now)
- http/2 push support via config files with tracker to avoid pushing same ressources several times
