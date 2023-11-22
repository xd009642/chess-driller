import { useState } from 'react'
import { invoke } from '@tauri-apps/api'

import { Chessboard } from "react-chessboard";

function App() {
  const [game, setGame] = useState("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")

  function onPieceDrop(sourceSquare, targetSquare){
    invoke('move_piece', { 'from': sourceSquare, 'to': targetSquare })
      .then((response) => setGame(response))
  }

  return (
    <div className="w-[100vmin] h-[100vmin]">
      <Chessboard id="BasicBoard" position={game} onPieceDrop={onPieceDrop}/>
    </div>
  )
}

export default App
