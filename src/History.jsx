/* eslint-disable react/prop-types */
import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Chessboard } from 'react-chessboard'

const providers = { chess_com: 'Chess.com', lichess: 'Lichess' }
const labels = { no_prep: 'No prep selected', prep_completed: 'Prep completed', your_deviation: 'Your deviation', opponent_left_prep: 'Opponent left prep', game_ended_in_prep: 'Game ended in prep' }
const date = ms => ms ? new Date(ms).toLocaleString() : 'Never'
const key = game => `${game.provider}:${game.id}`

function AccountForm({ provider, account, save }) {
  const [username, setUsername] = useState(account?.username || '')
  const [confirm, setConfirm] = useState(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  async function commit(value) {
    setBusy(true)
    setError('')
    try {
      await save(provider, value)
      setUsername(value || '')
      setConfirm(null)
    } catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }
  function submit(event) {
    event.preventDefault()
    const value = username.trim()
    if (account && value.toLowerCase() !== account.username.toLowerCase()) setConfirm({ value })
    else commit(value)
  }
  return <form className="panel account-form" onSubmit={submit}>
    <h3>{providers[provider]}</h3>
    <label htmlFor={`username-${provider}`}>Public username</label>
    <input id={`username-${provider}`} value={username} onChange={e => { setUsername(e.target.value); setConfirm(null) }} autoComplete="off" spellCheck={false} required minLength={2} maxLength={25} pattern="[A-Za-z0-9_-]+" disabled={busy} />
    <p className="help">{account ? `Saved as ${account.username}. Last successful fetch: ${date(account.last_success_ms)}.` : 'No account saved.'}</p>
    {error && <p className="error" role="alert">{error}</p>}
    <div className="actions"><button disabled={busy}>Save locally</button>{account && <button type="button" disabled={busy} onClick={() => setConfirm({ value: null })}>Remove account</button>}</div>
    {confirm && <div className="confirmation" role="alert">
      <p>{confirm.value === null ? 'Remove this account' : `Replace ${account.username} with ${confirm.value}`} and delete its downloaded history? Your prep files are kept.</p>
      <div className="actions"><button type="button" disabled={busy} onClick={() => commit(confirm.value)}>Confirm and clear history</button><button type="button" onClick={() => setConfirm(null)}>Keep account</button></div>
    </div>}
  </form>
}

function ReviewDetails({ row }) {
  const { game, review } = row
  const [ply, setPly] = useState(review.departure_ply ? review.departure_ply - 1 : review.matched_ply)
  const [width, setWidth] = useState(360)
  const [error, setError] = useState('')
  const container = useRef(null)
  useEffect(() => {
    const observer = new ResizeObserver(([entry]) => { if (entry.contentRect.width > 0) setWidth(Math.floor(entry.contentRect.width)) })
    observer.observe(container.current)
    return () => observer.disconnect()
  }, [])
  const position = ply === 0 ? (game.moves[0]?.before || review.fen) : game.moves[ply - 1].after
  return <section className="panel review-details" aria-label="Game review">
    <h2>{game.white} – {game.black}</h2>
    <p>{labels[review.classification]} · {review.matched_ply} matched plies</p>
    <div className="review-board" ref={container}><Chessboard id="HistoryBoard" position={position} boardWidth={width} boardOrientation={game.color} arePiecesDraggable={false} animationDuration={0} /></div>
    <div className="actions"><button disabled={ply === 0} onClick={() => setPly(ply - 1)}>Previous</button><button disabled={ply === game.moves.length} onClick={() => setPly(ply + 1)}>Next</button><button onClick={() => setPly(review.departure_ply ? review.departure_ply - 1 : review.matched_ply)}>Review position</button></div>
    {review.played && <p>Played: {review.played.san} ({review.played.uci})</p>}
    {review.alternatives.length > 0 && <><h3>Prepared alternatives</h3><ul>{review.alternatives.map(move => <li key={move.uci}>{move.san}<ul>{move.sources.map(source => <li key={`${source.file_id}:${source.chapter_number}`}>{source.file} · {source.chapter}</li>)}</ul></li>)}</ul></>}
    {!review.alternatives.length && review.sources.length > 0 && <><h3>Matching prep</h3><ul>{review.sources.map(source => <li key={`${source.file_id}:${source.chapter_number}`}>{source.file} · {source.chapter}</li>)}</ul></>}
    <p className="help">A departure means different from selected prep, not necessarily a bad move. Different move orders are reviewed separately.</p>
    <h3>Moves</h3><div className="history-moves">{game.moves.map((move, index) => <button key={index} aria-pressed={ply === index + 1} onClick={() => setPly(index + 1)}>{move.san}</button>)}</div>
    <a href={game.url} onClick={event => { event.preventDefault(); invoke('open_history_game', { provider: game.provider, url: game.url }).catch(reason => setError(String(reason))) }}>Open original game on {providers[game.provider]}</a>
    {error && <p className="error" role="alert">{error}</p>}
  </section>
}

export default function History({ page, files }) {
  const [state, setState] = useState(null)
  const [error, setError] = useState('')
  const [excluded, setExcluded] = useState([])
  const [rows, setRows] = useState([])
  const [detail, setDetail] = useState(null)
  const [provider, setProvider] = useState('')
  const [color, setColor] = useState('')
  const [result, setResult] = useState('')
  const [outcome, setOutcome] = useState('')
  const [reviewing, setReviewing] = useState(false)
  const selected = files.filter(file => !excluded.includes(file.id)).map(file => file.id)
  const selectionKey = JSON.stringify(selected)
  // Only persistent data changes trigger comparison; progress polling must not reset details.
  const dataKey = state?.revision
  useEffect(() => {
    let active = true
    async function poll() {
      try { const value = await invoke('history_state'); if (active) setState(value) }
      catch (reason) { if (active) setError(String(reason)) }
    }
    poll()
    const timer = setInterval(poll, 1000)
    return () => { active = false; clearInterval(timer) }
  }, [])
  useEffect(() => {
    let active = true
    setReviewing(true)
    setRows([])
    invoke('review_history', { selected: JSON.parse(selectionKey) }).then(value => {
      if (active) { setRows(value); setError('') }
    }).catch(reason => { if (active) setError(String(reason)) }).finally(() => { if (active) setReviewing(false) })
    return () => { active = false }
  }, [selectionKey, dataKey])
  async function save(provider, username) {
    const value = await invoke('save_account', { provider, username })
    setDetail(null)
    setState(value)
  }
  async function refresh() {
    setError('')
    try { await invoke('refresh_history'); setState(await invoke('history_state')) }
    catch (reason) { setError(String(reason)) }
  }
  const filtered = rows.filter(row => (!provider || row.game.provider === provider) && (!color || row.game.color === color) && (!outcome || row.review.classification === outcome) && (!result || (row.game.result === '1/2-1/2' ? 'draw' : (row.game.result === '1-0') === (row.game.color === 'white') ? 'win' : 'loss') === result))
  const row = filtered.find(row => key(row.game) === detail)
  return <div className="history-page" hidden={page === 'practice'}>
    {error && <div className="error" role="alert">{error}</div>}
    {state?.error && <div className="error" role="alert">{state.error}</div>}
    {page === 'settings' && <>
      <h2>Settings</h2><p>Save a public Chess.com or Lichess username to review recent games. This selects a public account; it does not sign you in or verify ownership. Saving works offline.</p>
      <p className="help">History needs no token. The token in Practice belongs to “Lichess players” continuation and stays in memory only.</p>
      <div className="account-grid">{Object.keys(providers).map(provider => <AccountForm key={`${provider}:${state?.accounts.find(a => a.provider === provider)?.username || ''}`} provider={provider} account={state?.accounts.find(a => a.provider === provider)} save={save} />)}</div>
    </>}
    {page === 'history' && <>
      <div className="history-heading"><h2>Game history</h2><div className="actions"><button disabled={!state?.accounts.length || state?.busy || Boolean(state?.error)} onClick={refresh}>Refresh games</button>{state?.busy && <button onClick={() => invoke('cancel_history').then(setState).catch(reason => setError(String(reason)))}>Cancel refresh</button>}</div></div>
      <p className="help">Latest 100 eligible games per account in the past 90 days. Completed standard live games, rated and casual; daily and correspondence games are excluded. Chess.com dates are completion times. Lichess filters by start time and displays last-move times.</p>
      <p className="help">Refresh is manual. Review works offline after downloading; Chess.com may publish recent games with a delay.</p>
      <p role="status">{state?.progress}</p>
      {!state?.accounts.length && <p className="empty">No account saved. Add a public username in Settings.</p>}
      {state?.accounts.map(account => <details className="coverage" key={account.provider} open={Boolean(account.error)}><summary>{providers[account.provider]} · {account.username} · {account.cached_count} cached games · Last success: {date(account.last_success_ms)}</summary>
        {account.error && <p className="error">{account.error} Previously cached games remain available.</p>}
        <p>Last attempt: {date(account.attempted_ms)}. Scanned {account.coverage.scanned}; retained {account.coverage.eligible} eligible games.</p>
        {account.coverage.since_ms > 0 && <p>Requested range: {date(account.coverage.since_ms)} – {date(account.coverage.until_ms)}</p>}
        {account.cached_count > 0 && <p>Cached dates: {date(account.oldest_ms)} – {date(account.newest_ms)}</p>}
        {Object.entries(account.coverage.skipped).map(([reason, count]) => <p key={reason}>Skipped {count}: {reason}</p>)}
        {account.coverage.limits.map(limit => <p key={limit}>{limit}</p>)}
        <p className="help">Coverage is bounded, not a complete account history.</p>
      </details>)}
      <div className="history-layout"><section>
        <details className="panel history-prep"><summary>Review prep · {selected.length} files selected</summary><p className="help">Independent of your active drill. New imports are selected automatically.</p>
          {['white', 'black'].map(side => <fieldset key={side}><legend>{side === 'white' ? 'White prep' : 'Black prep'}</legend>{!files.some(file => file.color === side && selected.includes(file.id)) && <p className="empty">No prep selected for {side}.</p>}{files.filter(file => file.color === side).map(file => <label className="file" key={file.id}><input type="checkbox" checked={selected.includes(file.id)} onChange={e => setExcluded(previous => e.target.checked ? previous.filter(id => id !== file.id) : [...previous, file.id])} />{file.name}</label>)}</fieldset>)}
        </details>
        <div className="history-filters"><label>Provider<select value={provider} onChange={e => setProvider(e.target.value)}><option value="">All providers</option>{Object.entries(providers).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label><label>Colour<select value={color} onChange={e => setColor(e.target.value)}><option value="">Both colours</option><option value="white">White</option><option value="black">Black</option></select></label><label>Result<select value={result} onChange={e => setResult(e.target.value)}><option value="">All results</option><option value="win">Win</option><option value="loss">Loss</option><option value="draw">Draw</option></select></label><label>Review outcome<select value={outcome} onChange={e => setOutcome(e.target.value)}><option value="">All outcomes</option>{Object.entries(labels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></label></div>
        {reviewing ? <p role="status">Comparing selected prep…</p> : !rows.length && state?.accounts.length > 0 ? <p className="empty">No downloaded games. Refresh to fetch recent games; any skipped games or errors appear above.</p> : !filtered.length && rows.length > 0 ? <p className="empty">No games match these filters.</p> : null}
        <div className="history-rows">{filtered.map(({ game, review }) => <button className="history-row" key={key(game)} aria-pressed={key(game) === detail} onClick={() => setDetail(key(game))}>
          <span>{game.color === 'white' ? game.black : game.white} · {game.color} · {game.result}</span><small>{date(game.played_ms)} · {providers[game.provider]}</small><span>{labels[review.classification]}{review.played ? ` · ${review.played.san}` : ''}</span>
        </button>)}</div>
      </section>{row ? <ReviewDetails key={`${key(row.game)}:${selectionKey}`} row={row} /> : <p className="empty">Select a game to inspect its moves and matching prep.</p>}</div>
    </>}
  </div>
}
