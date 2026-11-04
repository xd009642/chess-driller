import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Chessboard } from 'react-chessboard'
import History from './History'

// Rounded inverse of Stockfish's CCRL Blitz calibration for skills 0–19.
// Skill 20 disables weakening and has no fixed rating estimate.
// https://github.com/official-stockfish/Stockfish/blob/sf_19/src/search.h
const skillEloEstimates = [1300, 1400, 1600, 1700, 2000, 2200, 2400, 2500, 2600, 2700, 2800, 2900, 2900, 3000, 3000, 3100, 3100, 3100, 3200, 3200]

const initialPosition = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

function App() {
  const [page, setPage] = useState('practice')
  const [files, setFiles] = useState([])
  const [selected, setSelected] = useState([])
  const [color, setColor] = useState('white')
  const [session, setSession] = useState(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [engineError, setEngineError] = useState('')
  const [enginePath, setEnginePath] = useState(() => localStorage.getItem('stockfish-path') || 'stockfish')
  const [opponentKind, setOpponentKind] = useState('stockfish')
  const [rating, setRating] = useState(1200)
  const [speed, setSpeed] = useState('rapid')
  const [fallbackSkill, setFallbackSkill] = useState(0)
  const [lichessToken, setLichessToken] = useState('')
  const [boardWidth, setBoardWidth] = useState(480)
  const boardContainer = useRef(null)
  const playInfo = useRef(null)
  const pending = useRef(false)
  const visibleFiles = files.filter(file => file.color === color)
  const selectedIds = visibleFiles.filter(file => selected.includes(file.id)).map(file => file.id)

  useEffect(() => {
    invoke('list_prep').then(setFiles).catch(reason => setError(String(reason)))
    const observer = new ResizeObserver(([entry]) => { if (entry.contentRect.width > 0) setBoardWidth(Math.floor(entry.contentRect.width)) })
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
      run('engine_move', { executable: enginePath.trim() || 'stockfish', lichessToken })
    }
  }, [session, busy, engineError, enginePath, lichessToken, run])

  useEffect(() => {
    if (error || engineError) playInfo.current.scrollTop = 0
  }, [error, engineError])

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
  const phaseLabels = { prep: 'In prep', deviated: 'Out of prep', engine: session?.opponent || 'Continued play', finished: 'Game over' }

  return (
    <main>
      <header><div><p className="eyebrow">Opening practice</p><h1>Chess driller</h1></div><span className="local-label">Opening prep · continued play</span></header>
      <nav aria-label="Main navigation">{[['practice', 'Practice'], ['history', 'Game history'], ['settings', 'Settings']].map(([value, label]) => <button key={value} aria-current={page === value ? 'page' : undefined} onClick={() => setPage(value)}>{label}</button>)}</nav>
      <History page={page} files={files} />
      <div className="workspace" hidden={page !== 'practice'}>
        <aside className="panel library" tabIndex={0} aria-label="Repertoire and opponent settings">
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
          <button className="primary" disabled={busy || !selectedIds.length || (opponentKind === 'lichess' && !lichessToken.trim())} onClick={() => run('start', { color, selected: selectedIds, opponent: opponentKind === 'stockfish' ? { kind: 'stockfish' } : { kind: 'lichess', rating, speed, fallback_skill: fallbackSkill } })}>{session ? 'Start new drill' : 'Start drill'} · {selectedIds.length} selected</button>
          <p className="help">Selection, colour and opponent settings apply to your next drill.</p>
          <div className="engine-settings">
            <h3>Continue after prep</h3>
            <label htmlFor="opponent">Opponent</label>
            <select id="opponent" disabled={busy} value={opponentKind} onChange={event => setOpponentKind(event.target.value)}>
              <option value="stockfish">Stockfish</option>
              <option value="lichess">Lichess players</option>
            </select>
            {opponentKind === 'lichess' && <div className="opponent-options">
              <label htmlFor="rating">Lichess rating band</label>
              <select id="rating" disabled={busy} value={rating} onChange={event => setRating(Number(event.target.value))}>
                {[0, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2500].map(value => <option key={value} value={value}>{value === 0 ? 'Below 1000' : value === 2500 ? '2500+' : `${value}–${value === 2200 ? 2499 : value + 199}`}</option>)}
              </select>
              <label htmlFor="speed">Time control</label>
              <select id="speed" disabled={busy} value={speed} onChange={event => setSpeed(event.target.value)}>
                <option value="bullet">Bullet</option><option value="blitz">Blitz</option><option value="rapid">Rapid</option><option value="classical">Classical</option>
              </select>
              <p className="help">Replies are sampled by how often they were played. Bands use the average rating of both players.</p>
              <label htmlFor="fallback-skill">Stockfish fallback skill</label>
              <select id="fallback-skill" disabled={busy} value={fallbackSkill} onChange={event => setFallbackSkill(Number(event.target.value))}>
                {Array.from({ length: 21 }, (_, level) => <option key={level} value={level}>{level === 20 ? '20 (full strength)' : `${level} (~${skillEloEstimates[level]} ELO)`}</option>)}
              </select>
              <p className="help">With fewer than 10 recorded continuations, Stockfish takes over. Elo estimates use the CCRL engine scale, not Lichess ratings; actual strength varies with engine version and search settings.</p>
            </div>}
            {(opponentKind === 'lichess' || session?.uses_lichess) && <div className="opponent-options">
              <label htmlFor="lichess-token">Lichess API token</label>
              <input id="lichess-token" type="password" autoComplete="off" spellCheck={false} disabled={busy} value={lichessToken} onChange={event => setLichessToken(event.target.value)} aria-describedby="token-help" />
              <p className="help" id="token-help">Create a token in your browser at <span className="token-address">lichess.org/account/oauth/token</span> with no permissions selected. Held in memory for this app session only. Online play sends the board position to Lichess.</p>
            </div>}
            <h3 className="stockfish-heading">Local Stockfish</h3>
            <label htmlFor="engine-path">Executable name or full path</label>
            <input id="engine-path" value={enginePath} disabled={busy} onChange={event => {
              setEnginePath(event.target.value)
              localStorage.setItem('stockfish-path', event.target.value)
            }} spellCheck={false} placeholder="stockfish" />
            <p className="help">Install Stockfish separately for engine play and the Lichess fallback. Searches use 0.7 seconds per move.</p>
          </div>
        </aside>
        <section className="play-area" aria-label="Drill">
          <div className="board" ref={boardContainer}>
            <Chessboard id="DrillBoard" position={session?.fen || initialPosition} boardWidth={boardWidth}
              boardOrientation={session?.color || color} onPieceDrop={onPieceDrop}
              arePiecesDraggable={Boolean(canMove)} isDraggablePiece={({ piece }) => piece[0] === (session?.color === 'black' ? 'b' : 'w')}
              animationDuration={150} customDarkSquareStyle={{ backgroundColor: '#739080' }} customLightSquareStyle={{ backgroundColor: '#eee9dc' }} />
          </div>
          <div className="play-info" ref={playInfo} tabIndex={0} role="region" aria-label="Feedback and moves">
            {error && <div className="error" role="alert">{error}</div>}
            {engineError && <div className="error" role="alert"><p>{engineError}</p><button disabled={busy} onClick={() => run('engine_move', { executable: enginePath.trim() || 'stockfish', lichessToken })}>Retry opponent</button></div>}
            <div className="panel feedback" aria-live="polite">
              <span className={`badge ${session?.phase || ''}`}>{session ? phaseLabels[session.phase] : 'Ready to train'}</span>
              <p>{busy && session?.needs_engine ? `${session.opponent} is thinking…` : session?.message || 'Choose your colour and prep files, then start a drill.'}</p>
              {session?.fallback && <p className="handover">Lichess had fewer than 10 recorded continuations. This game is now using {session.opponent}; its strength is not matched to the Lichess rating band.</p>}
              {session?.phase === 'deviated' && <>
                <p>Prepared moves: <strong>{session.alternatives.join(', ')}</strong></p>
                <div className="actions"><button disabled={busy} onClick={() => run('retry')}>Retry move</button><button disabled={busy} onClick={() => run('continue_engine')}>Continue with {session.opponent}</button></div>
              </>}
            </div>
            {session && <div className="panel details">
              <h3>{session.phase === 'prep' ? 'Matching prep' : 'Last matched prep'}</h3>
              <ul>{session.sources.map(source => <li key={`${source.file_id}:${source.chapter_number}`}><span>{source.chapter}</span><small>{source.file}</small></li>)}</ul>
              <h3>Moves</h3>
              <p className="moves">{session.moves.length ? session.moves.map((move, index) => `${index % 2 === 0 ? `${Math.floor(index / 2) + 1}. ` : ''}${move}`).join(' ') : 'No moves yet.'}</p>
            </div>}
          </div>
        </section>
      </div>
    </main>
  )
}

export default App
