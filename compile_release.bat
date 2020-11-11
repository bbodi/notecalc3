cd frontend-web
wasm-pack build --release --target no-modules
cd ..
cp frontend-web/pkg/frontend_web_bg.wasm assets
cp frontend-web/pkg/frontend_web.js assets