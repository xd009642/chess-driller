# Design

Here's a rough attempt at characterising what is desired from the design of
this program. This is a desktop app not a webapp btw (though it does use
something similar to electron so javascript is used for the FE).

In chess the opening is the start of the game, and in order to get a strategic
edge people learn different openings. So for the first so many moves of the
game you have a general view of sequences of moves, decision points etc.
At any one move we make there may also be multiple potential moves in our
prep we can do - depending on how comprehensive it is. You can see an example
of how opening prep is rendered in lichess [here](https://lichess.org/study/DbO6ys1v)

Given files where each one is an opening to study that make up our "prep" we
want to do a few things. I'll split these into gameplay and analysis.

## Gameplay

* Pick a side (white, black, maybe random)
* Potentially limit which prep files to use (There will be a list of white opening files 
black opening files)
* Play through a opening and get feedback on when we go wrong 
    - If we go wrong either choose to reset or keep playing with computer
* After the opening play vs a computer 
    - We may also want to query lichess database to play the new few moves 
    based on what the average player does
* See the sequence of moves and if we play a full game export a PGN of it

## Analysis

* Check our own play via chess.com/lichess and see where we are forgetting our prep
* Explore win/lose statistics via certain openings (like https://www.openingtree.com/) 

