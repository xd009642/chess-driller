# chess-driller

Drill opening repetoires from PGN files. Continue from the opening playing
against an engine.

## Run

You'll need to install the tauri stuff to work (sort out links).

Then to launch the program:

```
npm install
cargo tauri dev
```

If that doesn't work you may need to do a 

```
npm install
```

Currently, I just throw in an `npm install` if it complains about javascript
packages and it seems to fix it...

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
