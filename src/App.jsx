import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Chessboard } from 'react-chessboard'

const initialPosition = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

function App() {
  const [files, setFiles] = useState([])
  const [selected, setSelected] = useState([])
  const [color, setColor] = useState('white')
  const [session, setSession] = useState(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [engineError, setEngineError] = useState('')
  const [enginePath, setEnginePath] = useState(() => localStorage.getItem('stockfish-path') || 'stockfish')
  const [boardWidth, setBoardWidth] = useState(480)
  const boardContainer = useRef(null)
  const pending = useRef(false)
  const visibleFiles = files.filter(file => file.color === color)
  const selectedIds = visibleFiles.filter(file => selected.includes(file.id)).map(file => file.id)

  useEffect(() => {
    invoke('list_prep').then(setFiles).catch(reason => setError(String(reason)))
    const observer = new ResizeObserver(([entry]) => setBoardWidth(Math.floor(entry.contentRect.width)))
    observer.observe(boardContainer.current)
    return () => observer.disconnect()
  }, [])

  const run = useCallback(async (command, args = {}) => {
    if (pending.current) return
    pending.current = true
    setBusy(true)
    setError('')
    setEngineError('')
    try {
      const result = await invoke(command, args)
      setSession(result)
    } catch (reason) {
      if (command === 'engine_move') setEngineError(String(reason))
      else setError(String(reason))
    } finally {
      pending.current = false
      setBusy(false)
    }
  }, [])

  useEffect(() => {
    if (session?.needs_engine && !busy && !engineError) {
      run('engine_move', { executable: enginePath.trim() || 'stockfish' })
    }
  }, [session, busy, engineError, enginePath, run])

  async function importFiles(event) {
    const uploads = Array.from(event.target.files)
    event.target.value = ''
    if (pending.current || uploads.length === 0) return
    pending.current = true
    setBusy(true)
    setError('')
    const failures = []
    try {
      for (const file of uploads) {
        try {
          const result = await invoke('import_prep', { name: file.name, color, pgn: await file.text() })
          setFiles(result)
          setSelected(previous => [...previous, result[result.length - 1].id])
        } catch (reason) {
          failures.push(`${file.name}: ${reason}`)
        }
      }
      setError(failures.join('\n'))
    } finally {
      pending.current = false
      setBusy(false)
    }
  }

  function onPieceDrop(from, to, piece) {
    if (pending.current || !session || session.needs_engine || !['prep', 'engine'].includes(session.phase)) return false
    // The callback supplies the chosen promotion piece, so inspect the source
    // position to distinguish promotion from an ordinary queen or rook move.
    const rank = session.fen.split(' ')[0].split('/')[8 - Number(from[1])]
      .replace(/\d/g, count => '.'.repeat(Number(count)))
    const pawn = rank[from.charCodeAt(0) - 97].toLowerCase() === 'p'
    const promotion = pawn && ['1', '8'].includes(to[1]) ? piece[1].toLowerCase() : ''
    run('move_piece', { uci: `${from}${to}${promotion}` })
    return false // The Rust position is authoritative, including rejected moves.
  }

  const canMove = session && !busy && !session.needs_engine && ['prep', 'engine'].includes(session.phase)
  const phaseLabels = { prep: 'In prep', deviated: 'Out of prep', engine: 'Engine play', finished: 'Game over' }

  return (
    <main>
      <header><div><p className="eyebrow">Opening practice</p><h1>Chess driller</h1></div><span className="local-label">Local prep · local engine</span></header>
      <div className="workspace">
        <aside className="panel library">
          <h2>Your repertoire</h2>
          <label htmlFor="color">Train as</label>
          <select id="color" value={color} disabled={busy} onChange={event => setColor(event.target.value)}>
            <option value="white">White</option><option value="black">Black</option>
          </select>
          <label className={`import-button ${busy ? 'disabled' : ''}`}>
            Import PGN files
            <input type="file" accept=".pgn" multiple disabled={busy} onChange={importFiles} />
          </label>
          <p className="help">Import downloaded studies for {color}. Files are copied into your local library.</p>
          <div className="selection-heading"><h3>Prep files</h3><button className="text-button" disabled={busy || !visibleFiles.length} onClick={() => setSelected(previous => [...new Set([...previous, ...visibleFiles.map(file => file.id)])])}>Select all</button></div>
          {!visibleFiles.length && <p className="empty">No {color} prep yet. Import a PGN to get started.</p>}
          <div className="file-list">
            {visibleFiles.map(file => <label className="file" key={file.id}>
              <input type="checkbox" disabled={busy} checked={selected.includes(file.id)} onChange={event => setSelected(previous => event.target.checked ? [...previous, file.id] : previous.filter(id => id !== file.id))} />
              <span>{file.name}</span>
            </label>)}
          </div>
          <button className="primary" disabled={busy || !selectedIds.length} onClick={() => run('start', { color, selected: selectedIds })}>{session ? 'Start new drill' : 'Start drill'} · {selectedIds.length} selected</button>
          <p className="help">Selection and colour apply to your next drill.</p>
          <div className="engine-settings">
            <h3>Stockfish</h3>
            <label htmlFor="engine-path">Executable name or full path</label>
            <input id="engine-path" value={enginePath} disabled={busy} onChange={event => {
              setEnginePath(event.target.value)
              localStorage.setItem('stockfish-path', event.target.value)
            }} spellCheck={false} placeholder="stockfish" />
            <p className="help">Install Stockfish separately. It takes over at the end of your prep, with 0.7 seconds per move.</p>
          </div>
        </aside>
        <section className="play-area" aria-label="Drill">
          <div className="panel feedback" aria-live="polite">
            <span className={`badge ${session?.phase || ''}`}>{session ? phaseLabels[session.phase] : 'Ready to train'}</span>
            <p>{busy && session?.needs_engine ? 'Stockfish is thinking…' : session?.message || 'Choose your colour and prep files, then start a drill.'}</p>
            {session?.phase === 'deviated' && <>
              <p>Prepared moves: <strong>{session.alternatives.join(', ')}</strong></p>
              <div className="actions"><button disabled={busy} onClick={() => run('retry')}>Retry move</button><button disabled={busy} onClick={() => run('continue_engine')}>Continue with Stockfish</button></div>
            </>}
          </div>
          {error && <div className="error" role="alert">{error}</div>}
          {engineError && <div className="error" role="alert"><p>{engineError}</p><button disabled={busy} onClick={() => run('engine_move', { executable: enginePath.trim() || 'stockfish' })}>Retry engine</button></div>}
          <div className="board" ref={boardContainer}>
            <Chessboard id="DrillBoard" position={session?.fen || initialPosition} boardWidth={boardWidth}
              boardOrientation={session?.color || color} onPieceDrop={onPieceDrop}
              arePiecesDraggable={Boolean(canMove)} isDraggablePiece={({ piece }) => piece[0] === (session?.color === 'black' ? 'b' : 'w')}
              animationDuration={150} customDarkSquareStyle={{ backgroundColor: '#739080' }} customLightSquareStyle={{ backgroundColor: '#eee9dc' }} />
          </div>
          {session && <div className="panel details">
            <h3>{session.phase === 'prep' ? 'Matching prep' : 'Last matched prep'}</h3>
            <ul>{session.sources.map(source => <li key={`${source.file_id}:${source.chapter_number}`}><span>{source.chapter}</span><small>{source.file}</small></li>)}</ul>
            <h3>Moves</h3>
            <p className="moves">{session.moves.length ? session.moves.map((move, index) => `${index % 2 === 0 ? `${Math.floor(index / 2) + 1}. ` : ''}${move}`).join(' ') : 'No moves yet.'}</p>
          </div>}
        </section>
      </div>
    </main>
  )
}

export default App
