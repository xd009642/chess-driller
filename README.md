# chess-driller

Drill opening repetoires from PGN files. Continue from the opening playing against an engine.

## Run

You'll need to install sdl2 and sdl2-image for this to work.

## Database

For testing I've made a simple sample database from random chapters from the following
lichess studies:

For white:

* [Vienna Gambit: Main Line](https://lichess.org/study/ePMOV5k4) 

For black:

* [Caro-Kann Advance Variation 1](https://lichess.org/study/VJb8YgoJ)

## Plan

* Have a folder with PGN files of planned opening repetoire (divided into black and white)
* Use radix tries and test you on random opening
* Once reaches end of prep chess engine takes over


