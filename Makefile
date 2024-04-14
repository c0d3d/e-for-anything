
NAME=e-for-anything
WASM_FILE=$(NAME).wasm
JS_FILE=$(NAME).js
HTML_FILE=index.html
FINAL_BUNDLE=$(NAME).zip
WASM_TARGET_OUT=target/wasm32-unknown-unknown/release/e-for-anything.wasm

all: $(FINAL_BUNDLE)

$(WASM_TARGET_OUT): src/main.rs
	cargo build --release --target wasm32-unknown-unknown

imported_assets: $(wildcard assets/*)
	@echo "Make sure to run \`cargo run --features bevy/asset_processor\`"

$(WASM_FILE) $(JS_FILE): $(WASM_TARGET_OUT) imported_assets
	wasm-bindgen --no-typescript --target web \
	    --out-dir ./ \
	    --out-name "$(NAME)" \
	    $<
	cp target/wasm32-unknown-unknown/release/e-for-anything.wasm $(WASM_FILE)
	cp imported_assets/Default/* assets

$(FINAL_BUNDLE): $(WASM_FILE) $(JS_FILE) $(HTML_FILE)
	zip -r $(FINAL_BUNDLE) assets $(JS_FILE) $(WASM_FILE) $(HTML_FILE)

clean:
	@rm -rf $(FINAL_BUNDLE) $(JS_FILE) *.wasm imported_assets
#	@cargo clean

.PHONY: all clean
.NOTPARALLEL: $(WASM_FILE) $(JS_FILE)
