# chess-driller

Drill opening repetoires from PGN files. Continue from the opening playing
against an engine.

## Run

Install Node.js, Rust, and the [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/).
The Rust crate and JavaScript API use Tauri 2.1; keep their minor versions aligned.
The Rust package uses edition 2021 and does not declare a minimum Rust version.
Native window drag-and-drop is disabled in the Tauri config so the chessboard
can receive HTML drag events, including on Windows.

```sh
npm install
npm run tauri dev
```

To build the desktop app without creating an installer:

```sh
npm run tauri -- build --no-bundle
```

## Database

For testing I've made a simple sample database from random chapters from the
following
lichess studies:

For white:

* [Vienna Gambit: Main Line](https://lichess.org/study/ePMOV5k4)

For black:

* [Caro-Kann Advance Variation 1](https://lichess.org/study/VJb8YgoJ)
* [Queens Gambit Declined (Variation 1+2)](https://lichess.org/study/rMrAjlAG)

## Plan

* Have a folder with PGN files of planned opening repetoire (divided into black
and white)
* Test you on random opening
* Once reaches end of prep chess engine takes over
* Generally spot holes in your opening, and also target prep based on game
history

## License

This project is currently licensed under GPL-v3.
