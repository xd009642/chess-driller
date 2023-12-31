import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api'

import { Chessboard } from "react-chessboard";

function App() {
  const [game, setGame] = useState("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")
  let [orientation, setOrientation] = useState("white")
  let [promotion] = useState("Q")

  useEffect(function(){
    document.onkeypress = handleKeyUp
  },[])

  function onPieceDrop(sourceSquare, targetSquare, piece){
    invoke('move_piece', { 'from': sourceSquare, 'to': targetSquare, "promotion": piece ?? "Q" })
      .then((response) => setGame(response))
  }

  function handleKeyUp(event) {
      if (event.key == 'f') {
          if (orientation == "white") {
              setOrientation("black");
              invoke("reset", {"color": "black" }) 
          } else {
              setOrientation("white");
              invoke("reset", {"color": "white" }) 
          }
      } else if (event.key == "s") {
          invoke("start", {  })
            .then((response) => setGame(response))
      } else if (event.key == "r") {
          setGame("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR");
          invoke("reset", {"color": orientation }) 
      }
  }

  return (
    <div className="w-[100vmin] h-[100vmin]">
      <Chessboard id="BasicBoard" position={game} onPieceDrop={onPieceDrop} boardOrientation={orientation} animationDuration="0"/>
    </div>
  )
}

export default App
