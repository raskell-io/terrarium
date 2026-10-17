# Terrarium in the browser

The terminal viewer (`terrarium --tui`) running in a web page. The engine and the TUI are
compiled to WebAssembly, and [Ratzilla](https://github.com/ratatui/ratzilla) renders the
TUI's ratatui frames into the page. It is the same code as the terminal client: the app
state, drawing and key handling live in `src/tui/` and are shared.

```sh
./build.sh                                  # builds the crate into pkg/ (needs wasm-pack)
python3 -m http.server 8080 -d .            # then open http://localhost:8080
```

Pick a world with the address hash (`#first_winter`, `#great_valley`, `#exoplanet`,
`#antarctica`, `#mars_colony`, `#space_station`), or press **W** to switch and **R** to
start over. All other keys are the TUI's (press **?**). People act on instinct; there is no
language model in the browser.
