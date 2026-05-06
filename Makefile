PREFIX  ?= /usr/local
BINDIR  ?= $(PREFIX)/bin
CARGO   ?= cargo
TARGET  := target/release/gol

.PHONY: all build release debug run test fmt clippy check clean install uninstall demo

all: release

build: release

release:
	$(CARGO) build --release

debug:
	$(CARGO) build

run:
	$(CARGO) run --release --

test:
	$(CARGO) test

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --release -- -D warnings

check: fmt clippy test
	@echo "all checks passed"

clean:
	$(CARGO) clean

install: release
	@install -d "$(DESTDIR)$(BINDIR)"
	@install -m 0755 "$(TARGET)" "$(DESTDIR)$(BINDIR)/gol"
	@echo "installed gol to $(DESTDIR)$(BINDIR)/gol"

uninstall:
	@rm -f "$(DESTDIR)$(BINDIR)/gol"
	@echo "removed $(DESTDIR)$(BINDIR)/gol"

demo: release
	@$(TARGET) --pattern gosper --width 80 --height 25 --delay 60 --gens 400
