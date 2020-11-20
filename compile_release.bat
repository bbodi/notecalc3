wasm-pack build --release --target no-modules frontend-web
cp frontend-web/pkg/frontend_web_bg.wasm assets
cp frontend-web/pkg/frontend_web.js assets