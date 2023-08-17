use anyhow::anyhow;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::EventPump;

enum DragState {
    Dragging,
    MaybeDragging,
    NotDragging,
}

pub struct EventSystem {
    pump: EventPump,
    drag_state: DragState,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub timestamp: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    Close,
    /// Flips the board perspective
    FlipBoard,
    /// Reset the board
    Reset,
    /// Click somewhere on the screen
    MouseDown {
        x: i32,
        y: i32,
    },
    MouseUp {
        x: i32,
        y: i32,
    },
    MouseClick {
        x: i32,
        y: i32,
    },
    MouseDragBegin {
        x: i32,
        y: i32,
    },
    MouseDragMove {
        x: i32,
        y: i32,
    },
    MouseDragEnd {
        x: i32,
        y: i32,
    },
    /// Start playing through your prep
    StartPractising,
}

impl EventSystem {
    pub fn new(sdl: sdl2::Sdl) -> anyhow::Result<Self> {
        Ok(Self {
            pump: sdl.event_pump().map_err(|e| anyhow!(e))?,
            drag_state: DragState::NotDragging,
        })
    }

    pub fn handle_events(&mut self) -> Vec<Event> {
        let mut events = vec![];

        for event in self.pump.poll_iter() {
            use sdl2::event::Event as SdlEvent;

            match event {
                SdlEvent::Quit { timestamp, .. } => {
                    events.push(Event {
                        kind: EventKind::Close,
                        timestamp,
                    });
                }
                SdlEvent::KeyUp {
                    keycode, timestamp, ..
                } => match keycode {
                    Some(Keycode::F) => {
                        events.push(Event {
                            kind: EventKind::FlipBoard,
                            timestamp,
                        });
                    }
                    Some(Keycode::R) => {
                        events.push(Event {
                            kind: EventKind::Reset,
                            timestamp,
                        });
                    }
                    Some(Keycode::Space) => {
                        events.push(Event {
                            kind: EventKind::StartPractising,
                            timestamp,
                        });
                    }
                    _ => {
                        println!("Unsupported key: {:?}", keycode);
                    }
                },
                SdlEvent::MouseButtonUp {
                    x,
                    y,
                    mouse_btn,
                    timestamp,
                    ..
                } if mouse_btn == MouseButton::Left => {
                    if let DragState::Dragging = self.drag_state {
                        self.drag_state = DragState::NotDragging;
                        events.push(Event {
                            kind: EventKind::MouseDragEnd { x, y },
                            timestamp,
                        });
                        continue;
                    }

                    if let Some(Event {
                        kind: EventKind::MouseDown { x: x2, y: y2 },
                        timestamp: last_timestamp,
                    }) = events.last()
                    {
                        if timestamp - last_timestamp < 150
                            && x.abs_diff(*x2) < 5
                            && y.abs_diff(*y2) < 5
                        {
                            events.pop();
                            events.push(Event {
                                kind: EventKind::MouseClick { x, y },
                                timestamp,
                            });
                        }
                    } else if let DragState::MaybeDragging = self.drag_state {
                        events.push(Event {
                            kind: EventKind::MouseClick { x, y },
                            timestamp,
                        });
                        self.drag_state = DragState::NotDragging;
                    } else {
                        events.push(Event {
                            kind: EventKind::MouseUp { x, y },
                            timestamp,
                        });
                    }
                }
                SdlEvent::MouseButtonDown {
                    x,
                    y,
                    mouse_btn,
                    timestamp,
                    ..
                } if mouse_btn == MouseButton::Left => {
                    events.push(Event {
                        kind: EventKind::MouseDown { x, y },
                        timestamp,
                    });
                }
                SdlEvent::MouseMotion {
                    timestamp, x, y, ..
                } => match self.drag_state {
                    DragState::Dragging => {
                        events.push(Event {
                            kind: EventKind::MouseDragMove { x, y },
                            timestamp,
                        });
                    }
                    DragState::MaybeDragging => {
                        events.push(Event {
                            kind: EventKind::MouseDragBegin { x, y },
                            timestamp,
                        });

                        self.drag_state = DragState::Dragging;
                    }
                    DragState::NotDragging => {
                        events.push(Event {
                            kind: EventKind::MouseUp { x, y },
                            timestamp,
                        });
                    }
                },
                _e => {}
            }
        }
        // I'm lazy and start practising should be at the end to make sure we don't start playing
        // against our white prep as black
        events.sort_by_key(|e| e.kind);

        if let Some(Event {
            kind: EventKind::MouseDown { .. },
            ..
        }) = events.last()
        {
            self.drag_state = DragState::MaybeDragging;
        }

        events
    }
}
