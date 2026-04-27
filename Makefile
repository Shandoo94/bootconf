.PHONY: test build check fmt clippy

IMAGE    := docker.io/library/rust:1.95
CACHEDIR := bootconf-cargo-registry

PODMAN   := podman run --rm \
            --cap-add SYS_ADMIN \
            --user 0 \
            -v "$(PWD)":/usr/src/app \
            -v $(CACHEDIR):/usr/local/cargo/registry \
            -w /usr/src/app \
            $(IMAGE)

test:
	$(PODMAN) cargo test -- --test-threads=1

build:
	$(PODMAN) cargo build

check:
	$(PODMAN) cargo check

fmt:
	$(PODMAN) cargo fmt --check

clippy:
	$(PODMAN) cargo clippy
