.PHONY: test fmt tidy

# The gate. `cargo test` includes rust/tests/interop.rs, which builds
# interop/main.go against github.com/zap-proto/go and drives a REAL Go
# ZAP server from the Rust client — so wire agreement is proved, not
# asserted. That test skips loudly if no Go toolchain is present.
#
# cargo clippy is deliberately NOT here yet: the rustup clippy shim
# fails with "multiple input filenames provided" on this workspace's
# build-dependency. Add it back once that is fixed.
test:
	cd rust && cargo fmt --all -- --check
	cd rust && cargo test --all-targets
	cd rust && cargo test --doc
	go vet ./...

fmt:
	cd rust && cargo fmt --all

tidy:
	go mod tidy
