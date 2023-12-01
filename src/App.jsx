import { useState } from 'react'
import { invoke } from '@tauri-apps/api'

import { Chessboard } from "react-chessboard";

function App() {
  const [game, setGame] = useState("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")
  let [orientation] = useState("white")
  let [promotion] = useState("Q")

  function onPieceDrop(sourceSquare, targetSquare, piece){
    invoke('move_piece', { 'from': sourceSquare, 'to': targetSquare, "promotion": piece ?? "Q" })
      .then((response) => setGame(response))
  }

  return (
    <div className="w-[100vmin] h-[100vmin]">
      <Chessboard id="BasicBoard" position={game} onPieceDrop={onPieceDrop} animationDuration="0"/>
    </div>
  )
}

export default App
