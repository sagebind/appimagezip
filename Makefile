PREFIX := /usr/local
BIN_NAME := appimagezip
CARGO := cargo
RUSTC_BOOTSTRAP_FLAGS := -C lto -C panic=abort


.PHONY: all
all: release

.PHONY: clean
clean:
	-rm -r bin target

.PHONY: install
install: release
	install -m 0755 target/release/$(BIN_NAME) $(PREFIX)/bin

.PHONY: uninstall
uninstall:
	-rm $(PREFIX)/bin/$(BIN_NAME)

.PHONY: debug
debug: target/debug/$(BIN_NAME)

.PHONY: release
release: target/release/$(BIN_NAME)

bin:
	mkdir bin

target/debug/$(BIN_NAME): $(wildcard src/*.rs) bin/bootstrap
	$(CARGO) build

target/release/$(BIN_NAME): $(wildcard src/*.rs) bin/bootstrap
	$(CARGO) build --release

bin/bootstrap: bin target/release/bootstrap
	cp target/release/bootstrap $@
	strip $@
	printf '\x41\x49\x02' | dd of=$@ bs=1 seek=8 count=3 conv=notrunc

target/release/bootstrap: $(wildcard bootstrap/*.rs)
	$(CARGO) rustc --manifest-path bootstrap/Cargo.toml --release --verbose -- $(RUSTC_BOOTSTRAP_FLAGS)
