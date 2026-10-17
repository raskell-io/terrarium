# Terrarium on the web

The simulation engine compiled to WebAssembly, with a browser front end in plain HTML, CSS
and JavaScript. Everything runs on the page; people act on instinct (no language model).

```sh
./build.sh                                  # builds engine/ into pkg/ (needs wasm-pack)
python3 -m http.server 8080 -d .            # then open http://localhost:8080
```

- `engine/`: the `terrarium-web` crate, a wasm-bindgen wrapper around the engine
  (`Simulation`, `step`, `world`, `frame`)
- `index.html`, `css/`: the title and game screens; `site.css` is the shared layout,
  `classic.css` and `premium.css` are the two looks
- `js/main.js`: boot, day loop, panels and controls
- `js/classic.js`: the classic renderer (8×8 CP437-style bitmap glyphs, VGA palette)
- `js/premium.js`: the premium renderer (16×16 pixel-art sprites, generated ground)
