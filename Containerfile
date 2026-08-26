FROM archlinux:base

USER root

LABEL org.opencontainers.image.source="https://github.com/jasperaelvoet/steam-remote-docker" \
      org.opencontainers.image.description="Always-on Steam Remote Play host" \
      org.opencontainers.image.licenses="MIT"

SHELL ["/usr/bin/bash", "-euxo", "pipefail", "-c"]

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
      xdg-utils \
      xorg-xwayland \
    && groupadd -f input \
    && groupadd -f render \
    && groupadd --gid 1000 steam \
    && useradd --no-create-home --home-dir /mnt/data --uid 1000 --gid 1000 --shell /bin/bash --groups video,input,render steam \
    && sed -i 's/^#\(en_US\.UTF-8 UTF-8\)/\1/' /etc/locale.gen \
    && locale-gen \
    && printf 'LANG=en_US.UTF-8\n' > /etc/locale.conf \
    && dbus-uuidgen > /etc/machine-id \
    && install -d -m 0755 -o steam -g steam /mnt/data \
    && install -d -m 0755 /var/lib/dbus \
    && ln -sfn /etc/machine-id /var/lib/dbus/machine-id \
    && rm -rf /var/cache/pacman/pkg/* /var/lib/pacman/sync/*

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
