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

## Practice

1. Download a study or repertoire as PGN, including its variations.
2. Choose White or Black and use **Import PGN files**. You can import multiple
   files at once. Each successfully imported file is selected automatically.
3. Select all or a subset of those files and click **Start drill**. Colour and
   selection changes apply when you start a new drill.
4. Play your moves on the board. Opponent replies are chosen uniformly from the
   available moves in the selected prep. Matching filenames and chapter names
   appear below the board.
5. If you deviate, the app shows the prepared alternatives. **Retry move** restores
   the position before the deviation; **Continue with Stockfish** keeps your move.
6. At the end of a prepared line, Stockfish takes over automatically.

Imports are copied into `prep.json` in Tauri's application data directory for
`com.chess-driller.app`. The original files are not modified. Importing the same
filename again creates a separate entry. Standard chess PGNs starting from the
initial position are supported, including multiple chapters and nested variations.
Custom FEN starts and other chess variants are rejected with an error.

Shared move prefixes retain every contributing file and chapter identity. Only
selected files contribute moves to a drill. Different move orders remain separate
lines; the app does not merge transpositions. The move list is shown during play.
Checkmate, stalemate and insufficient material end the game; draw claims based on
repetition or the fifty-move rule are not yet exposed in the UI.

## Local Stockfish

Download [Stockfish](https://stockfishchess.org/download/) separately. Set the
**Executable name or full path** field to its executable (for example,
`/usr/games/stockfish`), or leave `stockfish` if it is on your PATH. This setting
is remembered locally. No cloud engine or account is needed.

The app uses UCI and sends the full move history, allowing Stockfish to recognise
repetitions. Each move uses one thread, 32 MiB of hash memory and 700 ms of search.
Engine searches run off the UI thread and protocol waits time out after 15 seconds.
A failed launch or search leaves the board intact: correct the path and use
**Retry engine**. The engine is not bundled with the app.

## Checks

```sh
npm run lint
npm run build
cd src-tauri
cargo test
```

The real-engine test is optional and uses an installed Stockfish:

```sh
STOCKFISH_EXECUTABLE=/path/to/stockfish cargo test stockfish_returns_legal_reply -- --ignored
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
