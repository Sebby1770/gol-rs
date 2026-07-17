PREFIX  ?= /usr/local
BINDIR  ?= $(PREFIX)/bin
CARGO   ?= cargo
TARGET  := target/release/gol

.PHONY: all build release debug run test fmt fmt-check clippy check package clean install uninstall demo headless-demo patterns

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

fmt-check:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

check: fmt-check clippy test
	@echo "all checks passed"

package:
	$(CARGO) package

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
	@$(TARGET) --pattern gosper-glider-gun --width 80 --height 25 --delay 60 --gens 400

headless-demo: release
	@$(TARGET) --pattern blinker --width 9 --height 9 --headless --stop-on-cycle

patterns: release
	@$(TARGET) patterns
