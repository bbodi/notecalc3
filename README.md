<h1>
    <img src="https://bbodi.github.io/notecalc3/assets/logo.png"
         alt="logo" width="300"/>
</h1>

![Example GIF](assets/simple.gif)

## What is NoteCalc?
NoteCalc is a handy notepad with a smart builtin calculator.

Honestly, it just tries to be a free Soulver alternative in your browser.

[Features](https://bbodi.github.io/notecalc3/)

[Try out](https://bbodi.github.io/notecalc3/notecalc#)

## Run locally

First, you will need to make sure that the following tools are installed:
1. `rustup`. Instructions [here](https://rustup.rs/)
2. `npm`. Instructions [here](https://www.npmjs.com/get-npm)
3. `wasm-pack`. Instructions [here](https://rustwasm.github.io/wasm-pack/installer/)
4. `serve`. Simply run:
   ```sh
   npm install -g serve
   ```

Once the above tools are installed, you can run:
```sh
git clone https://github.com/bbodi/notecalc3.git
./compile_and_run.bat
```

Then, open your browser and go to  [http://localhost:5000/notecalc]().

## Libraries used
Huge thanks for the following libraries
- https://mathjs.org/
- https://crates.io/crates/rust-decimal
- https://crates.io/crates/base64
- https://crates.io/crates/flate2
- https://crates.io/crates/web-sys
- https://crates.io/crates/wasm-bindgen


 