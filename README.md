# chess-driller

Drill opening repetoires from PGN files. Continue from the opening playing
against Stockfish or replies drawn from Lichess games.

## Run

Install Node.js, Rust, and the [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/).
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
   the position before the deviation; **Continue with …** keeps your move and uses your selected opponent.
6. At the end of a prepared line, your selected opponent takes over automatically.
   Opponent settings, like your prep selection, apply when you start a drill.

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

## Game history

Given your chess.com or lichess username the chess driller can analyse your
game history against your prep files and identify where you're going out of prep.

## Lichess continuation

As well as playing against stockfish you can also continue against lichess players
of a given skill band. This allows you to get a representative game against players
of a desired strength level.

Choose **Lichess players** under **Continue after prep**, then select a rating
band and time control. The app queries the current position through the
[Lichess Opening Explorer API](https://lichess.org/api#tag/Opening-Explorer/operation/openingExplorerLichess)
and samples replies in proportion to their recorded frequency, counting wins,
draws and losses equally. This includes uncommon moves, not just the most popular
reply. Your imported prep still governs play until you reach its end or choose to
continue after a deviation.

The explorer requires a [Lichess API token](https://lichess.org/account/oauth/token/create)
with no permission scopes selected. Paste it into **Lichess API token**. The app
holds it in memory only, so you need to enter it again after restarting. Queries
send the current board position, rating band and time control to Lichess over
HTTPS. Network or authentication errors leave the board intact; correct the token
or connection and choose **Retry opponent**. Requests time out after 15 seconds;
a rate-limit response asks you to wait at least a minute before retrying.

Rating bands use the average of both players' Lichess ratings. For example,
1200–1399 is a game-average band, not a guarantee about the player making each
move. The result approximates common play in that population; it is not a
calibrated opponent with a specific rating.

With fewer than 10 recorded continuations, the app switches to local Stockfish
for the rest of the game and keeps a handover notice visible. Counting
continuations avoids treating games that ended at this position as usable replies;
the minimum prevents one or two games from defining an entire rating band's play.
Choose **Stockfish fallback skill** from 0 (weakest, the default) to 20 (strongest).
Dropdown estimates such as `0 (~1300 ELO)` use Stockfish's CCRL Blitz calibration,
rounded to the nearest hundred. They are not Lichess ratings, and actual strength
varies with engine version and search settings. Level 20 is shown as full strength
because it disables weakening rather than targeting a fixed rating. Install Stockfish before
starting if you want to finish games after the database runs out.

## Local Stockfish

Download [Stockfish](https://stockfishchess.org/download/) separately. Set the
**Executable name or full path** field to its executable (for example,
`/usr/games/stockfish`), or leave `stockfish` if it is on your PATH. This setting
is remembered locally. No cloud engine or account is needed. Stockfish may
be available via your system package manager.

The app uses UCI and sends the full move history, allowing Stockfish to recognise
repetitions. Each move uses one thread, 32 MiB of hash memory and 700 ms of search.
Engine searches run off the UI thread and protocol waits time out after 15 seconds.
A failed launch or search leaves the board intact: correct the path and use
**Retry opponent**. The engine is not bundled with the app.

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

Live history checks are also optional and use public usernames. They make real
requests and are excluded from the default test suite:

```sh
CHESS_DRILLER_SMOKE_CHESS_COM=erik CHESS_DRILLER_SMOKE_LICHESS=thibault \
  cargo test live_history_providers -- --ignored --nocapture
```

The HTTP fixture test binds a loopback socket. Sandboxed test runners must allow
local sockets. Desktop checks include switching back to an active drill, account
replacement/removal, cancellation, and restarting without network access.

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
