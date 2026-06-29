FROM ubuntu:26.04

ARG DEBIAN_FRONTEND=noninteractive

# Ubuntu 26.04 substituiu libasound2 por libasound2t64.
# O metapacote abaixo mantém compatibilidade com desktop-lite.
RUN apt-get update \
    && apt-get install -y --no-install-recommends libasound2t64 \
    && mkdir -p /tmp/libasound2-compat/DEBIAN \
    && printf '%s\n' \
        'Package: libasound2' \
        'Version: 99:1.2.99' \
        'Section: libs' \
        'Priority: optional' \
        'Architecture: all' \
        'Depends: libasound2t64' \
        'Maintainer: Nocky Codespaces test' \
        'Description: Compatibility package for desktop-lite on Ubuntu 26.04' \
        > /tmp/libasound2-compat/DEBIAN/control \
    && dpkg-deb --build /tmp/libasound2-compat /tmp/libasound2.deb \
    && apt-get install -y /tmp/libasound2.deb \
    && rm -rf /var/lib/apt/lists/* /tmp/libasound2*
