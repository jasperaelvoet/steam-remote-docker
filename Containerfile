FROM archlinux:base AS gamescope-builder

USER root

SHELL ["/usr/bin/bash", "-euxo", "pipefail", "-c"]

RUN pacman-key --init \
    && pacman-key --populate archlinux \
    && pacman -Syy --noconfirm archlinux-keyring \
    && pacman -Syu --noconfirm --needed base-devel sudo \
    && groupadd builder \
    && useradd --create-home --gid builder --shell /bin/bash builder \
    && install -d -m 0755 -o builder -g builder /build /packages \
    && printf 'builder ALL=(ALL) NOPASSWD: /usr/bin/pacman\n' > /etc/sudoers.d/builder \
    && chmod 0440 /etc/sudoers.d/builder

WORKDIR /build
COPY --chown=builder:builder container/gamescope/ ./

USER builder

RUN makepkg --syncdeps --noconfirm --cleanbuild --clean \
    && install -m 0644 gamescope-3.16.26-1.1-x86_64.pkg.tar.zst /packages/gamescope.pkg.tar.zst

FROM archlinux:base AS cursor-shim-builder

USER root

SHELL ["/usr/bin/bash", "-euxo", "pipefail", "-c"]

RUN pacman-key --init \
    && pacman-key --populate archlinux \
    && printf '\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n' >>/etc/pacman.conf \
    && pacman -Syy --noconfirm archlinux-keyring \
    && pacman -Syu --noconfirm --needed gcc-multilib lib32-glibc lib32-gcc-libs

COPY container/cursors/cursor-shim.c /build/cursor-shim.c

# ld.so expands $LIB per ABI, which on Arch is lib for x86-64 and lib32 for i386.
RUN install -d /out/lib /out/lib32 \
    && gcc -m64 -shared -fPIC -O2 -Wall -Wextra -o /out/lib/cursor-shim.so /build/cursor-shim.c \
    && gcc -m32 -shared -fPIC -O2 -Wall -Wextra -o /out/lib32/cursor-shim.so /build/cursor-shim.c

FROM archlinux:base

USER root

LABEL org.opencontainers.image.source="https://github.com/jasperaelvoet/steam-remote-docker" \
      org.opencontainers.image.description="Always-on Steam Remote Play host" \
      org.opencontainers.image.licenses="MIT"

SHELL ["/usr/bin/bash", "-euxo", "pipefail", "-c"]

COPY --from=gamescope-builder /packages/gamescope.pkg.tar.zst /tmp/gamescope.pkg.tar.zst

# Steam is distributed through Arch's multilib repository. All system software
# is installed here; the read-only runtime only updates Steam and games in the
# persistent home.
RUN printf '\nDisableSandbox\n\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n' >> /etc/pacman.conf \
    && pacman-key --init \
    && pacman-key --populate archlinux \
    && pacman -Syy --noconfirm archlinux-keyring \
    && pacman -Syu --noconfirm --needed \
      bash \
      ca-certificates \
      catatonit \
      dbus \
      gamescope \
      iproute2 \
      lib32-alsa-lib \
      lib32-alsa-plugins \
      lib32-libpulse \
      lib32-mesa \
      lib32-pipewire \
      lib32-vulkan-icd-loader \
      lib32-vulkan-radeon \
      libcap \
      libpulse \
      mesa \
      noto-fonts \
      noto-fonts-emoji \
      pipewire \
      pipewire-alsa \
      pipewire-pulse \
      procps-ng \
      steam \
      tar \
      util-linux \
      vulkan-icd-loader \
      vulkan-radeon \
      wireplumber \
      python \
      xdg-utils \
      xorg-xwayland \
    && pacman -U --noconfirm /tmp/gamescope.pkg.tar.zst \
    && groupadd -f input \
    && groupadd -f render \
    && groupadd --gid 1000 steam \
    && useradd --no-create-home --home-dir /mnt/data --uid 1000 --gid 1000 --shell /bin/bash --groups video,input,render steam \
    && setcap cap_sys_nice=ep /usr/bin/gamescope \
    && sed -i 's/^#\(en_US\.UTF-8 UTF-8\)/\1/' /etc/locale.gen \
    && locale-gen \
    && printf 'LANG=en_US.UTF-8\n' > /etc/locale.conf \
    && dbus-uuidgen > /etc/machine-id \
    && install -d -m 0755 -o steam -g steam /mnt/data \
    && install -d -m 0755 /var/lib/dbus \
    && ln -sfn /etc/machine-id /var/lib/dbus/machine-id \
    && rm -f /tmp/gamescope.pkg.tar.zst \
    && rm -rf /var/cache/pacman/pkg/* /var/lib/pacman/sync/*

COPY --from=cursor-shim-builder /out/ /usr/local/lib/steam-remote/
RUN printf '%s\n' '/usr/local/lib/steam-remote/$LIB/cursor-shim.so' >/etc/ld.so.preload
COPY --chmod=0755 container/steam-remote.sh /usr/local/bin/steam-remote

ARG VCS_REF
LABEL org.opencontainers.image.revision="${VCS_REF}"

ENV LANG=en_US.UTF-8 \
    LC_ALL=en_US.UTF-8 \
    AMD_VULKAN_ICD=RADV

VOLUME ["/mnt/data"]

STOPSIGNAL SIGTERM

HEALTHCHECK --interval=30s --timeout=10s --start-period=5m --retries=3 CMD ["/usr/local/bin/steam-remote", "health"]

ENTRYPOINT ["/usr/sbin/catatonit", "-g", "--", "/usr/local/bin/steam-remote"]
CMD ["run"]
