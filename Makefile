PODMAN ?= podman
SYSTEMCTL ?= systemctl
JOURNALCTL ?= journalctl
IMAGE ?= localhost/steam-remote-docker:latest
CONTAINER ?= steam-remote
SERVICE ?= steam-remote.service
CONTAINERFILE ?= build/container/Containerfile
QUADLET ?= deploy/steam-remote.container
QUADLET_DIR ?= /etc/containers/systemd
VCS_REF ?= $(shell git rev-parse --verify HEAD 2>/dev/null || printf unknown)

.PHONY: build install-quadlet start stop restart logs service-status status health shell admin-start admin-stop admin-status check

build:
	$(PODMAN) build --platform linux/amd64 --build-arg VCS_REF="$(VCS_REF)" --tag "$(IMAGE)" --file "$(CONTAINERFILE)" .

install-quadlet:
	install -D -m 0644 "$(QUADLET)" "$(QUADLET_DIR)/steam-remote.container"
	$(SYSTEMCTL) daemon-reload

start:
	$(SYSTEMCTL) start "$(SERVICE)"

stop:
	$(SYSTEMCTL) stop "$(SERVICE)"

restart:
	$(SYSTEMCTL) restart "$(SERVICE)"

logs:
	$(JOURNALCTL) -fu "$(SERVICE)"

service-status:
	$(SYSTEMCTL) --no-pager --full status "$(SERVICE)"

status:
	$(PODMAN) exec "$(CONTAINER)" steam-remote status

health:
	$(PODMAN) exec "$(CONTAINER)" steam-remote health

shell:
	$(PODMAN) exec -it "$(CONTAINER)" bash

admin-start:
	$(PODMAN) exec "$(CONTAINER)" steam-remote admin start

admin-stop:
	$(PODMAN) exec "$(CONTAINER)" steam-remote admin stop

admin-status:
	$(PODMAN) exec "$(CONTAINER)" steam-remote admin status

check:
	./scripts/check
